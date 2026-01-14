use orsx::columnar::{ColumnarBatch, ColumnarField, ColumnarSchema, ColumnarType, CopyBinaryBatchReader};
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

    // Prevent dead-code elimination in release perf runs.
    let mut checksum: u64 = 0;
    if let Some(values) = batch.fixed_values_bytes(0) {
        for &b in values.iter().take(64) {
            checksum = checksum.wrapping_add(b as u64);
        }
    }

    // Row-wise path
    let mut conn_row = sqlx::PgConnection::connect(&url).await.unwrap();
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

        for col_idx in 1..=fcols {
            let v: Option<f64> = row.try_get(col_idx).unwrap();
            if let Some(x) = v {
                sum_f64_bits = sum_f64_bits.wrapping_add(x.to_bits());
            }
        }

        let t: Option<&str> = row.try_get(fcols + 1).unwrap();
        if let Some(s) = t {
            sum_len = sum_len.wrapping_add(s.len());
        }

        let by: Option<&[u8]> = row.try_get(fcols + 2).unwrap();
        if let Some(b) = by {
            for &x in b {
                sum_bytes = sum_bytes.wrapping_add(x as u64);
            }
        }

        rows_seen += 1;
    }
    let dt_row = t1.elapsed();
    assert_eq!(rows_seen, rows);

    eprintln!(
        "orscol_perf: rows={rows} cols={cols} fcols={fcols} checksum={checksum} sum_len={sum_len} sum_i64={sum_i64} sum_f64_bits={sum_f64_bits} sum_bytes={sum_bytes} copy={:?} row_wise={:?}",
        dt_copy,
        dt_row
    );
}
