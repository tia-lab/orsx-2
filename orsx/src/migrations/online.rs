use crate::migrations::config::MigrationConfig;
use crate::migrations::introspection::{ColumnInfo, TableSchema};
use crate::schema::TableSpec;
use crate::{quote_identifier, Error, Result};
use sqlx::PgPool;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
struct CutoverStats {
    lock_ms: u64,
}

async fn maybe_set_synchronous_commit_off(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    cfg: &MigrationConfig,
) -> Result<Option<String>> {
    if !cfg.synchronous_commit_off {
        return Ok(None);
    }
    let prev: String = sqlx::query_scalar("SHOW synchronous_commit")
        .fetch_one(&mut **conn)
        .await?;
    sqlx::query("SET synchronous_commit TO off")
        .execute(&mut **conn)
        .await?;
    Ok(Some(prev))
}

async fn restore_synchronous_commit(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    prev: Option<String>,
) {
    if let Some(prev) = prev {
        let _ = sqlx::query("SET synchronous_commit TO $1")
            .bind(prev)
            .execute(&mut **conn)
            .await;
    }
}

#[derive(Debug, Clone, Copy)]
struct AdaptiveChunk {
    enabled: bool,
    min: i64,
    max: i64,
    target_ms: u64,
    max_ms: u64,
}

impl AdaptiveChunk {
    fn from_cfg(cfg: &MigrationConfig) -> Result<Self> {
        let mut min = cfg.online_chunk_size_min;
        let mut max = cfg.online_chunk_size_max;
        if min <= 0 || max <= 0 {
            return Err(Error::Other(
                "online chunk size bounds must be positive".to_string(),
            ));
        }
        if min > max {
            std::mem::swap(&mut min, &mut max);
        }
        Ok(Self {
            enabled: cfg.adaptive_chunk,
            min,
            max,
            target_ms: cfg.online_target_batch_ms.max(1),
            max_ms: cfg.online_max_batch_ms.max(cfg.online_target_batch_ms.max(1)),
        })
    }

    fn clamp_size(&self, v: i64) -> i64 {
        v.clamp(self.min, self.max)
    }

    fn adjust(&self, current: i64, batch_ms: u64) -> i64 {
        if !self.enabled {
            return current;
        }
        let current = self.clamp_size(current);
        let batch_ms = batch_ms.max(1);
        if batch_ms >= self.max_ms {
            return self.clamp_size(current.saturating_div(2).max(1));
        }

        // Use a proportional controller with a deadband around the target to avoid oscillation.
        //
        // Why not doubling/halving? On large tables, bigger batches can quickly hit WAL/IO ceilings,
        // and oscillation wastes time. This converges smoothly by scaling towards target_ms but
        // limits per-step change.
        let low = (self.target_ms * 85) / 100;
        let high = (self.target_ms * 115) / 100;
        if (low..=high).contains(&batch_ms) {
            return current;
        }

        let target = self.target_ms as i128;
        let batch = batch_ms as i128;
        let cur = current as i128;

        // Ideal proportional size: current * target / batch.
        let mut next = (cur.saturating_mul(target)).saturating_div(batch).max(1);

        // Limit per-step change to keep behavior stable.
        let min_step = (cur * 2) / 3; // -33%
        let max_step = (cur * 3) / 2; // +50%
        if next < min_step {
            next = min_step.max(1);
        } else if next > max_step {
            next = max_step.max(1);
        }

        self.clamp_size(next as i64)
    }
}

