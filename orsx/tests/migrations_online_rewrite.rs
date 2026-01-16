use orsx::migrations::config::MigrationConfig;
use orsx::prelude::*;

#[derive(OrsxMigrate)]
#[orsx_table("orsx2_online_rewrite")]
struct V2 {
    #[orsx_column(primary_key)]
    id: String,
    name: String,
    // This is the rewrite trigger: old table doesn't have it, and it's NOT NULL.
    #[orsx_column(default_sql = "0")]
    age: i32,
}

#[tokio::test]
async fn online_rewrite_add_not_null_with_default_and_concurrent_writes() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    // Clean any leftovers.
    sqlx::query("DROP TABLE IF EXISTS orsx2_online_rewrite CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    // Drop any prior backup/shadow/changelog tables from earlier runs.
    let _ = sqlx::query(
        r#"
        DO $$
        DECLARE r record;
        BEGIN
          FOR r IN
            SELECT tablename
            FROM pg_catalog.pg_tables
            WHERE schemaname = 'public'
              AND (
                tablename LIKE 'orsx2_online_rewrite__orsx2_backup_%' OR
                tablename LIKE 'orsx2_online_rewrite__orsx2_shadow_%' OR
                tablename LIKE 'orsx2_online_rewrite__orsx2_changelog_%'
              )
          LOOP
            EXECUTE format('DROP TABLE IF EXISTS %I CASCADE', r.tablename);
          END LOOP;
        END $$;
        "#,
    )
    .execute(&pool)
    .await;

    // Create old schema (no age column).
    sqlx::query(
        r#"
        CREATE TABLE orsx2_online_rewrite (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Seed rows.
    for i in 0..100 {
        sqlx::query("INSERT INTO orsx2_online_rewrite (id, name) VALUES ($1,$2)")
            .bind(format!("seed_{i}"))
            .bind("seed")
            .execute(&pool)
            .await
            .unwrap();
    }

    // Concurrent inserts while migration runs.
    let pool2 = pool.clone();
    let writer = tokio::spawn(async move {
        for i in 0..200 {
            let _ = sqlx::query("INSERT INTO orsx2_online_rewrite (id, name) VALUES ($1,$2)")
                .bind(format!("live_{i}"))
                .bind("live")
                .execute(&pool2)
                .await;
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    });

    let cfg = MigrationConfig {
        online_chunk_size: 10,
        online_sleep_ms: 2,
        ..MigrationConfig::default()
    };

    let dummy = V2 {
        id: "dummy".to_string(),
        name: "dummy".to_string(),
        age: 0,
    };

    Migrations::init_with_config(&pool, &[(dummy, None)], &cfg)
        .await
        .unwrap();

    writer.await.unwrap();

    // Verify the new table has the age column and all rows exist.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*)::BIGINT FROM orsx2_online_rewrite")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(count.0 >= 100, "expected at least seeded rows");

    // Default should be applied (age column exists and defaults to 0 for rows written during migration).
    let min_age: (i32,) = sqlx::query_as("SELECT MIN(age) FROM orsx2_online_rewrite")
        .fetch_one(&pool)
        .await
        .unwrap();
    let max_age: (i32,) = sqlx::query_as("SELECT MAX(age) FROM orsx2_online_rewrite")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(min_age.0, 0);
    assert_eq!(max_age.0, 0);
}
