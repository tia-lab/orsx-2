#![allow(dead_code)]

use orsx::migrations::config::MigrationConfig;
use orsx::prelude::*;
use uuid::Uuid;

#[derive(OrsxMigrate)]
#[orsx_table("orsx2_strict_order")]
struct StrictOrder {
    #[orsx_column(primary_key)]
    id: String,
    a: i32,
    b: i32,
}

#[derive(OrsxMigrate)]
#[orsx_table("orsx2_strict_exact")]
struct StrictExact {
    #[orsx_column(primary_key)]
    id: String,
    a: i32,
}

#[derive(OrsxMigrate)]
#[orsx_table("orsx2_rename")]
struct RenameV2 {
    #[orsx_column(primary_key)]
    id: String,
    #[orsx_column(rename_from = "old_name")]
    new_name: String,
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

async fn drop_with_prefix_cleanup(pool: &sqlx::PgPool, table: &str) {
    let _ = sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", orsx::quote_identifier(table)))
        .execute(pool)
        .await;
    let like_backup = format!("{}__orsx2_backup_%", table.replace('\'', "''"));
    let like_shadow = format!("{}__orsx2_shadow_%", table.replace('\'', "''"));
    let like_changelog = format!("{}__orsx2_changelog_%", table.replace('\'', "''"));

    let sql = format!(
        r#"
        DO $$
        DECLARE r record;
        BEGIN
          FOR r IN
            SELECT tablename
            FROM pg_catalog.pg_tables
            WHERE schemaname = 'public'
              AND (
                tablename LIKE '{like_backup}' OR
                tablename LIKE '{like_shadow}' OR
                tablename LIKE '{like_changelog}'
              )
          LOOP
            EXECUTE format('DROP TABLE IF EXISTS %I CASCADE', r.tablename);
          END LOOP;
        END $$;
        "#
    );
    let _ = sqlx::query(&sql).execute(pool).await;
}

#[tokio::test]
async fn strict_column_order_triggers_rewrite_and_matches() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    let table = format!("orsx2_strict_order_{}", Uuid::new_v4().simple());
    drop_with_prefix_cleanup(&pool, &table).await;

    // Create table with wrong physical order: id, b, a.
    sqlx::query(
        &format!(
            r#"
        CREATE TABLE {} (
          id TEXT PRIMARY KEY,
          b INTEGER NOT NULL,
          a INTEGER NOT NULL
        )
        "#,
            orsx::quote_identifier(&table)
        ),
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(&format!(
        "INSERT INTO {} (id, a, b) VALUES ($1,$2,$3)",
        orsx::quote_identifier(&table)
    ))
        .bind("r1")
        .bind(10_i32)
        .bind(20_i32)
        .execute(&pool)
        .await
        .unwrap();

    let cfg = MigrationConfig {
        enforce_column_order: true,
        online_chunk_size: 100,
        ..MigrationConfig::default()
    };

    let dummy = StrictOrder {
        id: "dummy".to_string(),
        a: 0,
        b: 0,
    };
    Migrations::init_with_config(&pool, &[(dummy, Some(&table))], &cfg)
        .await
        .unwrap();

    let cols = pg_columns_in_order(&pool, &table).await;
    assert_eq!(cols, vec!["id", "a", "b"]);

    let row: (i32, i32) = sqlx::query_as(&format!(
        "SELECT a, b FROM {} WHERE id = $1",
        orsx::quote_identifier(&table)
    ))
        .bind("r1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.0, 10);
    assert_eq!(row.1, 20);
}

#[tokio::test]
async fn strict_exact_columns_requires_destructive_flag_for_extra_columns() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    let table = format!("orsx2_strict_exact_{}", Uuid::new_v4().simple());
    drop_with_prefix_cleanup(&pool, &table).await;

    // Create table with an extra column not present in the spec.
    sqlx::query(
        &format!(
            r#"
        CREATE TABLE {} (
          id TEXT PRIMARY KEY,
          a INTEGER NOT NULL,
          extra TEXT NOT NULL
        )
        "#,
            orsx::quote_identifier(&table)
        ),
    )
    .execute(&pool)
    .await
    .unwrap();

    let cfg = MigrationConfig {
        enforce_exact_columns: true,
        allow_destructive_drops: false,
        ..MigrationConfig::default()
    };

    let dummy = StrictExact {
        id: "dummy".to_string(),
        a: 0,
    };
    let res = Migrations::init_with_config(&pool, &[(dummy, Some(&table))], &cfg).await;
    assert!(
        matches!(res, Err(orsx::Error::MigrationNeeded(_))),
        "expected MigrationNeeded, got: {res:?}"
    );
}

#[tokio::test]
async fn strict_exact_columns_rewrite_removes_extra_but_keeps_backup() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    let table = format!("orsx2_strict_exact_{}", Uuid::new_v4().simple());
    drop_with_prefix_cleanup(&pool, &table).await;

