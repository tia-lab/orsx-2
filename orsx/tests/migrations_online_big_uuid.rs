use orsx::migrations::config::MigrationConfig;
use orsx::prelude::*;
use uuid::Uuid;

fn init_tracing() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let filter = std::env::var("RUST_LOG")
            .unwrap_or_else(|_| "info,sqlx=warn".to_string());
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_test_writer()
            .init();
    });
}

#[derive(OrsxMigrate)]
#[orsx_table("orsx2_big_uuid")]
struct V2 {
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
    // Rewrite trigger: this column does not exist in the old table and is NOT NULL.
    #[orsx_column(default_sql = "0")]
    new_col: i32,
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
    cfg
}

#[tokio::test]
#[ignore]
async fn online_rewrite_big_table_uuid_pk() {
    init_tracing();

    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    let rows = env_u64("ORSX_BIG_ROWS", 1_000_000);
    let writer_rows_total = env_u64("ORSX_BIG_WRITER_ROWS", 100_000);
    let writer_batch = env_u64("ORSX_BIG_WRITER_BATCH", 10_000);
    let update_rows_total = env_u64("ORSX_BIG_UPDATE_ROWS", 0);
    let update_batch = env_u64("ORSX_BIG_UPDATE_BATCH", 50_000);
    let delete_rows_total = env_u64("ORSX_BIG_DELETE_ROWS", 0);
    let delete_batch = env_u64("ORSX_BIG_DELETE_BATCH", 10_000);

    // Ensure UUID generator is available (we prefer v1mc for mostly-ordered UUIDs).
    sqlx::query(r#"CREATE EXTENSION IF NOT EXISTS "uuid-ossp""#)
        .execute(&pool)
        .await
        .unwrap();

    // Clean any leftovers (base + prior backup/shadow/changelog).
    sqlx::query("DROP TABLE IF EXISTS orsx2_big_uuid CASCADE")
        .execute(&pool)
        .await
        .unwrap();
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
                tablename LIKE 'orsx2_big_uuid__orsx2_backup_%' OR
                tablename LIKE 'orsx2_big_uuid__orsx2_shadow_%' OR
                tablename LIKE 'orsx2_big_uuid__orsx2_changelog_%'
              )
          LOOP
            EXECUTE format('DROP TABLE IF EXISTS %I CASCADE', r.tablename);
          END LOOP;
        END $$;
        "#,
    )
    .execute(&pool)
    .await;

    // Create old schema (50 columns total: UUID PK + 49 ints).
    sqlx::query(
        r#"
        CREATE TABLE orsx2_big_uuid (
          id UUID PRIMARY KEY,
          c01 INTEGER NOT NULL,
          c02 INTEGER NOT NULL,
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
    )
    .execute(&pool)
    .await
    .unwrap();

    println!("seeding {rows} rows...");
    let seed_start = std::time::Instant::now();
    sqlx::query(
        r#"
        INSERT INTO orsx2_big_uuid (
          id,
          c01,c02,c03,c04,c05,c06,c07,c08,c09,c10,
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
    )
    .bind(rows as i64)
    .execute(&pool)
    .await
    .unwrap();
    println!("seed done in {:?}", seed_start.elapsed());

    // Concurrent writes while migration runs.
    let pool2 = pool.clone();
    let insert_task = tokio::spawn(async move {
        let mut inserted: u64 = 0;
        while inserted < writer_rows_total {
            let batch = std::cmp::min(writer_batch, writer_rows_total - inserted);
            let res = sqlx::query(
                r#"
                INSERT INTO orsx2_big_uuid (
                  id,
                  c01,c02,c03,c04,c05,c06,c07,c08,c09,c10,
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
            )
            .bind(batch as i64)
            .execute(&pool2)
            .await;
            match res {
                Ok(_) => inserted = inserted.saturating_add(batch),
                Err(_) => break,
            }
        }
        inserted
    });

    let pool3 = pool.clone();
    let update_task = tokio::spawn(async move {
        if update_rows_total == 0 || update_batch == 0 {
            return 0u64;
        }
        let mut updated_total: u64 = 0;
        let mut last_pk = Uuid::nil();
        while updated_total < update_rows_total {
            let batch = std::cmp::min(update_batch, update_rows_total - updated_total);
            let max_id_res = sqlx::query_scalar(
                r#"
                SELECT id FROM (
                  SELECT id FROM orsx2_big_uuid
                  WHERE id > $1
                  ORDER BY id
                  LIMIT $2
                ) s
                ORDER BY id DESC
                LIMIT 1
                "#,
            )
            .bind(last_pk)
            .bind(batch as i64)
            .fetch_one(&pool3)
            .await;
            let max_id: Option<Uuid> = match max_id_res {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("update_task: last-id query failed: {e}");
                    break;
                }
            };
            let Some(next_last) = max_id else {
                // No more rows in keyset range.
                break;
            };

            let res = sqlx::query(
                r#"
                WITH target AS (
                  SELECT id FROM orsx2_big_uuid
                  WHERE id > $1
                  ORDER BY id
                  LIMIT $2
                )
                UPDATE orsx2_big_uuid t
                SET c01 = t.c01 + 1
                FROM target
                WHERE t.id = target.id
                "#,
            )
            .bind(last_pk)
            .bind(batch as i64)
            .execute(&pool3)
            .await;
            match res {
                Ok(done) => {
                    if updated_total == 0 {
                        println!(
                            "update_task: first batch rows_affected={} next_last={}",
                            done.rows_affected(),
                            next_last
                        );
                    }
                    updated_total = updated_total.saturating_add(done.rows_affected() as u64);
                    last_pk = next_last;
                }
                Err(e) => {
                    eprintln!("update_task: update query failed: {e}");
                    break;
                }
            }
        }
        updated_total
    });

    let pool4 = pool.clone();
    let delete_task = tokio::spawn(async move {
        if delete_rows_total == 0 || delete_batch == 0 {
            return 0u64;
        }
        let mut deleted_total: u64 = 0;
        let mut last_pk = Uuid::nil();
        while deleted_total < delete_rows_total {
            let batch = std::cmp::min(delete_batch, delete_rows_total - deleted_total);
            let max_id_res = sqlx::query_scalar(
                r#"
                SELECT id FROM (
                  SELECT id FROM orsx2_big_uuid
                  WHERE id > $1
                  ORDER BY id
                  LIMIT $2
                ) s
                ORDER BY id DESC
                LIMIT 1
                "#,
            )
            .bind(last_pk)
            .bind(batch as i64)
            .fetch_one(&pool4)
            .await;
            let max_id: Option<Uuid> = match max_id_res {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("delete_task: last-id query failed: {e}");
                    break;
                }
            };
            let Some(next_last) = max_id else { break };

            let res = sqlx::query(
                r#"
                WITH target AS (
                  SELECT id FROM orsx2_big_uuid
                  WHERE id > $1
                  ORDER BY id
                  LIMIT $2
                )
                DELETE FROM orsx2_big_uuid t
                USING target
                WHERE t.id = target.id
                "#,
            )
            .bind(last_pk)
            .bind(batch as i64)
            .execute(&pool4)
            .await;
            match res {
                Ok(done) => {
                    if deleted_total == 0 {
                        println!(
                            "delete_task: first batch rows_affected={} next_last={}",
                            done.rows_affected(),
                            next_last
                        );
                    }
                    deleted_total = deleted_total.saturating_add(done.rows_affected() as u64);
                    last_pk = next_last;
                }
                Err(e) => {
                    eprintln!("delete_task: delete query failed: {e}");
                    break;
                }
            }
        }
        deleted_total
    });

    let cfg = cfg_from_env(MigrationConfig {
        online_chunk_size: 20_000,
        online_sleep_ms: 0,
        max_online_catchup_rounds: 500,
        cutover_lock_budget_ms: 5_000,
        ..MigrationConfig::default()
    });

    println!(
        "cfg: adaptive_chunk={} chunk_size={} min={} max={} target_ms={} max_ms={}",
        cfg.adaptive_chunk,
        cfg.online_chunk_size,
        cfg.online_chunk_size_min,
        cfg.online_chunk_size_max,
        cfg.online_target_batch_ms,
        cfg.online_max_batch_ms
    );

    let dummy = V2 {
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
        new_col: 0,
    };

    println!("starting migration (online rewrite)...");
    let mig_start = std::time::Instant::now();
    Migrations::init_with_config(&pool, &[(dummy, None)], &cfg)
        .await
        .unwrap();
    println!("migration done in {:?}", mig_start.elapsed());

    let inserted = insert_task.await.unwrap();
    let updated = update_task.await.unwrap();
    let deleted = delete_task.await.unwrap();
    println!(
        "writer summary: inserted={inserted} updated={updated} deleted={deleted}"
    );

    // Verify: column exists and is not null.
    let nulls: (i64,) =
        sqlx::query_as("SELECT COUNT(*)::BIGINT FROM orsx2_big_uuid WHERE new_col IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(nulls.0, 0);

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*)::BIGINT FROM orsx2_big_uuid")
        .fetch_one(&pool)
        .await
        .unwrap();
    println!("final rowcount: {}", total.0);
    let expected_min = (rows as i64)
        .saturating_add(inserted as i64)
        .saturating_sub(deleted as i64);
    assert!(
        total.0 >= expected_min,
        "expected at least {expected_min} rows after deletes"
    );
}
