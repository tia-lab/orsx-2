use orsx::columnar::{
    ColumnarBatch, ColumnarBatchReader, ColumnarField, ColumnarReaderMode, ColumnarSchema,
    ColumnarType, RowWiseBatchReader,
    RowWiseBatchReaderConfig,
};
use sqlx::{Connection, Executor};
use std::sync::atomic::{AtomicUsize, Ordering};

static TABLE_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unique_table_name() -> String {
    let n = TABLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("orscol_preflight_{n}")
}

async fn connect() -> sqlx::PgConnection {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    sqlx::PgConnection::connect(&url).await.unwrap()
}

#[tokio::test]
async fn row_wise_preflight_rejects_column_count_mismatch() {
    let mut conn = connect().await;
    let table = unique_table_name();

    conn.execute(&*format!("DROP TABLE IF EXISTS {table}"))
        .await
        .unwrap();
    conn.execute(&*format!(
        "CREATE TABLE {table} (name_ TEXT NOT NULL, pwt DOUBLE PRECISION NOT NULL)"
    ))
    .await
    .unwrap();
    conn.execute(&*format!(
        "INSERT INTO {table} (name_, pwt) VALUES ('a', 1.0)"
    ))
        .await
        .unwrap();

    let schema = ColumnarSchema::new(vec![
        ColumnarField {
            name: Some("name_".to_string()),
            ty: ColumnarType::Utf8,
        },
        ColumnarField {
            name: Some("pwt".to_string()),
            ty: ColumnarType::F64,
        },
    ])
    .unwrap();

    let select_sql = format!("SELECT name_ FROM {table}");

    // Query returns only one column.
    let mut reader = RowWiseBatchReader::new_select_unchecked(
        &mut conn,
        select_sql.as_str(),
        schema.clone(),
    )
    .await
    .unwrap()
    .with_config(RowWiseBatchReaderConfig {
        validate_column_count: true,
        validate_column_names: false,
        validate_type_compatible: false,
    });

    let mut batch = ColumnarBatch::new(schema, 16).unwrap();
    let err = reader.next_batch_into(&mut batch).await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("row-wise preflight failed"));
    assert!(msg.contains("column count mismatch"));
}

#[tokio::test]
async fn row_wise_preflight_rejects_name_mismatch() {
    let mut conn = connect().await;
    let table = unique_table_name();

    conn.execute(&*format!("DROP TABLE IF EXISTS {table}"))
        .await
        .unwrap();
    conn.execute(&*format!(
        "CREATE TABLE {table} (name_ TEXT NOT NULL, pwt DOUBLE PRECISION NOT NULL)"
    ))
    .await
    .unwrap();
    conn.execute(&*format!(
        "INSERT INTO {table} (name_, pwt) VALUES ('a', 1.0)"
    ))
        .await
        .unwrap();

    let schema = ColumnarSchema::new(vec![
        ColumnarField {
            name: Some("name_".to_string()),
            ty: ColumnarType::Utf8,
        },
        ColumnarField {
            name: Some("pwt".to_string()),
            ty: ColumnarType::F64,
        },
    ])
    .unwrap();

    let select_sql = format!("SELECT pwt, name_ FROM {table}");

    // Query returns the same two types but swapped order -> name mismatch at index 0.
    let mut reader = RowWiseBatchReader::new_select_unchecked(
        &mut conn,
        select_sql.as_str(),
        schema.clone(),
    )
    .await
    .unwrap()
    .with_config(RowWiseBatchReaderConfig {
        validate_column_count: true,
        validate_column_names: true,
        validate_type_compatible: false,
    });

    let mut batch = ColumnarBatch::new(schema, 16).unwrap();
    let err = reader.next_batch_into(&mut batch).await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("row-wise preflight failed"));
    assert!(msg.contains("column name mismatch"));
}