    sqlx::query(
        &format!(
            r#"
        CREATE TABLE {} (
          id TEXT PRIMARY KEY,
          a INTEGER NOT NULL,
          extra TEXT NOT NULL
        )
        "#,
            orsx::quote_identifier(&table)
        ),
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(&format!(
        "INSERT INTO {} (id, a, extra) VALUES ($1,$2,$3)",
        orsx::quote_identifier(&table)
    ))
        .bind("r1")
        .bind(7_i32)
        .bind("keep_in_backup")
        .execute(&pool)
        .await
        .unwrap();

    let cfg = MigrationConfig {
        enforce_exact_columns: true,
        allow_destructive_drops: true,
        online_chunk_size: 50,
        ..MigrationConfig::default()
    };

    let dummy = StrictExact {
        id: "dummy".to_string(),
        a: 0,
    };
    Migrations::init_with_config(&pool, &[(dummy, Some(&table))], &cfg)
        .await
        .unwrap();

    // Live table: extra column should be gone.
    let live_cols = pg_columns_in_order(&pool, &table).await;
    assert_eq!(live_cols, vec!["id", "a"]);

    let row: (i32,) = sqlx::query_as(&format!(
        "SELECT a FROM {} WHERE id = $1",
        orsx::quote_identifier(&table)
    ))
        .bind("r1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.0, 7);

    // Backup table: should exist and still contain `extra`.
    // Backup table names may be shortened to fit Postgres' identifier limit, so search by the
    // stable `__orsx2_backup_` marker and validate by contents.
    let candidates: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT tablename
        FROM pg_catalog.pg_tables
        WHERE schemaname = 'public'
          AND tablename LIKE '%__orsx2_backup_%'
        ORDER BY tablename DESC
        LIMIT 25
        "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let mut found_backup: Option<String> = None;
    for cand in candidates {
        let extra_exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
              SELECT 1
              FROM information_schema.columns
              WHERE table_schema = 'public'
                AND table_name = $1
                AND column_name = 'extra'
            )
            "#,
        )
        .bind(&cand)
        .fetch_one(&pool)
        .await
        .unwrap();
        if !extra_exists {
            continue;
        }

        let row: Option<(String,)> = sqlx::query_as(&format!(
            "SELECT extra FROM {} WHERE id = $1 LIMIT 1",
            orsx::quote_identifier(&cand)
        ))
        .bind("r1")
        .fetch_optional(&pool)
        .await
        .unwrap();
        if let Some((v,)) = row {
            if v == "keep_in_backup" {
                found_backup = Some(cand);
                break;
            }
        }
    }

    assert!(
        found_backup.is_some(),
        "expected to find backup table containing id=r1 and extra=keep_in_backup"
    );
}

#[tokio::test]
async fn rename_from_applies_via_safe_rename_and_preserves_data() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    let table = format!("orsx2_rename_{}", Uuid::new_v4().simple());
    drop_with_prefix_cleanup(&pool, &table).await;

    sqlx::query(
        &format!(
            r#"
        CREATE TABLE {} (
          id TEXT PRIMARY KEY,
          old_name TEXT NOT NULL
        )
        "#,
            orsx::quote_identifier(&table)
        ),
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(&format!(
        "INSERT INTO {} (id, old_name) VALUES ($1,$2)",
        orsx::quote_identifier(&table)
    ))
        .bind("r1")
        .bind("v1")
        .execute(&pool)
        .await
        .unwrap();

    let cfg = MigrationConfig::default();
    let dummy = RenameV2 {
        id: "dummy".to_string(),
        new_name: "dummy".to_string(),
    };
    Migrations::init_with_config(&pool, &[(dummy, Some(&table))], &cfg)
        .await
        .unwrap();

    let cols = pg_columns_in_order(&pool, &table).await;
    assert_eq!(cols, vec!["id", "new_name"]);

    let v: (String,) = sqlx::query_as(&format!(
        "SELECT new_name FROM {} WHERE id = $1",
        orsx::quote_identifier(&table)
    ))
        .bind("r1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(v.0, "v1");
}
