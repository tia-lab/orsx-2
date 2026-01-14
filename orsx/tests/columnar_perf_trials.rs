use orsx::columnar::{
    ColumnarBatch, ColumnarField, ColumnarSchema, ColumnarType, CopyBinaryBatchReader, FixedEncoding,
};
use futures_util::TryStreamExt;
use sqlx::{Connection, Executor};
use sqlx::Row;

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

#[tokio::test]
#[ignore]
async fn columnar_perf_trial_copy_binary_vs_row_wise() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());

    let rows = env_usize("ORSX_COL_ROWS", 100_000);
    let cols = env_usize("ORSX_COL_COLS", 50).max(3);

    let mut conn_setup = sqlx::PgConnection::connect(&url).await.unwrap();
    conn_setup
        .execute("DROP TABLE IF EXISTS orscol_perf")
        .await
        .unwrap();

    let fcols = cols.saturating_sub(3);
    let mut ddl = String::from("CREATE TABLE orscol_perf (id BIGINT PRIMARY KEY");
    for i in 1..=fcols {
        ddl.push_str(&format!(", c{:03} DOUBLE PRECISION NULL", i));
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
        select_list.push_str(&format!(", c{:03}", i));
    }
    select_list.push_str(", t, by");

    let select_sql = format!("SELECT {select_list} FROM orscol_perf ORDER BY id");

    // COPY BINARY path
    let mut conn_copy = sqlx::PgConnection::connect(&url).await.unwrap();
    let mut fields = Vec::with_capacity(cols);
    fields.push(ColumnarField {
        name: Some("id".to_string()),
        ty: ColumnarType::I64,
    });
    for i in 1..=fcols {
        fields.push(ColumnarField {
            name: Some(format!("c{:03}", i)),
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
    let mut batch = ColumnarBatch::new(schema, rows.max(1)).unwrap();

    let t0 = std::time::Instant::now();
    let got_rows = reader.next_batch_into(&mut batch).await.unwrap();
    let dt_copy = t0.elapsed();
    assert_eq!(got_rows, rows);

    // Columnar-side checksum/decoding to make the work comparable and prevent DCE.
    let mut checksum: u64 = 0;
    let mut col_sum_len = 0usize;
    let mut col_sum_i64: i64 = 0;
    let mut col_sum_f64_bits: u64 = 0;
    let mut col_sum_bytes: u64 = 0;
    {
        // id
        let validity = batch.column_validity_bytes(0).unwrap();
        let values = batch.fixed_values_bytes(0).unwrap();
        let enc = batch.fixed_encoding(0).unwrap();
        for row in 0..rows {
            if (validity[row / 8] & (1u8 << (row % 8))) == 0 {
                continue;
            }
            let start = row * 8;
            let slice = &values[start..start + 8];
            let v = match enc {
                FixedEncoding::Le => i64::from_le_bytes(slice.try_into().unwrap()),
                FixedEncoding::PgBe => i64::from_be_bytes(slice.try_into().unwrap()),
            };
            col_sum_i64 = col_sum_i64.wrapping_add(v);
        }

        // f64 columns (1..=fcols)
        for col in 1..=fcols {
            let validity = batch.column_validity_bytes(col).unwrap();
            let values = batch.fixed_values_bytes(col).unwrap();
            let enc = batch.fixed_encoding(col).unwrap();
            for row in 0..rows {
                if (validity[row / 8] & (1u8 << (row % 8))) == 0 {
                    continue;
                }
                let start = row * 8;
                let slice = &values[start..start + 8];
                let bits = match enc {
                    FixedEncoding::Le => u64::from_le_bytes(slice.try_into().unwrap()),
                    FixedEncoding::PgBe => u64::from_be_bytes(slice.try_into().unwrap()),
                };
                col_sum_f64_bits = col_sum_f64_bits.wrapping_add(bits);
            }
        }

        // t
        {
            let col_idx = fcols + 1;
            let validity = batch.column_validity_bytes(col_idx).unwrap();
            let (offsets, _data) = batch.var_slices(col_idx).unwrap();
            for row in 0..rows {
                if (validity[row / 8] & (1u8 << (row % 8))) == 0 {
                    continue;
                }
                let start = offsets[row] as usize;
                let end = offsets[row + 1] as usize;
                col_sum_len = col_sum_len.wrapping_add(end - start);
            }
        }

        // by
        {
            let col_idx = fcols + 2;
            let validity = batch.column_validity_bytes(col_idx).unwrap();
            let (offsets, data) = batch.var_slices(col_idx).unwrap();
            for row in 0..rows {
                if (validity[row / 8] & (1u8 << (row % 8))) == 0 {
                    continue;
                }
                let start = offsets[row] as usize;
                let end = offsets[row + 1] as usize;
                for &x in &data[start..end] {
                    col_sum_bytes = col_sum_bytes.wrapping_add(x as u64);
                }
            }
        }

        // Cheap checksum (first 64 bytes of the id column buffer).
        for &b in values.iter().take(64) {
            checksum = checksum.wrapping_add(b as u64);
        }
    }

    // Row-wise path
    let mut conn_row = sqlx::PgConnection::connect(&url).await.unwrap();
    let mut batch_row = ColumnarBatch::new(batch.schema().clone(), rows.max(1)).unwrap();
    let t1 = std::time::Instant::now();
    let mut stream = sqlx::query(select_sql.as_str()).fetch(&mut conn_row);
    let mut rows_seen = 0usize;
    let mut sum_len = 0usize;
    let mut sum_i64: i64 = 0;
    let mut sum_f64_bits: u64 = 0;
    let mut sum_bytes: u64 = 0;
    while let Some(row) = stream.try_next().await.unwrap() {
        let id: i64 = row.try_get(0).unwrap();
        sum_i64 = sum_i64.wrapping_add(id);
        batch_row
            .push_fixed_bytes(0, &id.to_be_bytes(), FixedEncoding::PgBe)
            .unwrap();

        for col_idx in 1..=fcols {
            let v: Option<f64> = row.try_get(col_idx).unwrap();
            if let Some(x) = v {
                sum_f64_bits = sum_f64_bits.wrapping_add(x.to_bits());
                batch_row
                    .push_fixed_bytes(
                        col_idx,
                        &x.to_bits().to_be_bytes(),
                        FixedEncoding::PgBe,
                    )
                    .unwrap();
            } else {
                batch_row.push_null(col_idx).unwrap();
            }
        }

        let t: Option<&str> = row.try_get(fcols + 1).unwrap();
        if let Some(s) = t {
            sum_len = sum_len.wrapping_add(s.len());
            batch_row.push_utf8(fcols + 1, s).unwrap();
        } else {
            batch_row.push_null(fcols + 1).unwrap();
        }

        let by: Option<&[u8]> = row.try_get(fcols + 2).unwrap();
        if let Some(b) = by {
            for &x in b {
                sum_bytes = sum_bytes.wrapping_add(x as u64);
            }
            batch_row.push_var_bytes(fcols + 2, b).unwrap();
        } else {
            batch_row.push_null(fcols + 2).unwrap();
        }

        batch_row.end_row().unwrap();

        rows_seen += 1;
    }
    let dt_row = t1.elapsed();
    assert_eq!(rows_seen, rows);

    eprintln!(
        "orscol_perf: rows={rows} cols={cols} fcols={fcols} checksum={checksum} copy={:?} row_wise_build={:?} | col_sum_len={col_sum_len} col_sum_i64={col_sum_i64} col_sum_f64_bits={col_sum_f64_bits} col_sum_bytes={col_sum_bytes} | row_sum_len={sum_len} row_sum_i64={sum_i64} row_sum_f64_bits={sum_f64_bits} row_sum_bytes={sum_bytes}",
        dt_copy, dt_row
    );
}