#[tokio::test]
async fn row_wise_preflight_passes_on_match() {
    let mut conn = connect().await;
    let table = unique_table_name();

    conn.execute(&*format!("DROP TABLE IF EXISTS {table}"))
        .await
        .unwrap();
    conn.execute(&*format!(
        "CREATE TABLE {table} (name_ TEXT NOT NULL, pwt DOUBLE PRECISION NOT NULL)"
    ))
    .await
    .unwrap();
    conn.execute(&*format!(
        "INSERT INTO {table} (name_, pwt) VALUES ('a', 1.0)"
    ))
        .await
        .unwrap();

    let schema = ColumnarSchema::new(vec![
        ColumnarField {
            name: Some("name_".to_string()),
            ty: ColumnarType::Utf8,
        },
        ColumnarField {
            name: Some("pwt".to_string()),
            ty: ColumnarType::F64,
        },
    ])
    .unwrap();

    let select_sql = format!("SELECT name_, pwt FROM {table}");

    let mut reader = RowWiseBatchReader::new_select_unchecked(
        &mut conn,
        select_sql.as_str(),
        schema.clone(),
    )
    .await
    .unwrap()
    .with_config(RowWiseBatchReaderConfig {
        validate_column_count: true,
        validate_column_names: true,
        validate_type_compatible: false,
    });

    let mut batch = ColumnarBatch::new(schema, 16).unwrap();
    let rows = reader.next_batch_into(&mut batch).await.unwrap();
    assert_eq!(rows, 1);
}

#[tokio::test]
async fn row_wise_preflight_rejects_type_mismatch() {
    let mut conn = connect().await;
    let table = unique_table_name();

    conn.execute(&*format!("DROP TABLE IF EXISTS {table}"))
        .await
        .unwrap();
    conn.execute(&*format!(
        "CREATE TABLE {table} (name_ TEXT NOT NULL, pwt DOUBLE PRECISION NOT NULL)"
    ))
    .await
    .unwrap();
    conn.execute(&*format!(
        "INSERT INTO {table} (name_, pwt) VALUES ('a', 1.0)"
    ))
    .await
    .unwrap();

    let schema = ColumnarSchema::new(vec![
        ColumnarField {
            name: Some("name_".to_string()),
            ty: ColumnarType::Utf8,
        },
        ColumnarField {
            name: Some("pwt".to_string()),
            ty: ColumnarType::F64,
        },
    ])
    .unwrap();

    // Force `pwt` to TEXT so type compatibility fails before decoding.
    let select_sql = format!("SELECT name_, pwt::text AS pwt FROM {table}");

    let mut reader = RowWiseBatchReader::new_select_unchecked(&mut conn, select_sql.as_str(), schema.clone())
        .await
        .unwrap()
        .with_config(RowWiseBatchReaderConfig {
            validate_column_count: true,
            validate_column_names: true,
            validate_type_compatible: true,
        });

    let mut batch = ColumnarBatch::new(schema, 16).unwrap();
    let err = reader.next_batch_into(&mut batch).await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("row-wise preflight failed"));
    assert!(msg.contains("type mismatch"));
}

#[tokio::test]
async fn wrapper_propagates_row_wise_preflight_config() {
    let mut conn = connect().await;
    let table = unique_table_name();

    conn.execute(&*format!("DROP TABLE IF EXISTS {table}"))
        .await
        .unwrap();
    conn.execute(&*format!(
        "CREATE TABLE {table} (name_ TEXT NOT NULL, pwt DOUBLE PRECISION NOT NULL)"
    ))
    .await
    .unwrap();
    conn.execute(&*format!(
        "INSERT INTO {table} (name_, pwt) VALUES ('a', 1.0)"
    ))
    .await
    .unwrap();

    let schema = ColumnarSchema::new(vec![
        ColumnarField {
            name: Some("name_".to_string()),
            ty: ColumnarType::Utf8,
        },
        ColumnarField {
            name: Some("pwt".to_string()),
            ty: ColumnarType::F64,
        },
    ])
    .unwrap();

    // Swapped columns should be rejected by name preflight if it is actually propagated.
    let select_sql = format!("SELECT pwt, name_ FROM {table}");

    let mut reader = ColumnarBatchReader::new_select_unchecked_with_row_wise_config(
        &mut conn,
        select_sql.as_str(),
        schema.clone(),
        ColumnarReaderMode::RowWise,
        Some(RowWiseBatchReaderConfig {
            validate_column_count: true,
            validate_column_names: true,
            validate_type_compatible: false,
        }),
    )
    .await
    .unwrap();

    let mut batch = ColumnarBatch::new(schema, 16).unwrap();
    let err = reader.next_batch_into(&mut batch).await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("row-wise preflight failed"));
    assert!(msg.contains("column name mismatch"));
}
