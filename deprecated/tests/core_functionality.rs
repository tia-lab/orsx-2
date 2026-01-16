// Comprehensive test suite for orso-postgres-v2 core functionality
use orsx::migrations::{comparison::compare_schemas, introspection::ColumnInfo};
use orsx::prelude::*;
use orsx::types::FieldType;

// Test struct with various field types
#[derive(OrsxMigrate, sqlx::FromRow, Debug, Clone)]
#[orsx_table("test_users")]
struct User {
    #[orsx_column(primary_key)]
    id: String,
    name: String,
    age: i32,
}

#[derive(OrsxMigrate, sqlx::FromRow, Debug, Clone)]
#[orsx_table("test_posts")]
struct Post {
    #[orsx_column(primary_key)]
    id: String,
    title: String,
    content: Option<String>,
    views: i64,
}

// Test struct with compressed fields
#[derive(OrsxMigrate, sqlx::FromRow, Debug, Clone)]
#[orsx_table("test_regime_data")]
struct RegimeData {
    #[orsx_column(primary_key)]
    id: String,
    pair: String,
    prices: Compressed<f64>,
    volumes: Compressed<i64>,
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_table_name() {
        assert_eq!(User::table_name(), "test_users");
        assert_eq!(Post::table_name(), "test_posts");
        assert_eq!(RegimeData::table_name(), "test_regime_data");
    }

    #[test]
    fn test_field_names() {
        let fields = User::field_names();
        assert_eq!(fields, vec!["id", "name", "age"]);

        let post_fields = Post::field_names();
        assert_eq!(post_fields, vec!["id", "title", "content", "views"]);

        let regime_fields = RegimeData::field_names();
        assert_eq!(regime_fields, vec!["id", "pair", "prices", "volumes"]);
    }

    #[test]
    fn test_primary_key() {
        assert_eq!(User::primary_key(), Some("id"));
        assert_eq!(Post::primary_key(), Some("id"));
        assert_eq!(RegimeData::primary_key(), Some("id"));
    }

    #[test]
    fn test_create_table_sql_contains_essential_elements() {
        let sql = User::create_table_sql();
        assert!(
            sql.contains("CREATE TABLE"),
            "SQL should contain CREATE TABLE"
        );
        assert!(sql.contains("test_users"), "SQL should contain table name");
        assert!(sql.contains("id"), "SQL should contain id field");
        assert!(sql.contains("name"), "SQL should contain name field");
        assert!(sql.contains("age"), "SQL should contain age field");
    }

    #[test]
    fn test_field_types() {
        let types = User::field_types();
        assert_eq!(types.len(), 3);
        assert!(matches!(types[0], FieldType::Text));
        assert!(matches!(types[1], FieldType::Text));
        assert!(matches!(types[2], FieldType::Integer));

        let regime_types = RegimeData::field_types();
        assert_eq!(regime_types.len(), 4);
        assert!(matches!(regime_types[0], FieldType::Text)); // id
        assert!(matches!(regime_types[1], FieldType::Text)); // pair
        assert!(matches!(regime_types[2], FieldType::Bytea)); // prices (compressed)
        assert!(matches!(regime_types[3], FieldType::Bytea)); // volumes (compressed)
    }

    #[test]
    fn test_field_nullable() {
        let nullable = User::field_nullable();
        assert_eq!(nullable, vec![false, false, false]);

        let post_nullable = Post::field_nullable();
        assert_eq!(post_nullable, vec![false, false, true, false]);

        let regime_nullable = RegimeData::field_nullable();
        assert_eq!(regime_nullable, vec![false, false, false, false]);
    }

    #[test]
    fn test_compressed_wrapper_creation() {
        let prices = vec![100.0, 101.5, 102.0, 103.5];
        let compressed = Compressed::new(prices.clone());
        assert_eq!(compressed.as_slice(), &prices[..]);
        assert_eq!(compressed.into_inner(), prices);
    }

    #[test]
    fn test_compressed_from_vec() {
        let prices = vec![100.0, 101.5, 102.0];
        let compressed: Compressed<f64> = prices.clone().into();
        assert_eq!(compressed.as_slice(), &prices[..]);
    }