pub async fn online_rewrite_table(
    pool: &PgPool,
    table_name: &str,
    spec: &TableSpec,
    current: &TableSchema,
    _expected: &TableSchema,
    cfg: &MigrationConfig,
) -> Result<()> {
    let total_start = Instant::now();

    // Phase 0: validate that online migration prerequisites are satisfied.
    let pk = primary_key_column(spec).ok_or_else(|| {
        Error::MigrationNeeded("online rewrite requires exactly one primary key column".to_string())
    })?;

    let current_pk = current
        .columns
        .iter()
        .find(|c| c.name == pk.name)
        .ok_or_else(|| Error::MigrationNeeded("primary key missing in current schema".to_string()))?;

    // Determine whether we can generate safe expressions for all expected columns.
    let col_exprs = build_column_exprs(current, spec)?;

    // Stable names.
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::Other("system time error".to_string()))?
        .as_secs();

    // Derived object names must stay under Postgres' 63-byte identifier limit.
    let shadow = derived_object_name(table_name, "shadow", ts);
    let backup = derived_object_name(table_name, "backup", ts);
    let changelog = derived_object_name(table_name, "changelog", ts);
    let trigger_fn = derived_object_name(table_name, "mirror_fn", ts);
    let trigger_name = derived_object_name(table_name, "mirror_trg", ts);

    // Phase 1: create shadow table with expected schema.
    let create_sql = {
        // Generate CREATE TABLE for shadow using the spec.
        let mut lines: Vec<String> = Vec::with_capacity(spec.columns.len() + 4);
        for col in spec.columns {
            let mut line = format!(
                "{} {}",
                quote_identifier(col.name),
                col.ty.to_sql()
            );
            if let Some(default_sql) = col.default_sql {
                line.push_str(" DEFAULT ");
                line.push_str(default_sql);
            }
            if col.primary_key {
                line.push_str(" PRIMARY KEY");
            }
            if col.unique && !col.primary_key {
                line.push_str(" UNIQUE");
            }
            if !col.nullable && !col.primary_key {
                line.push_str(" NOT NULL");
            }
            lines.push(line);
        }
        format!(
            "CREATE TABLE IF NOT EXISTS {} (\n  {}\n)",
            quote_identifier(&shadow),
            lines.join(",\n  ")
        )
    };

    sqlx::query(&create_sql).execute(pool).await?;

    // Phase 2: create changelog table and trigger that records changed PKs.
    create_changelog(pool, &changelog, current_pk).await?;
    create_mirror_trigger(
        pool,
        &trigger_fn,
        &trigger_name,
        table_name,
        &changelog,
        pk.name,
        &col_exprs,
        current,
    )
    .await?;

    // Phase 3: backfill in chunks while triggers mirror live writes.
    let backfill_start = Instant::now();
    let backfill_rows = {
        let mut conn = pool.acquire().await?;
        let prev = maybe_set_synchronous_commit_off(&mut conn, cfg).await?;
        let out = backfill(
            pool,
            &mut conn,
            table_name,
            &shadow,
            pk.name,
            &current_pk.sql_type,
            &col_exprs,
            cfg,
        )
        .await;
        restore_synchronous_commit(&mut conn, prev).await;
        out?
    };
    let backfill_ms = backfill_start.elapsed().as_millis() as u64;

    // Phase 4: catch up changes by reading changelog and applying to the shadow table.
    let catchup_start = Instant::now();
    let catchup_drained = {
        let mut conn = pool.acquire().await?;
        let prev = maybe_set_synchronous_commit_off(&mut conn, cfg).await?;
        let out = catch_up_best_effort(
            &mut conn,
            table_name,
            &shadow,
            &changelog,
            pk.name,
            &current_pk.sql_type,
            &col_exprs,
            cfg,
        )
        .await;
        restore_synchronous_commit(&mut conn, prev).await;
        out?
    };
    let catchup_ms = catchup_start.elapsed().as_millis() as u64;

    // Phase 5: cutover (short lock): disable trigger, final drain, swap, cleanup.
    let cutover_stats = cutover(
        pool,
        table_name,
        &shadow,
        &backup,
        &changelog,
        &trigger_name,
        &trigger_fn,
        pk.name,
        &current_pk.sql_type,
        &col_exprs,
        cfg,
    )
    .await?;

    // Ensure declared indexes exist on the new live table. Use CONCURRENTLY to keep writes flowing.
    super::planning::ensure_indexes_concurrently(pool, table_name, spec).await?;

    let total_ms = total_start.elapsed().as_millis() as u64;
    tracing::info!(
        table = table_name,
        shadow = shadow,
        backup = backup,
        total_ms = total_ms,
        backfill_ms = backfill_ms,
        backfill_rows = backfill_rows,
        catchup_ms = catchup_ms,
        catchup_drained_pk = catchup_drained,
        cutover_lock_ms = cutover_stats.lock_ms,
        "online rewrite completed"
    );

    Ok(())
}

fn primary_key_column(spec: &TableSpec) -> Option<&'static crate::schema::ColumnSpec> {
    let mut pk = None;
    for c in spec.columns {
        if c.primary_key {
            if pk.is_some() {
                return None;
            }
            pk = Some(c);
        }
    }
    pk
}

fn build_column_exprs(
    current: &TableSchema,
    spec: &TableSpec,
) -> Result<Vec<(&'static str, String)>> {
    let current_cols: std::collections::HashSet<&str> =
        current.columns.iter().map(|c| c.name.as_str()).collect();

    let mut out = Vec::with_capacity(spec.columns.len());

    for col in spec.columns {
        let expr = if current_cols.contains(col.name) {
            // Use OLD/NEW qualified name for triggers; backfill uses source alias `src`.
            // We'll substitute prefixes at call sites.
            quote_identifier(col.name)
        } else if let Some(from) = col.rename_from {
            if current_cols.contains(from) {
                quote_identifier(from)
            } else {
                // Fall through to defaults/nullability rules.
                if let Some(default_sql) = col.default_sql {
                    default_sql.to_string()
                } else if col.nullable {
                    "NULL".to_string()
                } else {
                    return Err(Error::MigrationNeeded(format!(
                        "new NOT NULL column '{}' requires default_sql for online rewrite",
                        col.name
                    )));
                }
            }
        } else if let Some(default_sql) = col.default_sql {
            default_sql.to_string()
        } else if col.nullable {
            "NULL".to_string()
        } else {
            return Err(Error::MigrationNeeded(format!(
                "new NOT NULL column '{}' requires default_sql for online rewrite",
                col.name
            )));
        };

        out.push((col.name, expr));
    }

    Ok(out)
}

