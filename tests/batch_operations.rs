use orsx::prelude::*;
use sqlx::PgPool;

#[derive(OrsxMigrate, sqlx::FromRow, Debug, Clone, PartialEq, serde::Serialize)]
#[orsx_table("batch_test_records")]
struct BatchTestRecord {
    #[orsx_column(primary_key)]
    id: String,
    name: String,
    value: i64,
    price: f64,
    active: bool,
}

async fn setup_test_db() -> PgPool {
    let url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/orsx_test".to_string());

    let pool = PgPool::connect(&url)
        .await
        .expect("Failed to connect to test database");

    // Drop and recreate table
    sqlx::query("DROP TABLE IF EXISTS batch_test_records CASCADE")
        .execute(&pool)
        .await
        .expect("Failed to drop table");

    sqlx::query(&BatchTestRecord::create_table_sql())
        .execute(&pool)
        .await
        .expect("Failed to create table");

    pool
}

fn create_test_records(count: usize) -> Vec<BatchTestRecord> {
    (0..count)
        .map(|i| BatchTestRecord {
            id: format!("test_{}", i),
            name: format!("Record {}", i),
            value: i as i64,
            price: i as f64 * 1.5,
            active: i % 2 == 0,
        })
        .collect()
}

#[tokio::test]
async fn test_batch_insert_empty() {
    let pool = setup_test_db().await;
    let records: Vec<BatchTestRecord> = vec![];

    let result =
        BatchTestRecord::batch_insert_into_table(&records, &pool, "batch_test_records").await;
    assert!(result.is_ok());

    let count = BatchTestRecord::count_in_table(&pool, "batch_test_records")
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_batch_insert_single_record() {
    let pool = setup_test_db().await;
    let records = create_test_records(1);

    BatchTestRecord::batch_insert_into_table(&records, &pool, "batch_test_records")
        .await
        .expect("Failed to batch insert single record");

    let count = BatchTestRecord::count_in_table(&pool, "batch_test_records")
        .await
        .unwrap();
    assert_eq!(count, 1);

    let fetched = BatchTestRecord::fetch_all_from_table(&pool, "batch_test_records")
        .await
        .unwrap();
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0], records[0]);
}

#[tokio::test]
async fn test_batch_insert_small_batch() {
    let pool = setup_test_db().await;
    let records = create_test_records(5);

    BatchTestRecord::batch_insert_into_table(&records, &pool, "batch_test_records")
        .await
        .expect("Failed to batch insert small batch");

    let count = BatchTestRecord::count_in_table(&pool, "batch_test_records")
        .await
        .unwrap();
    assert_eq!(count, 5);

    let fetched = BatchTestRecord::fetch_all_from_table(&pool, "batch_test_records")
        .await
        .unwrap();
    assert_eq!(fetched.len(), 5);
}

#[tokio::test]
async fn test_batch_insert_medium_batch() {
    let pool = setup_test_db().await;
    let records = create_test_records(100);

    BatchTestRecord::batch_insert_into_table(&records, &pool, "batch_test_records")
        .await
        .expect("Failed to batch insert medium batch");

    let count = BatchTestRecord::count_in_table(&pool, "batch_test_records")
        .await
        .unwrap();
    assert_eq!(count, 100);
}

#[tokio::test]
async fn test_batch_insert_large_batch() {
    let pool = setup_test_db().await;
    let records = create_test_records(2000);

    BatchTestRecord::batch_insert_into_table(&records, &pool, "batch_test_records")
        .await
        .expect("Failed to batch insert large batch");

    let count = BatchTestRecord::count_in_table(&pool, "batch_test_records")
        .await
        .unwrap();
    assert_eq!(count, 2000);
}

#[tokio::test]
async fn test_batch_update_empty() {
    let pool = setup_test_db().await;
    let records: Vec<BatchTestRecord> = vec![];

    let affected = BatchTestRecord::batch_update_in_table(&records, &pool, "batch_test_records")
        .await
        .expect("Failed to batch update empty");

    assert_eq!(affected, 0);
}

#[tokio::test]
async fn test_batch_update_small_batch() {
    let pool = setup_test_db().await;
    let mut records = create_test_records(5);

    // Insert first
    BatchTestRecord::batch_insert_into_table(&records, &pool, "batch_test_records")
        .await
        .expect("Failed to insert for update test");

    // Modify records
    for record in &mut records {
        record.name = format!("Updated {}", record.name);
        record.value *= 2;
    }

    // Update
    let affected = BatchTestRecord::batch_update_in_table(&records, &pool, "batch_test_records")
        .await
        .expect("Failed to batch update");

    assert_eq!(affected, 5);

    // Verify updates
    let fetched = BatchTestRecord::fetch_all_from_table(&pool, "batch_test_records")
        .await
        .unwrap();
    for record in fetched {
        assert!(record.name.starts_with("Updated"));
        assert!(record.value % 2 == 0 || record.value == 0);
    }
}

#[tokio::test]
async fn test_batch_update_large_batch() {
    let pool = setup_test_db().await;
    let mut records = create_test_records(150);

    // Insert first
    BatchTestRecord::batch_insert_into_table(&records, &pool, "batch_test_records")
        .await
        .expect("Failed to insert for update test");

    // Modify records
    for record in &mut records {
        record.price *= 1.1;
    }

    // Update
    let affected = BatchTestRecord::batch_update_in_table(&records, &pool, "batch_test_records")
        .await
        .expect("Failed to batch update");

    assert_eq!(affected, 150);
}

