use orsx::columnar::{
    ColumnarBatch, ColumnarField, ColumnarReadConfig, ColumnarSchema, ColumnarType,
    CopyBinaryBatchReader,
};
use orsx::SqlxTimestamp;
use sqlx::{Connection, Executor};
use uuid::Uuid;

fn validity_get(validity: &[u8], row: usize) -> bool {
    let byte = validity[row / 8];
    let bit = 1u8 << (row % 8);
    (byte & bit) != 0
}

#[tokio::test]
async fn columnar_copy_binary_matches_row_wise_for_mixed_types() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());

    let mut conn_setup = sqlx::PgConnection::connect(&url).await.unwrap();

    conn_setup
        .execute("DROP TABLE IF EXISTS orscol_test")
        .await
        .unwrap();
    conn_setup
        .execute(
        r#"
        CREATE TABLE orscol_test (
            id BIGINT PRIMARY KEY,
            b BOOLEAN NULL,
            i32 INTEGER NULL,
            f64 DOUBLE PRECISION NULL,
            u UUID NULL,
            ts TIMESTAMPTZ NULL,
            t TEXT NULL,
            by BYTEA NULL
        )
        "#,
        )
        .await
        .unwrap();

    let u0 = Uuid::new_v4();
    let u1 = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO orscol_test (id, b, i32, f64, u, ts, t, by)
        VALUES
            (1, true,  10, 1.25, $1, '2020-01-01T00:00:00Z', 'hello', E'\\x0102'),
            (2, NULL,  NULL, NULL, NULL, NULL, NULL, NULL),
            (3, false, -7,  3.5,  $2, '2020-01-01T00:00:00.000001Z', 'x', E'\\x')
        "#,
    )
    .bind(u0)
    .bind(u1)
    .execute(&mut conn_setup)
    .await
    .unwrap();

    drop(conn_setup);

    let mut conn_copy = sqlx::PgConnection::connect(&url).await.unwrap();
    let mut conn_row = sqlx::PgConnection::connect(&url).await.unwrap();

    let schema = ColumnarSchema::new(vec![
        ColumnarField {
            name: Some("id".to_string()),
            ty: ColumnarType::I64,
        },
        ColumnarField {
            name: Some("b".to_string()),
            ty: ColumnarType::Bool,
        },
        ColumnarField {
            name: Some("i32".to_string()),
            ty: ColumnarType::I32,
        },
        ColumnarField {
            name: Some("f64".to_string()),
            ty: ColumnarType::F64,
        },
        ColumnarField {
            name: Some("u".to_string()),
            ty: ColumnarType::Uuid,
        },
        ColumnarField {
            name: Some("ts".to_string()),
            ty: ColumnarType::TimestampTzMicros,
        },
        ColumnarField {
            name: Some("t".to_string()),
            ty: ColumnarType::Utf8,
        },
        ColumnarField {
            name: Some("by".to_string()),
            ty: ColumnarType::Bytes,
        },
    ])
    .unwrap();

    let mut reader = CopyBinaryBatchReader::new_select_unchecked(
        &mut conn_copy,
        "SELECT id, b, i32, f64, u, ts, t, by FROM orscol_test ORDER BY id",
        schema.clone(),
    )
    .await
    .unwrap()
    .with_read_config(ColumnarReadConfig {
        validate_utf8: true,
        var_inline_limit: 64 * 1024,
    });

    let mut batch = ColumnarBatch::new(schema, 16).unwrap();
    let rows = reader.next_batch_into(&mut batch).await.unwrap();
    assert_eq!(rows, 3);

    let expected: Vec<(i64, Option<bool>, Option<i32>, Option<f64>, Option<Uuid>, Option<i64>, Option<String>, Option<Vec<u8>>)> =
        sqlx::query_as(
            "SELECT id, b, i32, f64, u, ts, t, by FROM orscol_test ORDER BY id",
        )
        .fetch_all(&mut conn_row)
        .await
        .unwrap()
        .into_iter()
        .map(|(id, b, i32v, f64v, u, ts, t, by): (i64, Option<bool>, Option<i32>, Option<f64>, Option<Uuid>, Option<SqlxTimestamp>, Option<String>, Option<Vec<u8>>)| {
            let ts_micros = ts.map(|t| t.to_jiff().as_microsecond());
            (id, b, i32v, f64v, u, ts_micros, t, by)
        })
        .collect();

    // id
    {
        let validity = batch.column_validity_bytes(0).unwrap();
        let values = batch.fixed_i64(0).unwrap();
        for (row, (id, ..)) in expected.iter().enumerate() {
            assert!(validity_get(validity, row));
            let got = values[row];
            assert_eq!(got, *id);
        }
    }

    // b
    {
        let validity = batch.column_validity_bytes(1).unwrap();
        let values = batch.fixed_bool_bytes(1).unwrap();
        for (row, (_, b, ..)) in expected.iter().enumerate() {
            let is_valid = validity_get(validity, row);
            assert_eq!(is_valid, b.is_some());
            if let Some(v) = b {
                let got = values[row] != 0;
                assert_eq!(got, *v);
            }
        }
    }

    // i32
    {
        let validity = batch.column_validity_bytes(2).unwrap();
        let values = batch.fixed_i32(2).unwrap();
        for (row, (_, _, i32v, ..)) in expected.iter().enumerate() {
            let is_valid = validity_get(validity, row);
            assert_eq!(is_valid, i32v.is_some());
            if let Some(v) = i32v {
                let got = values[row];
                assert_eq!(got, *v);
            }
        }
    }

    // f64
    {
        let validity = batch.column_validity_bytes(3).unwrap();
        let values = batch.fixed_f64_bits(3).unwrap();
        for (row, (_, _, _, f64v, ..)) in expected.iter().enumerate() {
            let is_valid = validity_get(validity, row);
            assert_eq!(is_valid, f64v.is_some());
            if let Some(v) = f64v {
                let got = f64::from_bits(values[row]);
                assert_eq!(got, *v);
            }
        }
    }

    // uuid
    {
        let validity = batch.column_validity_bytes(4).unwrap();
        let values = batch.fixed_uuid_bytes(4).unwrap();
        for (row, (_, _, _, _, u, ..)) in expected.iter().enumerate() {
            let is_valid = validity_get(validity, row);
            assert_eq!(is_valid, u.is_some());
            if let Some(v) = u {
                let got = Uuid::from_slice(values[row].as_slice()).unwrap();
                assert_eq!(got, *v);
            }
        }
    }

    // timestamptz micros
    {
        let validity = batch.column_validity_bytes(5).unwrap();
        let values = batch.fixed_timestamp_micros(5).unwrap();
        for (row, (_, _, _, _, _, ts, ..)) in expected.iter().enumerate() {
            let is_valid = validity_get(validity, row);
            assert_eq!(is_valid, ts.is_some());
            if let Some(v) = ts {
                let got = values[row];
                assert_eq!(got, *v);
            }
        }
    }

    // utf8
    {
        let validity = batch.column_validity_bytes(6).unwrap();
        let offsets = batch.var_chunks(6).unwrap().0;
        let mut data = Vec::new();
        batch.coalesce_var_into(6, &mut data).unwrap();
        for (row, (_, _, _, _, _, _, t, ..)) in expected.iter().enumerate() {
            let is_valid = validity_get(validity, row);
            assert_eq!(is_valid, t.is_some());
            let start = offsets[row] as usize;
            let end = offsets[row + 1] as usize;
            if let Some(v) = t {
                let got = std::str::from_utf8(&data[start..end]).unwrap();
                assert_eq!(got, v);
            } else {
                assert_eq!(start, end);
            }
        }
    }

    // bytes
    {
        let validity = batch.column_validity_bytes(7).unwrap();
        let offsets = batch.var_chunks(7).unwrap().0;
        let mut data = Vec::new();
        batch.coalesce_var_into(7, &mut data).unwrap();
        for (row, (_, _, _, _, _, _, _, by)) in expected.iter().enumerate() {
            let is_valid = validity_get(validity, row);
            assert_eq!(is_valid, by.is_some());
            let start = offsets[row] as usize;
            let end = offsets[row + 1] as usize;
            if let Some(v) = by {
                assert_eq!(&data[start..end], v.as_slice());
            } else {
                assert_eq!(start, end);
            }
        }
    }
}
