use orsx::prelude::*;
use crate::integration::{setup_test_db, cleanup_all_tables, create_test_table};

#[derive(OrsxMigrate, sqlx::FromRow, Debug, Clone, serde::Serialize)]
#[orsx_table("test_crud")]
struct TestRecord {
    #[orsx_column(primary_key)]
    id: String,
    name: String,
    value: i64,
}

#[tokio::test]
async fn test_update_in_table() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;
    create_test_table::<TestRecord>(&pool, None).await?;

    // Insert initial record
    let mut record = TestRecord {
        id: "update_test".to_string(),
        name: "Original".to_string(),
        value: 100,
    };
    record.insert_into_table(&pool, TestRecord::table_name()).await?;

    // Update the record
    record.name = "Updated".to_string();
    record.value = 200;
    let affected = record.update_in_table(&pool, TestRecord::table_name()).await?;

    assert_eq!(affected, 1, "Should update 1 row");

    // Verify update
    let retrieved = TestRecord::find_by_id_in_table(&pool, TestRecord::table_name(), "update_test")
        .await?
        .expect("Record should exist");

    assert_eq!(retrieved.name, "Updated");
    assert_eq!(retrieved.value, 200);

    cleanup_all_tables(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn test_delete_from_table() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;
    create_test_table::<TestRecord>(&pool, None).await?;

    // Insert record
    let record = TestRecord {
        id: "delete_test".to_string(),
        name: "ToDelete".to_string(),
        value: 42,
    };
    record.insert_into_table(&pool, TestRecord::table_name()).await?;

    // Verify exists
    let count_before = TestRecord::count_in_table(&pool, TestRecord::table_name()).await?;
    assert_eq!(count_before, 1);

    // Delete
    let deleted = TestRecord::delete_from_table(&pool, TestRecord::table_name(), "delete_test").await?;
    assert_eq!(deleted, 1, "Should delete 1 row");

    // Verify deleted
    let count_after = TestRecord::count_in_table(&pool, TestRecord::table_name()).await?;
    assert_eq!(count_after, 0);

    let not_found = TestRecord::find_by_id_in_table(&pool, TestRecord::table_name(), "delete_test").await?;
    assert!(not_found.is_none(), "Record should not exist after deletion");

    cleanup_all_tables(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn test_find_by_id_in_table() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;
    create_test_table::<TestRecord>(&pool, None).await?;

    // Insert records
    for i in 0..5 {
        let record = TestRecord {
            id: format!("rec_{}", i),
            name: format!("Name {}", i),
            value: i,
        };
        record.insert_into_table(&pool, TestRecord::table_name()).await?;
    }

    // Find specific record
    let found = TestRecord::find_by_id_in_table(&pool, TestRecord::table_name(), "rec_2")
        .await?
        .expect("Record should exist");

    assert_eq!(found.id, "rec_2");
    assert_eq!(found.name, "Name 2");
    assert_eq!(found.value, 2);

    // Find non-existent record
    let not_found = TestRecord::find_by_id_in_table(&pool, TestRecord::table_name(), "nonexistent").await?;
    assert!(not_found.is_none());

    cleanup_all_tables(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn test_full_crud_cycle() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;
    create_test_table::<TestRecord>(&pool, None).await?;

    // CREATE
    let record = TestRecord {
        id: "cycle_test".to_string(),
        name: "Test".to_string(),
        value: 100,
    };
    record.insert_into_table(&pool, TestRecord::table_name()).await?;

    // READ
    let mut found = TestRecord::find_by_id_in_table(&pool, TestRecord::table_name(), "cycle_test")
        .await?
        .expect("Record should exist");

    assert_eq!(found.value, 100);

    // UPDATE
    found.value = 200;
    found.update_in_table(&pool, TestRecord::table_name()).await?;

    let updated = TestRecord::find_by_id_in_table(&pool, TestRecord::table_name(), "cycle_test")
        .await?
        .expect("Record should exist");

    assert_eq!(updated.value, 200);

    // DELETE
    TestRecord::delete_from_table(&pool, TestRecord::table_name(), "cycle_test").await?;

    let deleted = TestRecord::find_by_id_in_table(&pool, TestRecord::table_name(), "cycle_test").await?;
    assert!(deleted.is_none());

    cleanup_all_tables(&pool).await?;
    Ok(())
}
