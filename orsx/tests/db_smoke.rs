#[tokio::test]
async fn db_smoke_connect_and_query() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());

    let pool = sqlx::PgPool::connect(&url)
        .await
        .expect("failed to connect to test db");

    let one: (i64,) = sqlx::query_as("SELECT 1::BIGINT")
        .fetch_one(&pool)
        .await
        .expect("failed to query");

    assert_eq!(one.0, 1);
}