    #[test]
    fn test_column_order_detection() {
        // Current schema: id, name, age
        let current = vec![
            ColumnInfo {
                name: "id".to_string(),
                sql_type: "TEXT".to_string(),
                nullable: false,
                position: 0,
                is_unique: true,
                is_primary_key: true,
                foreign_key_reference: None,
                has_default: true,
                is_compressed: false,
            },
            ColumnInfo {
                name: "name".to_string(),
                sql_type: "TEXT".to_string(),
                nullable: false,
                position: 1,
                is_unique: false,
                is_primary_key: false,
                foreign_key_reference: None,
                has_default: false,
                is_compressed: false,
            },
            ColumnInfo {
                name: "age".to_string(),
                sql_type: "INTEGER".to_string(),
                nullable: false,
                position: 2,
                is_unique: false,
                is_primary_key: false,
                foreign_key_reference: None,
                has_default: false,
                is_compressed: false,
            },
        ];

        // Expected schema: id, age, name (changed order)
        let expected = vec![
            ColumnInfo {
                name: "id".to_string(),
                sql_type: "TEXT".to_string(),
                nullable: false,
                position: 0,
                is_unique: true,
                is_primary_key: true,
                foreign_key_reference: None,
                has_default: true,
                is_compressed: false,
            },
            ColumnInfo {
                name: "age".to_string(),
                sql_type: "INTEGER".to_string(),
                nullable: false,
                position: 1, // Changed from 2 to 1
                is_unique: false,
                is_primary_key: false,
                foreign_key_reference: None,
                has_default: false,
                is_compressed: false,
            },
            ColumnInfo {
                name: "name".to_string(),
                sql_type: "TEXT".to_string(),
                nullable: false,
                position: 2, // Changed from 1 to 2
                is_unique: false,
                is_primary_key: false,
                foreign_key_reference: None,
                has_default: false,
                is_compressed: false,
            },
        ];

        let comparison = compare_schemas(&current, &expected);

        // Should detect that migration is needed
        assert!(
            comparison.needs_migration,
            "Should detect column order change"
        );

        // Should have exactly 2 differences (both name and age positions changed)
        assert_eq!(
            comparison.differences.len(),
            2,
            "Should detect 2 position changes"
        );

        // Verify both columns are detected as having order changes
        let descriptions: Vec<String> = comparison
            .differences
            .iter()
            .map(|d| d.describe())
            .collect();

        assert!(
            descriptions
                .iter()
                .any(|d| d.contains("age") && d.contains("position")),
            "Should detect age position change"
        );
        assert!(
            descriptions
                .iter()
                .any(|d| d.contains("name") && d.contains("position")),
            "Should detect name position change"
        );
    }

