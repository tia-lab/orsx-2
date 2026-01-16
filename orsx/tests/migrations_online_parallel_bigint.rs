use orsx::migrations::config::MigrationConfig;
use orsx::prelude::*;

#[derive(OrsxMigrate)]
#[orsx_table("orsx2_parallel_bigint")]
struct V2 {
    #[orsx_column(primary_key)]
    id: i64,
    v: i32,
    #[orsx_column(default_sql = "0")]
    new_col: i32,
}

#[tokio::test]
async fn online_rewrite_parallel_backfill_bigint_pk_smoke() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    sqlx::query("DROP TABLE IF EXISTS orsx2_parallel_bigint CASCADE")
        .execute(&pool)
        .await
        .unwrap();

    // Old schema: no `new_col` and `v` NOT NULL.
    sqlx::query(
        r#"
        CREATE TABLE orsx2_parallel_bigint (
          id BIGINT PRIMARY KEY,
          v INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Seed enough rows to exercise multiple backfill chunks.
    sqlx::query(
        r#"
        INSERT INTO orsx2_parallel_bigint (id, v)
        SELECT gs::bigint, (gs % 2147483000)::int4
        FROM generate_series(1, 50000) AS gs
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let cfg = MigrationConfig {
        // Force online path even at small scale (we are testing behavior, not the planner threshold).
        offline_row_threshold: 0,
        online_chunk_size: 5_000,
        parallel_backfill: true,
        parallel_backfill_workers: 4,
        ..MigrationConfig::default()
    };

    let dummy = V2 {
        id: 1,
        v: 1,
        new_col: 0,
    };
    Migrations::init_with_config(&pool, &[(dummy, None)], &cfg)
        .await
        .unwrap();

    let nulls: (i64,) =
        sqlx::query_as("SELECT COUNT(*)::BIGINT FROM orsx2_parallel_bigint WHERE new_col IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(nulls.0, 0);

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*)::BIGINT FROM orsx2_parallel_bigint")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total.0, 50_000);
}

