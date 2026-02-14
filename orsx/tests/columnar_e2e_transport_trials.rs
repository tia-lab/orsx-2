use futures_util::TryStreamExt;
use orsx::columnar::{
    decode_orsxcol_v1, decode_orsxcol_v2, encode_orsxcol_v1_into, encode_orsxcol_v2_into,
    ColumnarBatch, ColumnarField, ColumnarSchema, ColumnarType, CopyBinaryBatchReader,
};
use sqlx::{Connection, Executor, Row};

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

fn require_test_db_url() -> String {
    let url = std::env::var("ORSX_TEST_DATABASE_URL").expect(
        "ORSX_TEST_DATABASE_URL must be set (this test refuses to run with a hard-coded default)",
    );
    if url.contains(":1364") {
        panic!("refusing to run: ORSX_TEST_DATABASE_URL points at port 1364 (production)");
    }
    url
}

fn checksum_batch(batch: &ColumnarBatch) -> u64 {
    let mut acc: u64 = 0;
    for (col_idx, f) in batch.schema().fields().iter().enumerate() {
        if let Some(validity) = batch.column_validity_bytes(col_idx) {
            for &b in validity {
                acc = acc.wrapping_add(b as u64);
                acc = acc.rotate_left(1);
            }
        }
        match f.ty {
            ColumnarType::I64 => {
                if let Some(values) = batch.fixed_i64(col_idx) {
                    for &v in values {
                        acc = acc.wrapping_add(v as u64);
                        acc = acc.rotate_left(1);
                    }
                }
            }
            ColumnarType::F64 => {
                if let Some(values) = batch.fixed_f64_bits(col_idx) {
                    for &bits in values {
                        acc ^= bits;
                        acc = acc.rotate_left(1);
                    }
                }
            }
            ColumnarType::Utf8 | ColumnarType::Bytes | ColumnarType::JsonbText => {
                if let Some((offsets, chunks, total_len)) = batch.var_chunks(col_idx) {
                    for &o in offsets {
                        acc = acc.wrapping_add(o as u64);
                        acc = acc.rotate_left(1);
                    }
                    if let Some(inline) = batch.var_inline_bytes(col_idx) {
                        for &b in inline {
                            acc = acc.wrapping_add(b as u64);
                            acc = acc.rotate_left(1);
                        }
                    }
                    for c in chunks {
                        for &b in c.as_ref() {
                            acc = acc.wrapping_add(b as u64);
                            acc = acc.rotate_left(1);
                        }
                    }
                    acc = acc.wrapping_add(total_len as u64);
                    acc = acc.rotate_left(1);
                }
            }
            _ => {}
        }
    }
    acc
}

