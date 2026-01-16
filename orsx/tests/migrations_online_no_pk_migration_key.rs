#![allow(dead_code)]

use orsx::migrations::config::MigrationConfig;
use orsx::prelude::*;
use sqlx::Connection;
use uuid::Uuid;

#[derive(OrsxMigrate)]
#[orsx_table("orsx2_no_pk")]
struct NoPkSpec {
    id: String,
    a: i32,
    b: i32,
}

async fn pg_columns_in_order(pool: &sqlx::PgPool, table: &str) -> Vec<String> {
    sqlx::query_scalar(
        r#"
        SELECT a.attname
        FROM pg_catalog.pg_attribute a
        JOIN pg_catalog.pg_class c ON c.oid = a.attrelid
        JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'public'
          AND c.relname = $1
          AND a.attnum > 0
          AND NOT a.attisdropped
        ORDER BY a.attnum
        "#,
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn online_rewrite_works_without_pk_when_migration_key_enabled() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    let table = format!("orsx2_no_pk_{}", Uuid::new_v4().simple());
    let mut conn = sqlx::PgConnection::connect(&url).await.unwrap();

    // Create table with wrong physical order and no PK: id, b, a.
    sqlx::query(&format!(
        "DROP TABLE IF EXISTS {} CASCADE",
        orsx::quote_identifier(&table)
    ))
    .execute(&mut conn)
    .await
    .unwrap();
    sqlx::query(&format!(
        "CREATE TABLE {} (id TEXT NOT NULL, b INTEGER NOT NULL, a INTEGER NOT NULL)",
        orsx::quote_identifier(&table)
    ))
    .execute(&mut conn)
    .await
    .unwrap();

    // Insert a row.
    sqlx::query(&format!(
        "INSERT INTO {} (id, a, b) VALUES ('r1', 10, 20)",
        orsx::quote_identifier(&table)
    ))
    .execute(&mut conn)
    .await
    .unwrap();

    let cfg = MigrationConfig {
        enable_migration_key: true,
        enforce_column_order: true,
        online_chunk_size: 50,
        ..MigrationConfig::default()
    };

    let dummy = NoPkSpec {
        id: "x".into(),
        a: 0,
        b: 0,
    };
    Migrations::init_with_config(&pool, &[(dummy, Some(&table))], &cfg)
        .await
        .unwrap();

    // Column order matches spec, with the migration key appended.
    let cols = pg_columns_in_order(&pool, &table).await;
    assert_eq!(cols[0..3], ["id", "a", "b"]);
    assert!(cols.contains(&"__orsx_mig_id".to_string()));

    // Data preserved.
    let row: (i32, i32) = sqlx::query_as(&format!(
        "SELECT a, b FROM {} WHERE id = 'r1'",
        orsx::quote_identifier(&table)
    ))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, 10);
    assert_eq!(row.1, 20);

    // Migration key populated (NOT NULL).
    let mig_count_null: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM {} WHERE __orsx_mig_id IS NULL",
        orsx::quote_identifier(&table)
    ))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mig_count_null, 0);

    // Backup table exists (online rewrite keeps it). Note: base table name may be truncated
    // to stay under Postgres' identifier limit, so search by the stable marker.
    let candidates: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT tablename
        FROM pg_catalog.pg_tables
        WHERE schemaname = 'public'
          AND tablename LIKE '%__orsx2_backup_%'
        "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let mut found = false;
    for b in candidates {
        let q = format!(
            "SELECT COUNT(*)::BIGINT FROM {} WHERE id = 'r1'",
            orsx::quote_identifier(&b)
        );
        let c: i64 = sqlx::query_scalar(&q).fetch_one(&pool).await.unwrap_or(0);
        if c == 1 {
            found = true;
            break;
        }
    }
    assert!(found, "expected a backup table containing id='r1' to exist");
}
