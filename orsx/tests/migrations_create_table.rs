use orsx::prelude::*;

#[derive(OrsxMigrate)]
#[orsx_table("orsx2_smoke_users")]
struct User {
    #[orsx_column(primary_key)]
    id: String,
    name: String,
    email: Option<String>,
    #[orsx_column(index(unique))]
    username: String,
}

#[tokio::test]
async fn migrations_create_table_smoke() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    sqlx::query("DROP TABLE IF EXISTS orsx2_smoke_users CASCADE")
        .execute(&pool)
        .await
        .unwrap();

    let dummy = User {
        id: uuid::Uuid::new_v4().to_string(),
        name: "n".to_string(),
        email: None,
        username: "u".to_string(),
    };

    Migrations::init(&pool, &[(dummy, None)]).await.unwrap();

    // Insert using raw sqlx (library remains raw-SQL-first).
    sqlx::query("INSERT INTO orsx2_smoke_users (id, name, email, username) VALUES ($1,$2,$3,$4)")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind("Alice")
        .bind(Option::<String>::None)
        .bind("alice")
        .execute(&pool)
        .await
        .unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*)::BIGINT FROM orsx2_smoke_users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1);
}

