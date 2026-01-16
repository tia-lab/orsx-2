#![allow(dead_code)]

use orsx::prelude::*;

#[derive(OrsxMigrate)]
#[orsx_table("orsx2_smoke_add_cols")]
struct V2 {
    #[orsx_column(primary_key)]
    id: String,
    name: String,
    // Newly added, nullable (safe fast-path).
    email: Option<String>,
    // Newly added, nullable + unique (supported via unique index concurrently).
    #[orsx_column(unique)]
    username: Option<String>,
}

#[tokio::test]
async fn migrations_add_nullable_columns_and_unique() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    sqlx::query("DROP TABLE IF EXISTS orsx2_smoke_add_cols CASCADE")
        .execute(&pool)
        .await
        .unwrap();

    // Create an older/smaller schema version of the table.
    sqlx::query(
        r#"
        CREATE TABLE orsx2_smoke_add_cols (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let dummy = V2 {
        id: uuid::Uuid::new_v4().to_string(),
        name: "n".to_string(),
        email: None,
        username: None,
    };

    // Should apply ALTER TABLE ADD COLUMN for nullable columns and add uniqueness.
    Migrations::init(&pool, &[(dummy, None)]).await.unwrap();

    // Ensure we can insert with NULLs.
    sqlx::query("INSERT INTO orsx2_smoke_add_cols (id, name, email, username) VALUES ($1,$2,$3,$4)")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind("Alice")
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .execute(&pool)
        .await
        .unwrap();

    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*)::BIGINT FROM orsx2_smoke_add_cols")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count.0, 1);
}