async fn create_changelog(pool: &PgPool, changelog: &str, pk: &ColumnInfo) -> Result<()> {
    let sql = format!(
        "CREATE TABLE IF NOT EXISTS {} (pk {} PRIMARY KEY)",
        quote_identifier(changelog),
        pk.sql_type
    );
    sqlx::query(&sql).execute(pool).await?;
    Ok(())
}

async fn create_mirror_trigger(
    pool: &PgPool,
    trigger_fn: &str,
    trigger_name: &str,
    source_table: &str,
    changelog: &str,
    pk_name: &'static str,
    _col_exprs: &[(&'static str, String)],
    _current: &TableSchema,
) -> Result<()> {
    let pk_q = quote_identifier(pk_name);

    let fn_sql = format!(
        r#"
        CREATE OR REPLACE FUNCTION {fn_ident}() RETURNS trigger AS $$
        BEGIN
          IF (TG_OP = 'DELETE') THEN
            INSERT INTO {changelog} (pk) VALUES (OLD.{pk_q})
              ON CONFLICT (pk) DO NOTHING;
            RETURN OLD;
          ELSE
            INSERT INTO {changelog} (pk) VALUES (NEW.{pk_q})
              ON CONFLICT (pk) DO NOTHING;
            RETURN NEW;
          END IF;
        END;
        $$ LANGUAGE plpgsql;
        "#,
        fn_ident = quote_identifier(trigger_fn),
        changelog = quote_identifier(changelog),
        pk_q = pk_q,
    );

    sqlx::query(&fn_sql).execute(pool).await?;

    let drop_trigger = format!(
        "DROP TRIGGER IF EXISTS {} ON {}",
        quote_identifier(trigger_name),
        quote_identifier(source_table)
    );
    sqlx::query(&drop_trigger).execute(pool).await?;

    let trg_sql = format!(
        "CREATE TRIGGER {} AFTER INSERT OR UPDATE OR DELETE ON {} FOR EACH ROW EXECUTE FUNCTION {}()",
        quote_identifier(trigger_name),
        quote_identifier(source_table),
        quote_identifier(trigger_fn),
    );
    sqlx::query(&trg_sql).execute(pool).await?;

    Ok(())
}

async fn backfill(
    pool: &PgPool,
    conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    source: &str,
    shadow: &str,
    pk: &'static str,
    pk_sql_type: &str,
    col_exprs: &[(&'static str, String)],
    cfg: &MigrationConfig,
) -> Result<u64> {
    let pk_q = quote_identifier(pk);
    let pk_cast = cast_type_for_param(pk_sql_type)?;

    if cfg.parallel_backfill && is_parallel_backfill_pk(pk_sql_type) {
        return parallel_backfill_bigint(
            pool,
            source,
            shadow,
            pk,
            pk_sql_type,
            col_exprs,
            cfg,
        )
        .await;
    }

    backfill_sequential(conn, source, shadow, &pk_q, pk_cast, col_exprs, cfg).await
}

async fn backfill_sequential(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    source: &str,
    shadow: &str,
    pk_q: &str,
    pk_cast: &str,
    col_exprs: &[(&'static str, String)],
    cfg: &MigrationConfig,
) -> Result<u64> {
    let mut total_rows: u64 = 0;
    let mut last_pk: Option<String> = None;

    let columns = col_exprs
        .iter()
        .map(|(c, _)| quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", ");

    // Build SELECT list from source alias `src`, with deterministic aliases matching target columns.
    let moved_select = col_exprs
        .iter()
        .map(|(c, expr)| {
            let expr_sql = if expr == "NULL" {
                "NULL".to_string()
            } else if expr.starts_with('"') {
                format!("src.{expr}")
            } else {
                expr.clone()
            };
            format!("{expr_sql} AS {}", quote_identifier(c))
        })
        .collect::<Vec<_>>()
        .join(", ");

    // Single round-trip per batch:
    // - pick the next chunk using keyset pagination on the PK index
    // - insert from the materialized chunk
    // - return the last PK as text + moved rowcount
    let backfill_initial = format!(
        r#"
        WITH moved AS MATERIALIZED (
          SELECT {moved_select}
          FROM {src} AS src
          ORDER BY src.{pk_q}
          LIMIT $1
        ),
        _ins AS (
          INSERT INTO {shadow} ({columns})
          SELECT {columns} FROM moved
          ON CONFLICT ({pk_q}) DO NOTHING
        )
        SELECT
          (SELECT {pk_q}::text FROM moved ORDER BY {pk_q} DESC LIMIT 1) AS last_pk,
          (SELECT COUNT(*)::bigint FROM moved) AS moved_rows
        "#,
        moved_select = moved_select,
        src = quote_identifier(source),
        shadow = quote_identifier(shadow),
        columns = columns,
        pk_q = pk_q,
    );

    let backfill_next = format!(
        r#"
        WITH moved AS MATERIALIZED (
          SELECT {moved_select}
          FROM {src} AS src
          WHERE src.{pk_q} > ($1::{pk_cast})
          ORDER BY src.{pk_q}
          LIMIT $2
        ),
        _ins AS (
          INSERT INTO {shadow} ({columns})
          SELECT {columns} FROM moved
          ON CONFLICT ({pk_q}) DO NOTHING
        )
        SELECT
          (SELECT {pk_q}::text FROM moved ORDER BY {pk_q} DESC LIMIT 1) AS last_pk,
          (SELECT COUNT(*)::bigint FROM moved) AS moved_rows
        "#,
        moved_select = moved_select,
        src = quote_identifier(source),
        shadow = quote_identifier(shadow),
        columns = columns,
        pk_q = pk_q,
        pk_cast = pk_cast,
    );

    // Backfill is the dominant cost for large tables and is typically stable.
    // Keeping a fixed chunk size avoids oscillation and makes perf easier to reason about.
    let chunk_size = cfg.online_chunk_size.max(1);
    loop {
        let (next_last, moved_rows): (Option<String>, i64) = if let Some(ref lp) = last_pk {
            sqlx::query_as(&backfill_next)
                .bind(lp)
                .bind(chunk_size)
                .fetch_one(&mut **conn)
                .await?
        } else {
            sqlx::query_as(&backfill_initial)
                .bind(chunk_size)
                .fetch_one(&mut **conn)
                .await?
        };

        let Some(next_last) = next_last else { break };
        if moved_rows <= 0 {
            break;
        }

        total_rows = total_rows.saturating_add(moved_rows as u64);
        last_pk = Some(next_last);

        if cfg.online_sleep_ms > 0 {
            tokio::time::sleep(Duration::from_millis(cfg.online_sleep_ms)).await;
        }
    }

    Ok(total_rows)
}

async fn catch_up_best_effort(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    source: &str,
    shadow: &str,
    changelog: &str,
    pk: &'static str,
    pk_sql_type: &str,
    col_exprs: &[(&'static str, String)],
    cfg: &MigrationConfig,
) -> Result<u64> {
    let adaptive = AdaptiveChunk::from_cfg(cfg)?;
    let pk_q = quote_identifier(pk);
    let pk_cast = cast_type_for_param(pk_sql_type)?;
    let changelog_q = quote_identifier(changelog);
    let columns = col_exprs
        .iter()
        .map(|(c, _)| quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", ");

    let values = col_exprs
        .iter()
        .map(|(_c, expr)| {
            if expr == "NULL" {
                "NULL".to_string()
            } else if expr.starts_with('"') {
                format!("src.{expr}")
            } else {
                expr.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    let set_parts = col_exprs
        .iter()
        .filter(|(c, _)| *c != pk)
        .map(|(c, _)| {
            let q = quote_identifier(c);
            format!("{q} = EXCLUDED.{q}")
        })
        .collect::<Vec<_>>()
        .join(", ");

    // Range boundary selection from changelog (order by typed pk, return boundary as text).
    // This keeps `DELETE FROM changelog` range-based (fast) and avoids heavy `RETURNING` work.
    let select_last_initial = format!(
        "SELECT pk::text FROM (SELECT pk FROM {changelog} ORDER BY pk LIMIT $1) s ORDER BY pk DESC LIMIT 1",
        changelog = changelog_q
    );
    let select_last_next = format!(
        "SELECT pk::text FROM (SELECT pk FROM {changelog} WHERE pk > ($1::{pk_cast}) ORDER BY pk LIMIT $2) s ORDER BY pk DESC LIMIT 1",
        changelog = changelog_q,
        pk_cast = pk_cast,
    );

    let mut drained_total: u64 = 0;
    let mut last_pk: Option<String> = None;
    let mut chunk_size = adaptive.clamp_size(cfg.online_chunk_size);
    for _round in 0..cfg.max_online_catchup_rounds {
        let next_last: Option<String> = if let Some(ref lp) = last_pk {
            sqlx::query_scalar(&select_last_next)
                .bind(lp)
                .bind(chunk_size)
                .fetch_optional(&mut **conn)
                .await?
        } else {
            sqlx::query_scalar(&select_last_initial)
                .bind(chunk_size)
                .fetch_optional(&mut **conn)
                .await?
        };

        let Some(next_last) = next_last else {
            return Ok(drained_total);
        };

        let upsert = if last_pk.is_some() {
            format!(
                "INSERT INTO {shadow} ({columns}) \
                 SELECT {values} \
                   FROM {src} AS src \
                   JOIN {changelog} c ON c.pk = src.{pk_q} \
                  WHERE c.pk > ($1::{pk_cast}) AND c.pk <= ($2::{pk_cast}) \
                 {on_conflict}",
                shadow = quote_identifier(shadow),
                src = quote_identifier(source),
                changelog = changelog_q,
                columns = columns,
                values = values,
                pk_q = pk_q,
                pk_cast = pk_cast,
                on_conflict = on_conflict_clause(&pk_q, &set_parts),
            )
        } else {
            format!(
                "INSERT INTO {shadow} ({columns}) \
                 SELECT {values} \
                   FROM {src} AS src \
                   JOIN {changelog} c ON c.pk = src.{pk_q} \
                  WHERE c.pk <= ($1::{pk_cast}) \
                 {on_conflict}",
                shadow = quote_identifier(shadow),
                src = quote_identifier(source),
                changelog = changelog_q,
                columns = columns,
                values = values,
                pk_q = pk_q,
                pk_cast = pk_cast,
                on_conflict = on_conflict_clause(&pk_q, &set_parts),
            )
        };

        let batch_start = Instant::now();
        if let Some(ref lp) = last_pk {
            sqlx::query(&upsert)
                .bind(lp)
                .bind(&next_last)
                .execute(&mut **conn)
                .await?;
        } else {
            sqlx::query(&upsert)
                .bind(&next_last)
                .execute(&mut **conn)
                .await?;
        }

        let delete_missing = if last_pk.is_some() {
            format!(
                "DELETE FROM {shadow} s \
                  USING {changelog} c \
                 WHERE c.pk = s.{pk_q} \
                   AND c.pk > ($1::{pk_cast}) AND c.pk <= ($2::{pk_cast}) \
                   AND NOT EXISTS (SELECT 1 FROM {src} o WHERE o.{pk_q} = s.{pk_q})",
                shadow = quote_identifier(shadow),
                changelog = changelog_q,
                src = quote_identifier(source),
                pk_q = pk_q,
                pk_cast = pk_cast,
            )
        } else {
            format!(
                "DELETE FROM {shadow} s \
                  USING {changelog} c \
                 WHERE c.pk = s.{pk_q} \
                   AND c.pk <= ($1::{pk_cast}) \
                   AND NOT EXISTS (SELECT 1 FROM {src} o WHERE o.{pk_q} = s.{pk_q})",
                shadow = quote_identifier(shadow),
                changelog = changelog_q,
                src = quote_identifier(source),
                pk_q = pk_q,
                pk_cast = pk_cast,
            )
        };

        if let Some(ref lp) = last_pk {
            sqlx::query(&delete_missing)
                .bind(lp)
                .bind(&next_last)
                .execute(&mut **conn)
                .await?;
        } else {
            sqlx::query(&delete_missing)
                .bind(&next_last)
                .execute(&mut **conn)
                .await?;
        }

        let clear = if last_pk.is_some() {
            format!(
                "DELETE FROM {changelog} WHERE pk > ($1::{pk_cast}) AND pk <= ($2::{pk_cast})",
                changelog = changelog_q,
                pk_cast = pk_cast
            )
        } else {
            format!(
                "DELETE FROM {changelog} WHERE pk <= ($1::{pk_cast})",
                changelog = changelog_q,
                pk_cast = pk_cast
            )
        };

        let cleared = if let Some(ref lp) = last_pk {
            sqlx::query(&clear)
                .bind(lp)
                .bind(&next_last)
                .execute(&mut **conn)
                .await?
                .rows_affected()
        } else {
            sqlx::query(&clear)
                .bind(&next_last)
                .execute(&mut **conn)
                .await?
                .rows_affected()
        };
        let batch_ms = batch_start.elapsed().as_millis() as u64;

        drained_total = drained_total.saturating_add(cleared as u64);
        last_pk = Some(next_last);
        chunk_size = adaptive.adjust(chunk_size, batch_ms);

        if cfg.online_sleep_ms > 0 {
            tokio::time::sleep(Duration::from_millis(cfg.online_sleep_ms)).await;
        }
    }

    // Under ongoing writes, changelog may not reach zero. This is expected; final drain
    // happens inside cutover after triggers are disabled.
    Ok(drained_total)
}

async fn cutover(
    pool: &PgPool,
    source: &str,
    shadow: &str,
    backup: &str,
    changelog: &str,
    trigger_name: &str,
    trigger_fn: &str,
    pk: &'static str,
    pk_sql_type: &str,
    col_exprs: &[(&'static str, String)],
    cfg: &MigrationConfig,
) -> Result<CutoverStats> {
    // Cutover lock.
    let mut tx = pool.begin().await?;
    let lock_start = Instant::now();

    sqlx::query(&format!(
        "LOCK TABLE {} IN ACCESS EXCLUSIVE MODE",
        quote_identifier(source)
    ))
    .execute(tx.as_mut())
    .await?;

    // Drop the mirror trigger to stop recording changes.
    sqlx::query(&format!(
        "DROP TRIGGER IF EXISTS {} ON {}",
        quote_identifier(trigger_name),
        quote_identifier(source)
    ))
    .execute(tx.as_mut())
    .await?;

    // Final drain inside lock (trigger dropped, so it must converge).
    let pk_q = quote_identifier(pk);
    let pk_cast = cast_type_for_param(pk_sql_type)?;
    let changelog_q = quote_identifier(changelog);

    let columns = col_exprs
        .iter()
        .map(|(c, _)| quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", ");

    let values = col_exprs
        .iter()
        .map(|(_c, expr)| {
            if expr == "NULL" {
                "NULL".to_string()
            } else if expr.starts_with('"') {
                format!("src.{expr}")
            } else {
                expr.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    let set_parts = col_exprs
        .iter()
        .filter(|(c, _)| *c != pk)
        .map(|(c, _)| {
            let q = quote_identifier(c);
            format!("{q} = EXCLUDED.{q}")
        })
        .collect::<Vec<_>>()
        .join(", ");

    let select_last_initial = format!(
        "SELECT pk::text FROM (SELECT pk FROM {changelog} ORDER BY pk LIMIT $1) s ORDER BY pk DESC LIMIT 1",
        changelog = changelog_q
    );

    for _round in 0..cfg.max_online_catchup_rounds {
        let next_last: Option<String> = sqlx::query_scalar(&select_last_initial)
            .bind(cfg.online_chunk_size)
            .fetch_optional(tx.as_mut())
            .await?;
        let Some(next_last) = next_last else {
            break;
        };

        let upsert = format!(
            "INSERT INTO {shadow} ({columns}) \
             SELECT {values} \
               FROM {src} AS src \
               JOIN {changelog} c ON c.pk = src.{pk_q} \
              WHERE c.pk <= ($1::{pk_cast}) \
             {on_conflict}",
            shadow = quote_identifier(shadow),
            src = quote_identifier(source),
            changelog = changelog_q,
            columns = columns,
            values = values,
            pk_q = pk_q,
            pk_cast = pk_cast,
            on_conflict = on_conflict_clause(&pk_q, &set_parts),
        );
        sqlx::query(&upsert)
            .bind(&next_last)
            .execute(tx.as_mut())
            .await?;

        let delete_missing = format!(
            "DELETE FROM {shadow} s \
              USING {changelog} c \
             WHERE c.pk = s.{pk_q} \
               AND c.pk <= ($1::{pk_cast}) \
               AND NOT EXISTS (SELECT 1 FROM {src} o WHERE o.{pk_q} = s.{pk_q})",
            shadow = quote_identifier(shadow),
            changelog = changelog_q,
            src = quote_identifier(source),
            pk_q = pk_q,
            pk_cast = pk_cast,
        );
        sqlx::query(&delete_missing)
            .bind(&next_last)
            .execute(tx.as_mut())
            .await?;

        let clear = format!(
            "DELETE FROM {changelog} WHERE pk <= ($1::{pk_cast})",
            changelog = changelog_q,
            pk_cast = pk_cast,
        );
        sqlx::query(&clear)
            .bind(&next_last)
            .execute(tx.as_mut())
            .await?;
    }

    // If still not empty, this violates the cutover safety contract.
    let remaining: (i64,) = sqlx::query_as(&format!(
        "SELECT COUNT(*)::BIGINT FROM {}",
        changelog_q
    ))
    .fetch_one(tx.as_mut())
    .await?;
    if remaining.0 != 0 {
        return Err(Error::MigrationNeeded(format!(
            "cutover drain did not converge; remaining changelog rows: {}",
            remaining.0
        )));
    }

    // Swap names.
    sqlx::query(&format!(
        "ALTER TABLE {} RENAME TO {}",
        quote_identifier(source),
        quote_identifier(backup)
    ))
    .execute(tx.as_mut())
    .await?;

    sqlx::query(&format!(
        "ALTER TABLE {} RENAME TO {}",
        quote_identifier(shadow),
        quote_identifier(source)
    ))
    .execute(tx.as_mut())
    .await?;

    // Cleanup trigger function and changelog table.
    sqlx::query(&format!("DROP FUNCTION IF EXISTS {}()", quote_identifier(trigger_fn)))
        .execute(tx.as_mut())
        .await?;
    sqlx::query(&format!("DROP TABLE IF EXISTS {}", quote_identifier(changelog)))
        .execute(tx.as_mut())
        .await?;

    tx.commit().await?;

    let elapsed = lock_start.elapsed();
    if elapsed > Duration::from_millis(cfg.cutover_lock_budget_ms) {
        return Err(Error::MigrationNeeded(format!(
            "cutover exceeded lock budget: {}ms",
            elapsed.as_millis()
        )));
    }

    Ok(CutoverStats {
        lock_ms: elapsed.as_millis() as u64,
    })
}

// NOTE: Prior versions used pk list batches (`ANY($1::uuid[])`). We now use typed ordering with
// keyset/range batching to avoid large allocations and keep Postgres plans index-friendly.

fn cast_type_for_param(pk_sql_type: &str) -> Result<&'static str> {
    match pk_sql_type.to_uppercase().as_str() {
        "TEXT" | "VARCHAR" | "CHARACTER VARYING" => Ok("text"),
        "UUID" => Ok("uuid"),
        "BIGINT" | "INT8" => Ok("bigint"),
        "INTEGER" | "INT4" | "INT" => Ok("integer"),
        other => Err(Error::MigrationNeeded(format!(
            "unsupported primary key type for online rewrite: {other}"
        ))),
    }
}

fn is_parallel_backfill_pk(pk_sql_type: &str) -> bool {
    matches!(pk_sql_type.to_uppercase().as_str(), "BIGINT" | "INT8")
}

async fn parallel_backfill_bigint(
    pool: &PgPool,
    source: &str,
    shadow: &str,
    pk: &'static str,
    pk_sql_type: &str,
    col_exprs: &[(&'static str, String)],
    cfg: &MigrationConfig,
) -> Result<u64> {
    let pk_q = quote_identifier(pk);
    let pk_cast = cast_type_for_param(pk_sql_type)?;
    if pk_cast != "bigint" {
        return Err(Error::MigrationNeeded(
            "parallel backfill is only supported for BIGINT primary keys".to_string(),
        ));
    }

    let (min_pk, max_pk): (Option<i64>, Option<i64>) = sqlx::query_as(&format!(
        "SELECT MIN({pk_q})::bigint, MAX({pk_q})::bigint FROM {src}",
        pk_q = pk_q,
        src = quote_identifier(source),
    ))
    .fetch_one(pool)
    .await?;

    let (Some(min_pk), Some(max_pk)) = (min_pk, max_pk) else {
        return Ok(0);
    };

    let workers = cfg.parallel_backfill_workers.clamp(1, 64);
    if workers <= 1 || min_pk >= max_pk {
        // Fall back to sequential; parallelism doesn't help on tiny/empty ranges.
        let mut conn = pool.acquire().await?;
        let mut cfg2 = cfg.clone();
        cfg2.parallel_backfill = false;
        return backfill_sequential(
            &mut conn,
            source,
            shadow,
            &pk_q,
            "bigint",
            col_exprs,
            &cfg2,
        )
        .await;
    }

    let span = (max_pk - min_pk).saturating_add(1);
    let step = (span + workers as i64 - 1) / workers as i64; // ceil

    let columns = col_exprs
        .iter()
        .map(|(c, _)| quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", ");

    let moved_select = col_exprs
        .iter()
        .map(|(c, expr)| {
            let expr_sql = if expr == "NULL" {
                "NULL".to_string()
            } else if expr.starts_with('"') {
                format!("src.{expr}")
            } else {
                expr.clone()
            };
            format!("{expr_sql} AS {}", quote_identifier(c))
        })
        .collect::<Vec<_>>()
        .join(", ");

    let chunk_size = cfg.online_chunk_size.max(1);

    let mut join_set: tokio::task::JoinSet<Result<u64>> = tokio::task::JoinSet::new();
    for i in 0..workers {
        let lo = min_pk.saturating_add(step.saturating_mul(i as i64));
        let mut hi = lo.saturating_add(step).saturating_sub(1);
        if hi > max_pk {
            hi = max_pk;
        }
        if lo > hi {
            continue;
        }

        let source = source.to_string();
        let shadow = shadow.to_string();
        let pk_q = pk_q.clone();
        let columns = columns.clone();
        let moved_select = moved_select.clone();
        let cfg = cfg.clone();
        let pool = pool.clone();

        join_set.spawn(async move {
            let initial_sql = format!(
                r#"
                WITH moved AS MATERIALIZED (
                  SELECT {moved_select}
                  FROM {src} AS src
                  WHERE src.{pk_q} >= ($1::bigint) AND src.{pk_q} <= ($2::bigint)
                  ORDER BY src.{pk_q}
                  LIMIT $3
                ),
                _ins AS (
                  INSERT INTO {shadow} ({columns})
                  SELECT {columns} FROM moved
                  ON CONFLICT ({pk_q}) DO NOTHING
                )
                SELECT
                  (SELECT {pk_q}::bigint FROM moved ORDER BY {pk_q} DESC LIMIT 1) AS last_pk,
                  (SELECT COUNT(*)::bigint FROM moved) AS moved_rows
                "#,
                moved_select = moved_select,
                src = quote_identifier(&source),
                shadow = quote_identifier(&shadow),
                columns = columns,
                pk_q = pk_q,
            );

            let next_sql = format!(
                r#"
                WITH moved AS MATERIALIZED (
                  SELECT {moved_select}
                  FROM {src} AS src
                  WHERE src.{pk_q} > ($1::bigint) AND src.{pk_q} <= ($2::bigint)
                  ORDER BY src.{pk_q}
                  LIMIT $3
                ),
                _ins AS (
                  INSERT INTO {shadow} ({columns})
                  SELECT {columns} FROM moved
                  ON CONFLICT ({pk_q}) DO NOTHING
                )
                SELECT
                  (SELECT {pk_q}::bigint FROM moved ORDER BY {pk_q} DESC LIMIT 1) AS last_pk,
                  (SELECT COUNT(*)::bigint FROM moved) AS moved_rows
                "#,
                moved_select = moved_select,
                src = quote_identifier(&source),
                shadow = quote_identifier(&shadow),
                columns = columns,
                pk_q = pk_q,
            );

            let mut moved_total: u64 = 0;
            let mut last: Option<i64> = None;

            let mut conn = pool.acquire().await?;
            let prev = maybe_set_synchronous_commit_off(&mut conn, &cfg).await?;

            let res: Result<u64> = async {
                loop {
                    let (next_last, moved_rows): (Option<i64>, i64) = if let Some(lp) = last {
                        sqlx::query_as(&next_sql)
                            .bind(lp)
                            .bind(hi)
                            .bind(chunk_size)
                            .fetch_one(&mut *conn)
                            .await?
                    } else {
                        sqlx::query_as(&initial_sql)
                            .bind(lo)
                            .bind(hi)
                            .bind(chunk_size)
                            .fetch_one(&mut *conn)
                            .await?
                    };

                    let Some(next_last) = next_last else { break };
                    if moved_rows <= 0 {
                        break;
                    }
                    moved_total = moved_total.saturating_add(moved_rows as u64);
                    last = Some(next_last);

                    if cfg.online_sleep_ms > 0 {
                        tokio::time::sleep(Duration::from_millis(cfg.online_sleep_ms)).await;
                    }
                }
                Ok(moved_total)
            }
            .await;

            restore_synchronous_commit(&mut conn, prev).await;
            res
        });
    }

    let mut total: u64 = 0;
    while let Some(res) = join_set.join_next().await {
        let r = res.map_err(|e| Error::Other(format!("parallel backfill join error: {e}")))?;
        let v = r?;
        total = total.saturating_add(v);
    }
    Ok(total)
}

fn on_conflict_clause(pk_q: &str, set_parts: &str) -> String {
    if set_parts.trim().is_empty() {
        format!("ON CONFLICT ({pk_q}) DO NOTHING")
    } else {
        format!("ON CONFLICT ({pk_q}) DO UPDATE SET {set_parts}")
    }
}

fn derived_object_name(base: &str, kind: &str, ts: u64) -> String {
    // Keep suffix stable and readable.
    let suffix = format!("__orsx2_{kind}_{ts}");
    if base.len() + suffix.len() <= 63 {
        return format!("{base}{suffix}");
    }

    // Shorten the base name while keeping uniqueness via a stable hash of the full base.
    let hash = crc32fast::hash(base.as_bytes());
    let hash_str = format!("{hash:08x}");
    let suffix = format!("__orsx2_{kind}_{ts}_{hash_str}");

    // Leave as much of the base as possible while staying under the limit.
    let max_base_len = 63usize.saturating_sub(suffix.len());
    let truncated = if base.len() <= max_base_len {
        base.to_string()
    } else {
        // Identifiers are ASCII in our expected usage; truncate on char boundary anyway.
        base.chars().take(max_base_len).collect::<String>()
    };
    format!("{truncated}{suffix}")
}

// Index creation logic is centralized in `planning::ensure_indexes_concurrently`.
