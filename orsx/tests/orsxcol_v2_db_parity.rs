use orsx::columnar::{
    decode_orsxcol_v2, encode_orsxcol_v2_into, ColumnarBatch, ColumnarField,
    ColumnarSchema, ColumnarType, CopyBinaryBatchReader, RowWiseBatchReader,
};
use sqlx::{Connection, Executor};
use uuid::Uuid;

fn test_db_url() -> String {
    std::env::var("ORSX_TEST_DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string()
    })
}

fn validity_get(validity: &[u8], row: usize) -> bool {
    let byte = validity[row / 8];
    let bit = 1u8 << (row % 8);
    (byte & bit) != 0
}

#[tokio::test]
async fn orscol_v2_encode_decode_matches_copy_batch_and_row_wise() {
    let url = test_db_url();

    let table = format!("orsx2_orscol_v2_parity_{}", Uuid::new_v4().simple());
    let qt = orsx::quote_identifier(&table);

    let mut conn_setup = sqlx::PgConnection::connect(&url).await.unwrap();
    conn_setup
        .execute(&*format!("DROP TABLE IF EXISTS {qt}"))
        .await
        .unwrap();
    conn_setup
        .execute(&*format!(
            r#"
            CREATE TABLE {qt} (
                id BIGINT PRIMARY KEY,
                b BOOLEAN NULL,
                i32 INTEGER NULL,
                f64 DOUBLE PRECISION NULL,
                u UUID NULL,
                ts TIMESTAMPTZ NULL,
                t TEXT NULL,
                by BYTEA NULL
            )
            "#
        ))
        .await
        .unwrap();

    let u0 = Uuid::new_v4();
    let u1 = Uuid::new_v4();

    sqlx::query(&*format!(
        r#"
        INSERT INTO {qt} (id, b, i32, f64, u, ts, t, by)
        VALUES
            (1, true,  10, 1.25, $1, '2020-01-01T00:00:00Z', 'hello', E'\\x0102'),
            (2, NULL,  NULL, NULL, NULL, NULL, NULL, NULL),
            (3, false, -7,  3.5,  $2, '2020-01-01T00:00:00.000001Z', 'x', E'\\x')
        "#
    ))
    .bind(u0)
    .bind(u1)
    .execute(&mut conn_setup)
    .await
    .unwrap();
    drop(conn_setup);

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

    let sql = format!("SELECT id, b, i32, f64, u, ts, t, by FROM {qt} ORDER BY id");

    // COPY BINARY -> ColumnarBatch
    let mut conn_copy = sqlx::PgConnection::connect(&url).await.unwrap();
    let mut reader_copy =
        CopyBinaryBatchReader::new_select_unchecked(&mut conn_copy, sql.as_str(), schema.clone())
            .await
            .unwrap();
    let mut batch_copy = ColumnarBatch::new(schema.clone(), 16).unwrap();
    let rows_copy = reader_copy.next_batch_into(&mut batch_copy).await.unwrap();
    assert_eq!(rows_copy, 3);

    // Row-wise -> ColumnarBatch
    let mut conn_row = sqlx::PgConnection::connect(&url).await.unwrap();
    let mut reader_row =
        RowWiseBatchReader::new_select_unchecked(&mut conn_row, sql.as_str(), schema.clone())
            .await
            .unwrap();
    let mut batch_row = ColumnarBatch::new(schema.clone(), 16).unwrap();
    let rows_row = reader_row.next_batch_into(&mut batch_row).await.unwrap();
    assert_eq!(rows_row, 3);

    // Encode ORSXCOL2 from the COPY batch, then decode back.
    let mut encoded = Vec::new();
    encode_orsxcol_v2_into(&batch_copy, &mut encoded).unwrap();
    let decoded = decode_orsxcol_v2(&encoded).unwrap();

    assert_eq!(decoded.row_count(), batch_copy.row_count());
    assert_eq!(*decoded.schema(), *batch_copy.schema());

    // Compare a subset of column values and varlen bytes deterministically.
    // id
    {
        let exp = (batch_copy.row_count() + 7) / 8;
        let v0 = batch_copy.column_validity_bytes(0).unwrap();
        let v1 = decoded.column_validity_bytes(0).unwrap();
        assert_eq!(&v0[..exp], v1);
        let a = batch_copy.fixed_i64(0).unwrap();
        let b = decoded.fixed_i64(0).unwrap();
        assert_eq!(a, b);
    }

    // f64 bits
    {
        let a = batch_copy.fixed_f64_bits(3).unwrap();
        let b = decoded.fixed_f64_bits(3).unwrap();
        assert_eq!(a, b);
    }

    // text
    {
        let col = 6usize;
        let exp = (batch_copy.row_count() + 7) / 8;
        let validity = batch_copy.column_validity_bytes(col).unwrap();
        let validity_dec = decoded.column_validity_bytes(col).unwrap();
        assert_eq!(&validity[..exp], validity_dec);

        let (off0, _chunks0, total0) = batch_copy.var_chunks(col).unwrap();
        let (off1, _chunks1, total1) = decoded.var_chunks(col).unwrap();
        assert_eq!(off0, off1);
        assert_eq!(total0, total1);

        let mut data0 = Vec::new();
        let mut data1 = Vec::new();
        batch_copy.coalesce_var_into(col, &mut data0).unwrap();
        decoded.coalesce_var_into(col, &mut data1).unwrap();
        assert_eq!(data0, data1);

        // sanity: row-wise and COPY agree on NULL positions for this column
        let validity_row = batch_row.column_validity_bytes(col).unwrap();
        for row in 0..batch_copy.row_count() {
            assert_eq!(validity_get(validity_row, row), validity_get(validity, row));
        }
    }
}
