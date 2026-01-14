use orsx::prelude::*;
use crate::integration::{setup_test_db, cleanup_all_tables, create_test_table};

#[derive(OrsxMigrate, sqlx::FromRow, Debug, Clone)]
struct TestRecord {
    #[orsx_column(primary_key)]
    id: String,
    name: String,
    value: i64,
    price: f64,
    active: bool,
    description: Option<String>,
}

#[tokio::test]
async fn test_insert_basic_record() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;
    create_test_table::<TestRecord>(&pool, Some("test_records")).await?;

    let record = TestRecord {
        id: "rec_1".to_string(),
        name: "Test".to_string(),
        value: 42,
        price: 99.99,
        active: true,
        description: Some("Test description".to_string()),
    };

    // Use insert_into_table()
    record.insert_into_table(&pool, "test_records").await?;

    // Verify
    let retrieved: TestRecord = sqlx::query_as("SELECT * FROM test_records WHERE id = $1")
        .bind("rec_1")
        .fetch_one(&pool)
        .await?;

    assert_eq!(retrieved.id, "rec_1");
    assert_eq!(retrieved.name, "Test");
    assert_eq!(retrieved.value, 42);
    assert_eq!(retrieved.price, 99.99);
    assert_eq!(retrieved.active, true);
    assert_eq!(retrieved.description, Some("Test description".to_string()));

    cleanup_all_tables(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn test_insert_with_null_fields() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;
    create_test_table::<TestRecord>(&pool, Some("test_records")).await?;

    let record = TestRecord {
        id: "rec_2".to_string(),
        name: "Test".to_string(),
        value: 100,
        price: 50.0,
        active: false,
        description: None, // NULL
    };

    record.insert_into_table(&pool, "test_records").await?;

    let retrieved: TestRecord = sqlx::query_as("SELECT * FROM test_records WHERE id = $1")
        .bind("rec_2")
        .fetch_one(&pool)
        .await?;

    assert_eq!(retrieved.description, None);

    cleanup_all_tables(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn test_insert_multiple_records() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;
    create_test_table::<TestRecord>(&pool, Some("test_records")).await?;

    // Insert 100 records
    for i in 0..100 {
        let record = TestRecord {
            id: format!("rec_{}", i),
            name: format!("Name {}", i),
            value: i,
            price: i as f64 * 1.5,
            active: i % 2 == 0,
            description: if i % 3 == 0 {
                Some(format!("Desc {}", i))
            } else {
                None
            },
        };

        record.insert_into_table(&pool, "test_records").await?;
    }

    // Verify count
    let count = TestRecord::count_in_table(&pool, "test_records").await?;
    assert_eq!(count, 100);

    cleanup_all_tables(&pool).await?;
    Ok(())
}
