use orsx::prelude::*;
use orsx::indexes::{IndexInfo, IndexType, create_index, drop_index, introspect_indexes, ensure_indexes};
use crate::integration::{setup_test_db, cleanup_all_tables, create_test_table};

#[derive(OrsxMigrate, sqlx::FromRow, Debug, Clone, serde::Serialize)]
#[orsx_table("test_indexed")]
struct TestIndexed {
    #[orsx_column(primary_key)]
    id: String,
    email: String,
    username: String,
    age: i32,
}

#[tokio::test]
async fn test_create_btree_index() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;
    create_test_table::<TestIndexed>(&pool, None).await?;

    let index = IndexInfo {
        name: "idx_test_email".to_string(),
        columns: vec!["email".to_string()],
        unique: false,
        index_type: IndexType::BTree,
    };

    create_index(&pool, &index, "test_indexed").await?;

    // Verify index exists
    let indexes = introspect_indexes(&pool, "test_indexed").await?;
    let found = indexes.iter().any(|i| i.name == "idx_test_email");

    assert!(found, "Index should exist after creation");

    cleanup_all_tables(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn test_create_unique_index() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;
    create_test_table::<TestIndexed>(&pool, None).await?;

    let index = IndexInfo {
        name: "idx_test_username_unique".to_string(),
        columns: vec!["username".to_string()],
        unique: true,
        index_type: IndexType::BTree,
    };

    create_index(&pool, &index, "test_indexed").await?;

    // Verify index exists and is unique
    let indexes = introspect_indexes(&pool, "test_indexed").await?;
    let found = indexes.iter().find(|i| i.name == "idx_test_username_unique");

    assert!(found.is_some(), "Index should exist");
    assert!(found.unwrap().unique, "Index should be unique");

    // Test unique constraint enforcement
    let rec1 = TestIndexed {
        id: "user1".to_string(),
        email: "user1@test.com".to_string(),
        username: "john".to_string(),
        age: 30,
    };
    rec1.insert_into_table(&pool, "test_indexed").await?;

    let rec2 = TestIndexed {
        id: "user2".to_string(),
        email: "user2@test.com".to_string(),
        username: "john".to_string(), // Duplicate username
        age: 25,
    };

    let result = rec2.insert_into_table(&pool, "test_indexed").await;
    assert!(result.is_err(), "Should fail due to unique constraint");

    cleanup_all_tables(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn test_composite_index() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;
    create_test_table::<TestIndexed>(&pool, None).await?;

    let index = IndexInfo {
        name: "idx_test_email_age".to_string(),
        columns: vec!["email".to_string(), "age".to_string()],
        unique: false,
        index_type: IndexType::BTree,
    };

    create_index(&pool, &index, "test_indexed").await?;

    // Verify composite index
    let indexes = introspect_indexes(&pool, "test_indexed").await?;
    let found = indexes.iter().find(|i| i.name == "idx_test_email_age");

    assert!(found.is_some(), "Composite index should exist");
    assert_eq!(found.unwrap().columns.len(), 2, "Should have 2 columns");
    assert!(found.unwrap().columns.contains(&"email".to_string()));
    assert!(found.unwrap().columns.contains(&"age".to_string()));

    cleanup_all_tables(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn test_drop_index() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;
    create_test_table::<TestIndexed>(&pool, None).await?;

    // Create index
    let index = IndexInfo {
        name: "idx_test_to_drop".to_string(),
        columns: vec!["email".to_string()],
        unique: false,
        index_type: IndexType::BTree,
    };

    create_index(&pool, &index, "test_indexed").await?;

    // Verify created
    let indexes_before = introspect_indexes(&pool, "test_indexed").await?;
    assert!(indexes_before.iter().any(|i| i.name == "idx_test_to_drop"));

    // Drop index
    drop_index(&pool, &index).await?;

    // Verify dropped
    let indexes_after = introspect_indexes(&pool, "test_indexed").await?;
    assert!(!indexes_after.iter().any(|i| i.name == "idx_test_to_drop"));

    cleanup_all_tables(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn test_ensure_indexes() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;
    create_test_table::<TestIndexed>(&pool, None).await?;

    // Define expected indexes
    let expected_indexes = vec![
        IndexInfo {
            name: "idx_email".to_string(),
            columns: vec!["email".to_string()],
            unique: false,
            index_type: IndexType::BTree,
        },
        IndexInfo {
            name: "idx_username".to_string(),
            columns: vec!["username".to_string()],
            unique: true,
            index_type: IndexType::BTree,
        },
    ];

    // Ensure indexes (should create both)
    let changes = ensure_indexes(&pool, "test_indexed", &expected_indexes).await?;
    assert_eq!(changes.len(), 2, "Should create 2 indexes");

    // Run again (should create nothing)
    let changes2 = ensure_indexes(&pool, "test_indexed", &expected_indexes).await?;
    assert_eq!(changes2.len(), 0, "Should create no new indexes");

    // Verify all indexes exist
    let indexes = introspect_indexes(&pool, "test_indexed").await?;
    assert!(indexes.iter().any(|i| i.name == "idx_email"));
    assert!(indexes.iter().any(|i| i.name == "idx_username"));

    cleanup_all_tables(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn test_introspect_indexes() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;
    create_test_table::<TestIndexed>(&pool, None).await?;

    // Initially only primary key index exists
    let indexes_initial = introspect_indexes(&pool, "test_indexed").await?;
    assert!(indexes_initial.len() >= 1, "Should have at least primary key index");

    // Create additional indexes
    let idx1 = IndexInfo {
        name: "idx_a".to_string(),
        columns: vec!["email".to_string()],
        unique: false,
        index_type: IndexType::BTree,
    };

    let idx2 = IndexInfo {
        name: "idx_b".to_string(),
        columns: vec!["username".to_string()],
        unique: true,
        index_type: IndexType::BTree,
    };

    create_index(&pool, &idx1, "test_indexed").await?;
    create_index(&pool, &idx2, "test_indexed").await?;

    // Introspect again
    let indexes_after = introspect_indexes(&pool, "test_indexed").await?;
    assert!(indexes_after.iter().any(|i| i.name == "idx_a"));
    assert!(indexes_after.iter().any(|i| i.name == "idx_b"));

    cleanup_all_tables(&pool).await?;
    Ok(())
}