#[tokio::test]
async fn test_batch_delete_empty() {
    let pool = setup_test_db().await;
    let ids: Vec<String> = vec![];

    let affected = BatchTestRecord::batch_delete_from_table(&pool, "batch_test_records", &ids)
        .await
        .expect("Failed to batch delete empty");

    assert_eq!(affected, 0);
}

#[tokio::test]
async fn test_batch_delete_small_batch() {
    let pool = setup_test_db().await;
    let records = create_test_records(10);

    // Insert first
    BatchTestRecord::batch_insert_into_table(&records, &pool, "batch_test_records")
        .await
        .expect("Failed to insert for delete test");

    // Delete first 5
    let ids_to_delete: Vec<String> = records.iter().take(5).map(|r| r.id.clone()).collect();

    let affected =
        BatchTestRecord::batch_delete_from_table(&pool, "batch_test_records", &ids_to_delete)
            .await
            .expect("Failed to batch delete");

    assert_eq!(affected, 5);

    // Verify remaining
    let count = BatchTestRecord::count_in_table(&pool, "batch_test_records")
        .await
        .unwrap();
    assert_eq!(count, 5);
}

#[tokio::test]
async fn test_batch_delete_large_batch() {
    let pool = setup_test_db().await;
    let records = create_test_records(200);

    // Insert first
    BatchTestRecord::batch_insert_into_table(&records, &pool, "batch_test_records")
        .await
        .expect("Failed to insert for delete test");

    // Delete first 150
    let ids_to_delete: Vec<String> = records.iter().take(150).map(|r| r.id.clone()).collect();

    let affected =
        BatchTestRecord::batch_delete_from_table(&pool, "batch_test_records", &ids_to_delete)
            .await
            .expect("Failed to batch delete");

    assert_eq!(affected, 150);

    // Verify remaining
    let count = BatchTestRecord::count_in_table(&pool, "batch_test_records")
        .await
        .unwrap();
    assert_eq!(count, 50);
}

#[tokio::test]
async fn test_batch_upsert() {
    let pool = setup_test_db().await;
    let mut records = create_test_records(10);

    // Initial insert
    BatchTestRecord::batch_insert_into_table(&records[0..5].to_vec(), &pool, "batch_test_records")
        .await
        .expect("Failed initial insert");

    // Modify some existing and add new
    for record in &mut records[0..5] {
        record.name = format!("Upserted {}", record.name);
    }

    // Upsert all 10 (5 updates, 5 inserts)
    let affected = BatchTestRecord::batch_upsert_into_table(
        &records,
        &pool,
        "batch_test_records",
        &["id"],
        &["name", "value", "price", "active"],
    )
    .await
    .expect("Failed to batch upsert");

    assert_eq!(affected, 10);

    // Verify all 10 exist
    let count = BatchTestRecord::count_in_table(&pool, "batch_test_records")
        .await
        .unwrap();
    assert_eq!(count, 10);

    // Verify updates
    let fetched = BatchTestRecord::fetch_all_from_table(&pool, "batch_test_records")
        .await
        .unwrap();
    let upserted_count = fetched
        .iter()
        .filter(|r| r.name.starts_with("Upserted"))
        .count();
    assert_eq!(upserted_count, 5);
}

#[tokio::test]
async fn test_data_integrity() {
    let pool = setup_test_db().await;
    let records = create_test_records(100);

    // Insert
    BatchTestRecord::batch_insert_into_table(&records, &pool, "batch_test_records")
        .await
        .expect("Failed to insert");

    // Fetch and verify each record
    let fetched = BatchTestRecord::fetch_all_from_table(&pool, "batch_test_records")
        .await
        .unwrap();

    assert_eq!(fetched.len(), records.len());

    // Sort both for comparison
    let mut sorted_original = records.clone();
    sorted_original.sort_by_key(|r| r.id.clone());

    let mut sorted_fetched = fetched;
    sorted_fetched.sort_by_key(|r| r.id.clone());

    for (original, fetched) in sorted_original.iter().zip(sorted_fetched.iter()) {
        assert_eq!(original.id, fetched.id);
        assert_eq!(original.name, fetched.name);
        assert_eq!(original.value, fetched.value);
        assert!((original.price - fetched.price).abs() < 0.001);
        assert_eq!(original.active, fetched.active);
    }
}

// Large-scale test with MATHILDE-like data volume
#[tokio::test]
async fn test_mathilde_scale() {
    let pool = setup_test_db().await;

    // Create 600+ records (simulating MATHILDE's regime data)
    let records = create_test_records(650);

    let start = std::time::Instant::now();

    BatchTestRecord::batch_insert_into_table(&records, &pool, "batch_test_records")
        .await
        .expect("Failed to insert MATHILDE-scale batch");

    let elapsed = start.elapsed();
    println!("Inserted 650 records in {:?}", elapsed);

    // Verify
    let count = BatchTestRecord::count_in_table(&pool, "batch_test_records")
        .await
        .unwrap();
    assert_eq!(count, 650);

    // Performance assertion - should be fast
    assert!(
        elapsed.as_millis() < 2000,
        "Batch insert took too long: {:?}",
        elapsed
    );
}
