use orsx::columnar::{ColumnarBatch, ColumnarField, ColumnarSchema, ColumnarType, CopyBinaryBatchReader, RowWiseBatchReader};
use sqlx::{Connection, Executor};
use uuid::Uuid;

fn validity_get(validity: &[u8], row: usize) -> bool {
    let byte = validity[row / 8];
    let bit = 1u8 << (row % 8);
    (byte & bit) != 0
}

#[tokio::test]
async fn columnar_jsonb_text_matches_copy_and_row_wise() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());

    let table = format!("orscol_jsonb_{}", Uuid::new_v4().simple());
    let mut conn_setup = sqlx::PgConnection::connect(&url).await.unwrap();

    conn_setup
        .execute(&*format!("DROP TABLE IF EXISTS {table}"))
        .await
        .unwrap();
    conn_setup
        .execute(&*format!(
            "CREATE TABLE {table} (id BIGINT PRIMARY KEY, j JSONB NULL)"
        ))
        .await
        .unwrap();

    // Insert JSONB values (including NULL).
    conn_setup
        .execute(&*format!(
            "INSERT INTO {table} (id, j) VALUES \
             (1, '{{\"a\":1,\"b\":2}}'::jsonb), \
             (2, NULL), \
             (3, '[1,2,3]'::jsonb)"
        ))
        .await
        .unwrap();

    drop(conn_setup);

    let mut conn_copy = sqlx::PgConnection::connect(&url).await.unwrap();
    let mut conn_row = sqlx::PgConnection::connect(&url).await.unwrap();
    let mut conn_expected = sqlx::PgConnection::connect(&url).await.unwrap();

    let schema = ColumnarSchema::new(vec![
        ColumnarField {
            name: Some("id".to_string()),
            ty: ColumnarType::I64,
        },
        ColumnarField {
            name: Some("j".to_string()),
            ty: ColumnarType::JsonbText,
        },
    ])
    .unwrap();

    let sql = format!("SELECT id, j FROM {table} ORDER BY id");

    let mut reader_copy =
        CopyBinaryBatchReader::new_select_unchecked(&mut conn_copy, sql.as_str(), schema.clone())
            .await
            .unwrap();
    let mut batch_copy = ColumnarBatch::new(schema.clone(), 16).unwrap();
    let rows_copy = reader_copy.next_batch_into(&mut batch_copy).await.unwrap();
    assert_eq!(rows_copy, 3);

    let mut reader_row =
        RowWiseBatchReader::new_select_unchecked(&mut conn_row, sql.as_str(), schema.clone())
            .await
            .unwrap();
    let mut batch_row = ColumnarBatch::new(schema.clone(), 16).unwrap();
    let rows_row = reader_row.next_batch_into(&mut batch_row).await.unwrap();
    assert_eq!(rows_row, 3);

    let expected: Vec<(i64, Option<String>)> = sqlx::query_as(&*format!(
        "SELECT id, j::text FROM {table} ORDER BY id"
    ))
    .fetch_all(&mut conn_expected)
    .await
    .unwrap();

    // Compare JSON text var column between COPY and row-wise, and against `j::text`.
    let col_idx = 1usize;
    let validity_copy = batch_copy.column_validity_bytes(col_idx).unwrap();
    let validity_row = batch_row.column_validity_bytes(col_idx).unwrap();

    let (offsets_copy, _, _) = batch_copy.var_chunks(col_idx).unwrap();
    let (offsets_row, _, _) = batch_row.var_chunks(col_idx).unwrap();

    let mut data_copy = Vec::new();
    let mut data_row = Vec::new();
    batch_copy.coalesce_var_into(col_idx, &mut data_copy).unwrap();
    batch_row.coalesce_var_into(col_idx, &mut data_row).unwrap();

    for (i, (_id, jtxt)) in expected.iter().enumerate() {
        let is_valid = validity_get(validity_copy, i);
        let is_valid_row = validity_get(validity_row, i);
        assert_eq!(is_valid, jtxt.is_some());
        assert_eq!(is_valid_row, jtxt.is_some());

        if let Some(jtxt) = jtxt {
            let s0 = offsets_copy[i] as usize;
            let e0 = offsets_copy[i + 1] as usize;
            let s1 = offsets_row[i] as usize;
            let e1 = offsets_row[i + 1] as usize;

            let got_copy = std::str::from_utf8(&data_copy[s0..e0]).unwrap();
            let got_row = std::str::from_utf8(&data_row[s1..e1]).unwrap();
            assert_eq!(got_copy, jtxt);
            assert_eq!(got_row, jtxt);
            assert_eq!(got_copy, got_row);
        }
    }
}