    #[test]
    fn test_column_order_no_change() {
        // Both schemas have same order
        let current = vec![
            ColumnInfo {
                name: "id".to_string(),
                sql_type: "TEXT".to_string(),
                nullable: false,
                position: 0,
                is_unique: true,
                is_primary_key: true,
                foreign_key_reference: None,
                has_default: true,
                is_compressed: false,
            },
            ColumnInfo {
                name: "name".to_string(),
                sql_type: "TEXT".to_string(),
                nullable: false,
                position: 1,
                is_unique: false,
                is_primary_key: false,
                foreign_key_reference: None,
                has_default: false,
                is_compressed: false,
            },
        ];

        let expected = current.clone();

        let comparison = compare_schemas(&current, &expected);

        // Should NOT need migration
        assert!(
            !comparison.needs_migration,
            "Should match when order is same"
        );
        assert_eq!(
            comparison.differences.len(),
            0,
            "Should have no differences"
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    // Integration tests require a PostgreSQL database
    // These tests are marked with #[ignore] by default
    // Run with: cargo test -- --ignored

    async fn setup_test_db() -> std::result::Result<sqlx::PgPool, Box<dyn std::error::Error>> {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://postgres:password@localhost/orso_test".to_string());

        let pool = sqlx::PgPool::connect(&database_url).await?;
        Ok(pool)
    }

    async fn cleanup_test_tables(
        pool: &sqlx::PgPool,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        sqlx::query("DROP TABLE IF EXISTS test_users CASCADE")
            .execute(pool)
            .await?;
        sqlx::query("DROP TABLE IF EXISTS test_posts CASCADE")
            .execute(pool)
            .await?;
        sqlx::query("DROP TABLE IF EXISTS test_regime_data CASCADE")
            .execute(pool)
            .await?;
        Ok(())
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    async fn test_compressed_roundtrip() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await?;
        cleanup_test_tables(&pool).await?;

        // Create table
        sqlx::query(&RegimeData::create_table_sql())
            .execute(&pool)
            .await?;

        // Insert data with compressed fields
        let prices = vec![100.0, 101.5, 102.0, 103.5, 104.0];
        let volumes = vec![1000, 1100, 1050, 1200, 1150];

        let data = RegimeData {
            id: "test_1".to_string(),
            pair: "BTCUSDT".to_string(),
            prices: Compressed::new(prices.clone()),
            volumes: Compressed::new(volumes.clone()),
        };

        // INSERT
        sqlx::query(
            "INSERT INTO test_regime_data (id, pair, prices, volumes) VALUES ($1, $2, $3, $4)",
        )
        .bind(&data.id)
        .bind(&data.pair)
        .bind(&data.prices)
        .bind(&data.volumes)
        .execute(&pool)
        .await?;

        // SELECT
        let retrieved: RegimeData =
            sqlx::query_as("SELECT id, pair, prices, volumes FROM test_regime_data WHERE id = $1")
                .bind("test_1")
                .fetch_one(&pool)
                .await?;

        // Verify
        assert_eq!(retrieved.id, "test_1");
        assert_eq!(retrieved.pair, "BTCUSDT");
        assert_eq!(retrieved.prices.as_slice(), &prices[..]);
        assert_eq!(retrieved.volumes.as_slice(), &volumes[..]);

        cleanup_test_tables(&pool).await?;
        Ok(())
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    async fn test_basic_crud_operations() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await?;
        cleanup_test_tables(&pool).await?;

        // Create table
        sqlx::query(&User::create_table_sql())
            .execute(&pool)
            .await?;

        // INSERT
        sqlx::query("INSERT INTO test_users (id, name, age) VALUES ($1, $2, $3)")
            .bind("user_1")
            .bind("Alice")
            .bind(25)
            .execute(&pool)
            .await?;

        // SELECT
        let user: User = sqlx::query_as("SELECT id, name, age FROM test_users WHERE id = $1")
            .bind("user_1")
            .fetch_one(&pool)
            .await?;

        assert_eq!(user.id, "user_1");
        assert_eq!(user.name, "Alice");
        assert_eq!(user.age, 25);

        // UPDATE
        sqlx::query("UPDATE test_users SET age = $1 WHERE id = $2")
            .bind(26)
            .bind("user_1")
            .execute(&pool)
            .await?;

        let updated: User = sqlx::query_as("SELECT id, name, age FROM test_users WHERE id = $1")
            .bind("user_1")
            .fetch_one(&pool)
            .await?;

        assert_eq!(updated.age, 26);

        // DELETE
        sqlx::query("DELETE FROM test_users WHERE id = $1")
            .bind("user_1")
            .execute(&pool)
            .await?;

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM test_users")
            .fetch_one(&pool)
            .await?;

        assert_eq!(count.0, 0);

        cleanup_test_tables(&pool).await?;
        Ok(())
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    async fn test_nullable_fields() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await?;
        cleanup_test_tables(&pool).await?;

        // Create table
        sqlx::query(&Post::create_table_sql())
            .execute(&pool)
            .await?;

        // INSERT with NULL content
        sqlx::query("INSERT INTO test_posts (id, title, content, views) VALUES ($1, $2, $3, $4)")
            .bind("post_1")
            .bind("First Post")
            .bind(None::<String>)
            .bind(100i64)
            .execute(&pool)
            .await?;

        let post: Post =
            sqlx::query_as("SELECT id, title, content, views FROM test_posts WHERE id = $1")
                .bind("post_1")
                .fetch_one(&pool)
                .await?;

        assert_eq!(post.id, "post_1");
        assert_eq!(post.title, "First Post");
        assert_eq!(post.content, None);
        assert_eq!(post.views, 100);

        // INSERT with non-NULL content
        sqlx::query("INSERT INTO test_posts (id, title, content, views) VALUES ($1, $2, $3, $4)")
            .bind("post_2")
            .bind("Second Post")
            .bind(Some("Content here".to_string()))
            .bind(200i64)
            .execute(&pool)
            .await?;

        let post2: Post =
            sqlx::query_as("SELECT id, title, content, views FROM test_posts WHERE id = $1")
                .bind("post_2")
                .fetch_one(&pool)
                .await?;

        assert_eq!(post2.content, Some("Content here".to_string()));

        cleanup_test_tables(&pool).await?;
        Ok(())
    }
}