#[tokio::test]
#[ignore]
async fn columnar_e2e_trial_copy_vs_rowwise_plus_transport_v1_vs_v2() {
    let url = require_test_db_url();

    let rows = env_usize("ORSX_COL_ROWS", 100_000);
    let cols = env_usize("ORSX_COL_COLS", 50).max(3);
    let fcols = cols.saturating_sub(3);

    let mut conn_setup = sqlx::PgConnection::connect(&url).await.unwrap();
    conn_setup
        .execute("DROP TABLE IF EXISTS orscol_perf")
        .await
        .unwrap();

    let mut ddl = String::from("CREATE TABLE orscol_perf (id BIGINT PRIMARY KEY");
    for i in 1..=fcols {
        ddl.push_str(&format!(", c{i:03} DOUBLE PRECISION NULL"));
    }
    ddl.push_str(", t TEXT NULL, by BYTEA NULL)");
    conn_setup.execute(ddl.as_str()).await.unwrap();

    let mut insert_sql = String::from("INSERT INTO orscol_perf SELECT gs");
    for i in 1..=fcols {
        insert_sql.push_str(&format!(
            ", CASE WHEN gs % 10 = 0 THEN NULL ELSE (gs::double precision * 0.001 + {i}.0) END"
        ));
    }
    insert_sql.push_str(", CASE WHEN gs % 10 = 0 THEN NULL ELSE 'hello' END");
    insert_sql.push_str(", CASE WHEN gs % 10 = 0 THEN NULL ELSE E'\\\\x010203'::bytea END");
    insert_sql.push_str(" FROM generate_series(1, $1) gs");

    sqlx::query(insert_sql.as_str())
        .bind(rows as i64)
        .execute(&mut conn_setup)
        .await
        .unwrap();
    drop(conn_setup);

    let mut select_list = String::from("id");
    for i in 1..=fcols {
        select_list.push_str(&format!(", c{i:03}"));
    }
    select_list.push_str(", t, by");
    let select_sql = format!("SELECT {select_list} FROM orscol_perf ORDER BY id");

    // COPY → ColumnarBatch
    let mut conn_copy = sqlx::PgConnection::connect(&url).await.unwrap();
    let mut fields = Vec::with_capacity(cols);
    fields.push(ColumnarField {
        name: Some("id".to_string()),
        ty: ColumnarType::I64,
    });
    for i in 1..=fcols {
        fields.push(ColumnarField {
            name: Some(format!("c{i:03}")),
            ty: ColumnarType::F64,
        });
    }
    fields.push(ColumnarField {
        name: Some("t".to_string()),
        ty: ColumnarType::Utf8,
    });
    fields.push(ColumnarField {
        name: Some("by".to_string()),
        ty: ColumnarType::Bytes,
    });
    let schema = ColumnarSchema::new(fields).unwrap();

    let mut reader =
        CopyBinaryBatchReader::new_select_unchecked(&mut conn_copy, &select_sql, schema.clone())
            .await
            .unwrap();
    let mut batch_copy = ColumnarBatch::new(schema.clone(), rows.max(1)).unwrap();

    let t0 = std::time::Instant::now();
    let got_rows = reader.next_batch_into(&mut batch_copy).await.unwrap();
    let dt_copy_build = t0.elapsed();
    assert_eq!(got_rows, rows);
    let checksum_copy = checksum_batch(&batch_copy);

    // Row-wise → ColumnarBatch
    let mut conn_row = sqlx::PgConnection::connect(&url).await.unwrap();
    let mut batch_row = ColumnarBatch::new(schema, rows.max(1)).unwrap();
    let t1 = std::time::Instant::now();
    let mut stream = sqlx::query(select_sql.as_str()).fetch(&mut conn_row);
    let mut rows_seen = 0usize;
    while let Some(row) = stream.try_next().await.unwrap() {
        let id: i64 = row.try_get(0).unwrap();
        batch_row.push_i64(0, id).unwrap();

        for col_idx in 1..=fcols {
            let v: Option<f64> = row.try_get(col_idx).unwrap();
            if let Some(x) = v {
                batch_row.push_f64_bits(col_idx, x.to_bits()).unwrap();
            } else {
                batch_row.push_null(col_idx).unwrap();
            }
        }

        let t: Option<&str> = row.try_get(fcols + 1).unwrap();
        if let Some(s) = t {
            batch_row.push_utf8(fcols + 1, s).unwrap();
        } else {
            batch_row.push_null(fcols + 1).unwrap();
        }

        let by: Option<&[u8]> = row.try_get(fcols + 2).unwrap();
        if let Some(b) = by {
            batch_row.push_var_bytes(fcols + 2, b).unwrap();
        } else {
            batch_row.push_null(fcols + 2).unwrap();
        }

        batch_row.end_row().unwrap();
        rows_seen += 1;
    }
    let dt_row_build = t1.elapsed();
    assert_eq!(rows_seen, rows);
    let checksum_row = checksum_batch(&batch_row);
    assert_eq!(checksum_copy, checksum_row);

    // Transport (encode/decode) on the COPY-built batch.
    let mut bytes_v1 = Vec::<u8>::new();
    let mut bytes_v2 = Vec::<u8>::new();

    let t2 = std::time::Instant::now();
    encode_orsxcol_v1_into(&batch_copy, &mut bytes_v1).unwrap();
    let dt_v1_encode = t2.elapsed();

    let t3 = std::time::Instant::now();
    encode_orsxcol_v2_into(&batch_copy, &mut bytes_v2).unwrap();
    let dt_v2_encode = t3.elapsed();

    let t4 = std::time::Instant::now();
    let decoded_v1 = decode_orsxcol_v1(&bytes_v1).unwrap();
    let dt_v1_decode = t4.elapsed();

    let t5 = std::time::Instant::now();
    let decoded_v2 = decode_orsxcol_v2(&bytes_v2).unwrap();
    let dt_v2_decode = t5.elapsed();

    assert_eq!(checksum_batch(&decoded_v1), checksum_copy);
    assert_eq!(checksum_batch(&decoded_v2), checksum_copy);

    eprintln!(
        "orscol_e2e: rows={rows} cols={cols} fcols={fcols} | copy_build={:?} row_build={:?} | v1_encode={:?} v2_encode={:?} v1_decode={:?} v2_decode={:?} | bytes_v1_len={} bytes_v2_len={} | checksum={checksum_copy}",
        dt_copy_build,
        dt_row_build,
        dt_v1_encode,
        dt_v2_encode,
        dt_v1_decode,
        dt_v2_decode,
        bytes_v1.len(),
        bytes_v2.len(),
    );
}
