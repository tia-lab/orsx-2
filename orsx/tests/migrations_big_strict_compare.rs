#![allow(dead_code)]

use orsx::migrations::config::MigrationConfig;
use orsx::prelude::*;
use uuid::Uuid;

#[derive(OrsxMigrate)]
#[orsx_table("orsx2_big_cmp")]
struct BigCmpSpec {
    #[orsx_column(primary_key)]
    id: Uuid,
    c01: i32,
    c02: i32,
    c03: i32,
    c04: i32,
    c05: i32,
    c06: i32,
    c07: i32,
    c08: i32,
    c09: i32,
    c10: i32,
    c11: i32,
    c12: i32,
    c13: i32,
    c14: i32,
    c15: i32,
    c16: i32,
    c17: i32,
    c18: i32,
    c19: i32,
    c20: i32,
    c21: i32,
    c22: i32,
    c23: i32,
    c24: i32,
    c25: i32,
    c26: i32,
    c27: i32,
    c28: i32,
    c29: i32,
    c30: i32,
    c31: i32,
    c32: i32,
    c33: i32,
    c34: i32,
    c35: i32,
    c36: i32,
    c37: i32,
    c38: i32,
    c39: i32,
    c40: i32,
    c41: i32,
    c42: i32,
    c43: i32,
    c44: i32,
    c45: i32,
    c46: i32,
    c47: i32,
    c48: i32,
    c49: i32,
    // Safe-alter candidate: nullable column.
    new_nullable: Option<i32>,
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .and_then(|s| match s.as_str() {
            "1" | "true" | "TRUE" | "yes" | "YES" => Some(true),
            "0" | "false" | "FALSE" | "no" | "NO" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

fn cfg_from_env(mut cfg: MigrationConfig) -> MigrationConfig {
    let adaptive = env_bool("ORSX_ADAPTIVE_CHUNK", false);
    cfg.adaptive_chunk = adaptive;
    cfg.online_chunk_size_min = env_u64("ORSX_CHUNK_MIN", cfg.online_chunk_size_min as u64) as i64;
    cfg.online_chunk_size_max = env_u64("ORSX_CHUNK_MAX", cfg.online_chunk_size_max as u64) as i64;
    cfg.online_target_batch_ms = env_u64("ORSX_TARGET_BATCH_MS", cfg.online_target_batch_ms);
    cfg.online_max_batch_ms = env_u64("ORSX_MAX_BATCH_MS", cfg.online_max_batch_ms);
    cfg.synchronous_commit_off = env_bool("ORSX_SYNC_COMMIT_OFF", cfg.synchronous_commit_off);
    cfg
}

async fn create_base_table_wrong_order(pool: &sqlx::PgPool, table: &str) {
    // Ensure UUID generator is available.
    sqlx::query(r#"CREATE EXTENSION IF NOT EXISTS "uuid-ossp""#)
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", orsx::quote_identifier(table)))
        .execute(pool)
        .await
        .unwrap();

    // Wrong physical order vs spec: id, c02, c01, c03..c49 (no new_nullable).
    sqlx::query(&format!(
        r#"
        CREATE TABLE {} (
          id UUID PRIMARY KEY,
          c02 INTEGER NOT NULL,
          c01 INTEGER NOT NULL,
          c03 INTEGER NOT NULL,
          c04 INTEGER NOT NULL,
          c05 INTEGER NOT NULL,
          c06 INTEGER NOT NULL,
          c07 INTEGER NOT NULL,
          c08 INTEGER NOT NULL,
          c09 INTEGER NOT NULL,
          c10 INTEGER NOT NULL,
          c11 INTEGER NOT NULL,
          c12 INTEGER NOT NULL,
          c13 INTEGER NOT NULL,
          c14 INTEGER NOT NULL,
          c15 INTEGER NOT NULL,
          c16 INTEGER NOT NULL,
          c17 INTEGER NOT NULL,
          c18 INTEGER NOT NULL,
          c19 INTEGER NOT NULL,
          c20 INTEGER NOT NULL,
          c21 INTEGER NOT NULL,
          c22 INTEGER NOT NULL,
          c23 INTEGER NOT NULL,
          c24 INTEGER NOT NULL,
          c25 INTEGER NOT NULL,
          c26 INTEGER NOT NULL,
          c27 INTEGER NOT NULL,
          c28 INTEGER NOT NULL,
          c29 INTEGER NOT NULL,
          c30 INTEGER NOT NULL,
          c31 INTEGER NOT NULL,
          c32 INTEGER NOT NULL,
          c33 INTEGER NOT NULL,
          c34 INTEGER NOT NULL,
          c35 INTEGER NOT NULL,
          c36 INTEGER NOT NULL,
          c37 INTEGER NOT NULL,
          c38 INTEGER NOT NULL,
          c39 INTEGER NOT NULL,
          c40 INTEGER NOT NULL,
          c41 INTEGER NOT NULL,
          c42 INTEGER NOT NULL,
          c43 INTEGER NOT NULL,
          c44 INTEGER NOT NULL,
          c45 INTEGER NOT NULL,
          c46 INTEGER NOT NULL,
          c47 INTEGER NOT NULL,
          c48 INTEGER NOT NULL,
          c49 INTEGER NOT NULL
        )
        "#,
        orsx::quote_identifier(table)
    ))
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_rows(pool: &sqlx::PgPool, table: &str, rows: u64) -> std::time::Duration {
    let start = std::time::Instant::now();
    sqlx::query(&format!(
        r#"
        INSERT INTO {} (
          id,
          c02,c01,c03,c04,c05,c06,c07,c08,c09,c10,
          c11,c12,c13,c14,c15,c16,c17,c18,c19,c20,
          c21,c22,c23,c24,c25,c26,c27,c28,c29,c30,
          c31,c32,c33,c34,c35,c36,c37,c38,c39,c40,
          c41,c42,c43,c44,c45,c46,c47,c48,c49
        )
        SELECT
          uuid_generate_v1mc(),
          (gs % 2147483000)::int4,
          ((gs + 1) % 2147483000)::int4,
          ((gs + 2) % 2147483000)::int4,
          ((gs + 3) % 2147483000)::int4,
          ((gs + 4) % 2147483000)::int4,
          ((gs + 5) % 2147483000)::int4,
          ((gs + 6) % 2147483000)::int4,
          ((gs + 7) % 2147483000)::int4,
          ((gs + 8) % 2147483000)::int4,
          ((gs + 9) % 2147483000)::int4,
          ((gs + 10) % 2147483000)::int4,
          ((gs + 11) % 2147483000)::int4,
          ((gs + 12) % 2147483000)::int4,
          ((gs + 13) % 2147483000)::int4,
          ((gs + 14) % 2147483000)::int4,
          ((gs + 15) % 2147483000)::int4,
          ((gs + 16) % 2147483000)::int4,
          ((gs + 17) % 2147483000)::int4,
          ((gs + 18) % 2147483000)::int4,
          ((gs + 19) % 2147483000)::int4,
          ((gs + 20) % 2147483000)::int4,
          ((gs + 21) % 2147483000)::int4,
          ((gs + 22) % 2147483000)::int4,
          ((gs + 23) % 2147483000)::int4,
          ((gs + 24) % 2147483000)::int4,
          ((gs + 25) % 2147483000)::int4,
          ((gs + 26) % 2147483000)::int4,
          ((gs + 27) % 2147483000)::int4,
          ((gs + 28) % 2147483000)::int4,
          ((gs + 29) % 2147483000)::int4,
          ((gs + 30) % 2147483000)::int4,
          ((gs + 31) % 2147483000)::int4,
          ((gs + 32) % 2147483000)::int4,
          ((gs + 33) % 2147483000)::int4,
          ((gs + 34) % 2147483000)::int4,
          ((gs + 35) % 2147483000)::int4,
          ((gs + 36) % 2147483000)::int4,
          ((gs + 37) % 2147483000)::int4,
          ((gs + 38) % 2147483000)::int4,
          ((gs + 39) % 2147483000)::int4,
          ((gs + 40) % 2147483000)::int4,
          ((gs + 41) % 2147483000)::int4,
          ((gs + 42) % 2147483000)::int4,
          ((gs + 43) % 2147483000)::int4,
          ((gs + 44) % 2147483000)::int4,
          ((gs + 45) % 2147483000)::int4,
          ((gs + 46) % 2147483000)::int4,
          ((gs + 47) % 2147483000)::int4,
          ((gs + 48) % 2147483000)::int4
        FROM generate_series(1, $1::bigint) AS gs
        "#,
        orsx::quote_identifier(table)
    ))
    .bind(rows as i64)
    .execute(pool)
    .await
    .unwrap();
    start.elapsed()
}

#[tokio::test]
#[ignore]
async fn big_table_compare_default_vs_strict_enforcement() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    let rows = env_u64("ORSX_BIG_ROWS", 200_000);
    let default_table = format!("orsx2_big_cmp_def_{}", Uuid::new_v4().simple());
    let strict_table = format!("orsx2_big_cmp_str_{}", Uuid::new_v4().simple());

    // DEFAULT run: safe alter should add nullable column without rewrite (order mismatch ignored).
    create_base_table_wrong_order(&pool, &default_table).await;
    let seed_def = seed_rows(&pool, &default_table, rows).await;

    let default_cfg = cfg_from_env(MigrationConfig::default());
    let dummy1 = BigCmpSpec {
        id: Uuid::new_v4(),
        c01: 0,
        c02: 0,
        c03: 0,
        c04: 0,
        c05: 0,
        c06: 0,
        c07: 0,
        c08: 0,
        c09: 0,
        c10: 0,
        c11: 0,
        c12: 0,
        c13: 0,
        c14: 0,
        c15: 0,
        c16: 0,
        c17: 0,
        c18: 0,
        c19: 0,
        c20: 0,
        c21: 0,
        c22: 0,
        c23: 0,
        c24: 0,
        c25: 0,
        c26: 0,
        c27: 0,
        c28: 0,
        c29: 0,
        c30: 0,
        c31: 0,
        c32: 0,
        c33: 0,
        c34: 0,
        c35: 0,
        c36: 0,
        c37: 0,
        c38: 0,
        c39: 0,
        c40: 0,
        c41: 0,
        c42: 0,
        c43: 0,
        c44: 0,
        c45: 0,
        c46: 0,
        c47: 0,
        c48: 0,
        c49: 0,
        new_nullable: None,
    };

    let t0 = std::time::Instant::now();
    Migrations::init_with_config(&pool, &[(dummy1, Some(&default_table))], &default_cfg)
        .await
        .unwrap();
    let mig_def = t0.elapsed();

    // STRICT run: should force rewrite because column order mismatch is enforced.
    create_base_table_wrong_order(&pool, &strict_table).await;
    let seed_str = seed_rows(&pool, &strict_table, rows).await;

    let strict_cfg = cfg_from_env(MigrationConfig {
        enforce_column_order: true,
        enforce_exact_columns: true,
        allow_destructive_drops: true,
        online_chunk_size: 20_000,
        ..MigrationConfig::default()
    });

    let dummy2 = BigCmpSpec {
        id: Uuid::new_v4(),
        c01: 0,
        c02: 0,
        c03: 0,
        c04: 0,
        c05: 0,
        c06: 0,
        c07: 0,
        c08: 0,
        c09: 0,
        c10: 0,
        c11: 0,
        c12: 0,
        c13: 0,
        c14: 0,
        c15: 0,
        c16: 0,
        c17: 0,
        c18: 0,
        c19: 0,
        c20: 0,
        c21: 0,
        c22: 0,
        c23: 0,
        c24: 0,
        c25: 0,
        c26: 0,
        c27: 0,
        c28: 0,
        c29: 0,
        c30: 0,
        c31: 0,
        c32: 0,
        c33: 0,
        c34: 0,
        c35: 0,
        c36: 0,
        c37: 0,
        c38: 0,
        c39: 0,
        c40: 0,
        c41: 0,
        c42: 0,
        c43: 0,
        c44: 0,
        c45: 0,
        c46: 0,
        c47: 0,
        c48: 0,
        c49: 0,
        new_nullable: None,
    };

    let t1 = std::time::Instant::now();
    Migrations::init_with_config(&pool, &[(dummy2, Some(&strict_table))], &strict_cfg)
        .await
        .unwrap();
    let mig_str = t1.elapsed();

    // Verify strict table column order matches spec order.
    let cols: Vec<String> = sqlx::query_scalar(
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
    .bind(&strict_table)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(cols[0], "id");
    assert_eq!(cols[1], "c01");
    assert_eq!(cols[2], "c02");
    assert_eq!(cols.last().map(|s| s.as_str()), Some("new_nullable"));

    println!("rows={rows}");
    println!("default: seed={:?} migrate={:?}", seed_def, mig_def);
    println!("strict:  seed={:?} migrate={:?}", seed_str, mig_str);
    println!(
        "cfg: adaptive_chunk={} chunk_size={} min={} max={} target_ms={} max_ms={}",
        strict_cfg.adaptive_chunk,
        strict_cfg.online_chunk_size,
        strict_cfg.online_chunk_size_min,
        strict_cfg.online_chunk_size_max,
        strict_cfg.online_target_batch_ms,
        strict_cfg.online_max_batch_ms
    );
}
