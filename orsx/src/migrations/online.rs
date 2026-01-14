use crate::migrations::config::MigrationConfig;
use crate::migrations::introspection::{ColumnInfo, TableSchema};
use crate::schema::TableSpec;
use crate::{quote_identifier, Error, Result};
use sqlx::PgPool;
use std::time::{Duration, Instant};

pub async fn online_rewrite_table(
    pool: &PgPool,
    table_name: &str,
    spec: &TableSpec,
    current: &TableSchema,
    _expected: &TableSchema,
    cfg: &MigrationConfig,
) -> Result<()> {
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

    let shadow = format!("{table_name}__orsx2_shadow_{ts}");
    let backup = format!("{table_name}__orsx2_backup_{ts}");
    let changelog = format!("{table_name}__orsx2_changelog_{ts}");
    let trigger_fn = format!("{table_name}__orsx2_mirror_fn_{ts}");
    let trigger_name = format!("{table_name}__orsx2_mirror_trg_{ts}");

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

    // Phase 2: create changelog table and mirror trigger.
    create_changelog(pool, &changelog, current_pk).await?;
    create_mirror_trigger(
        pool,
        &trigger_fn,
        &trigger_name,
        table_name,
        &shadow,
        &changelog,
        pk.name,
        &col_exprs,
        current,
    )
    .await?;

    // Phase 3: backfill in chunks while triggers mirror live writes.
    let backfill_start = Instant::now();
    backfill(
        pool,
        table_name,
        &shadow,
        pk.name,
        &current_pk.sql_type,
        &col_exprs,
        cfg,
    )
    .await?;

    // Phase 4: catch up changes until changelog is empty.
    catch_up_best_effort(
        pool,
        table_name,
        &shadow,
        &changelog,
        pk.name,
        &current_pk.sql_type,
        &col_exprs,
        cfg,
    )
    .await?;

    // Phase 5: cutover (short lock): disable trigger, final catch-up, swap, cleanup.
    cutover(
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
    for idx in spec.indexes {
        let sql = create_index_sql_concurrently(table_name, idx);
        let mut conn = pool.acquire().await?;
        sqlx::query(&sql).execute(&mut *conn).await?;
    }

    tracing::info!(
        table = table_name,
        shadow = shadow,
        backup = backup,
        elapsed_ms = backfill_start.elapsed().as_millis() as u64,
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
    shadow_table: &str,
    changelog: &str,
    pk_name: &'static str,
    col_exprs: &[(&'static str, String)],
    current: &TableSchema,
) -> Result<()> {
    let current_cols: std::collections::HashSet<&str> =
        current.columns.iter().map(|c| c.name.as_str()).collect();

    // Build INSERT column list and values list for NEW (INSERT/UPDATE).
    let columns = col_exprs
        .iter()
        .map(|(c, _)| quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", ");

    let values_new = col_exprs
        .iter()
        .map(|(c, expr)| {
            if current_cols.contains(c) {
                format!("NEW.{}", quote_identifier(c))
            } else if expr == "NULL" {
                "NULL".to_string()
            } else {
                expr.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    let set_parts = col_exprs
        .iter()
        .filter(|(c, _)| *c != pk_name)
        .map(|(c, _)| {
            let q = quote_identifier(c);
            format!("{q} = EXCLUDED.{q}")
        })
        .collect::<Vec<_>>()
        .join(", ");

    let pk_q = quote_identifier(pk_name);

    let fn_sql = format!(
        r#"
        CREATE OR REPLACE FUNCTION {fn_ident}() RETURNS trigger AS $$
        BEGIN
          IF (TG_OP = 'DELETE') THEN
            DELETE FROM {shadow} WHERE {pk_q} = OLD.{pk_q};
            INSERT INTO {changelog} (pk) VALUES (OLD.{pk_q})
              ON CONFLICT (pk) DO NOTHING;
            RETURN OLD;
          ELSE
            INSERT INTO {shadow} ({columns})
              VALUES ({values_new})
              {on_conflict};
            INSERT INTO {changelog} (pk) VALUES (NEW.{pk_q})
              ON CONFLICT (pk) DO NOTHING;
            RETURN NEW;
          END IF;
        END;
        $$ LANGUAGE plpgsql;
        "#,
        fn_ident = quote_identifier(trigger_fn),
        shadow = quote_identifier(shadow_table),
        changelog = quote_identifier(changelog),
        pk_q = pk_q,
        columns = columns,
        values_new = values_new,
        on_conflict = on_conflict_clause(&pk_q, &set_parts),
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
    source: &str,
    shadow: &str,
    pk: &'static str,
    pk_sql_type: &str,
    col_exprs: &[(&'static str, String)],
    cfg: &MigrationConfig,
) -> Result<()> {
    let pk_q = quote_identifier(pk);
    let pk_cast = cast_type_for_param(pk_sql_type)?;

    let mut last_pk: Option<String> = None;

    loop {
        // Keyset pagination with parameter cast (keeps index usability).
        let select_batch = if let Some(_) = last_pk {
            format!(
                "SELECT {pk_q}::text FROM {src} WHERE {pk_q} > ($1::{pk_cast}) ORDER BY {pk_q} LIMIT $2",
                pk_q = pk_q,
                src = quote_identifier(source),
                pk_cast = pk_cast,
            )
        } else {
            format!(
                "SELECT {pk_q}::text FROM {src} ORDER BY {pk_q} LIMIT $1",
                pk_q = pk_q,
                src = quote_identifier(source),
            )
        };

        let batch: Vec<(String,)> = if let Some(ref lp) = last_pk {
            sqlx::query_as(&select_batch)
                .bind(lp)
                .bind(cfg.online_chunk_size)
                .fetch_all(pool)
                .await?
        } else {
            sqlx::query_as(&select_batch)
                .bind(cfg.online_chunk_size)
                .fetch_all(pool)
                .await?
        };

        if batch.is_empty() {
            break;
        }

        let pks: Vec<String> = batch.into_iter().map(|t| t.0).collect();
        last_pk = pks.last().cloned();

        let columns = col_exprs
            .iter()
            .map(|(c, _)| quote_identifier(c))
            .collect::<Vec<_>>()
            .join(", ");

        // Build SELECT list from source alias `src`.
        let values = col_exprs
            .iter()
            .map(|(c, expr)| {
                if expr == "NULL" {
                    "NULL".to_string()
                } else if expr.starts_with('"') {
                    format!("src.{}", quote_identifier(c))
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

        let upsert = format!(
            "INSERT INTO {shadow} ({columns}) \
             SELECT {values} FROM {src} AS src \
             WHERE {pk_q} = ANY($1::{pk_cast}[]) \
             {on_conflict}",
            shadow = quote_identifier(shadow),
            src = quote_identifier(source),
            pk_q = pk_q,
            columns = columns,
            values = values,
            pk_cast = pk_cast,
            on_conflict = on_conflict_clause(&pk_q, &set_parts),
        );

        sqlx::query(&upsert).bind(&pks).execute(pool).await?;

        if cfg.online_sleep_ms > 0 {
            tokio::time::sleep(Duration::from_millis(cfg.online_sleep_ms)).await;
        }
    }

    Ok(())
}

async fn catch_up_best_effort(
    pool: &PgPool,
    source: &str,
    shadow: &str,
    changelog: &str,
    pk: &'static str,
    pk_sql_type: &str,
    col_exprs: &[(&'static str, String)],
    cfg: &MigrationConfig,
) -> Result<()> {
    let pk_q = quote_identifier(pk);
    let pk_cast = cast_type_for_param(pk_sql_type)?;
    let columns = col_exprs
        .iter()
        .map(|(c, _)| quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", ");

    let values = col_exprs
        .iter()
        .map(|(c, expr)| {
            if expr == "NULL" {
                "NULL".to_string()
            } else if expr.starts_with('"') {
                format!("src.{}", quote_identifier(c))
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

    for _round in 0..cfg.max_online_catchup_rounds {
        let pks: Vec<(String,)> = sqlx::query_as(&format!(
            "SELECT pk::text FROM {} ORDER BY pk::text LIMIT $1",
            quote_identifier(changelog)
        ))
        .bind(cfg.online_chunk_size)
        .fetch_all(pool)
        .await?;

        if pks.is_empty() {
            return Ok(());
        }

        let pk_values: Vec<String> = pks.into_iter().map(|t| t.0).collect();

        apply_changelog_batch(
            pool,
            source,
            shadow,
            changelog,
            &pk_values,
            &pk_q,
            pk_cast,
            &columns,
            &values,
            &set_parts,
        )
        .await?;
    }

    // Under ongoing writes, changelog may not reach zero. This is expected; final drain
    // happens after triggers are disabled in cutover.
    Ok(())
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
) -> Result<()> {
    // Best-effort: reduce changelog before locking.
    catch_up_best_effort(pool, source, shadow, changelog, pk, pk_sql_type, col_exprs, cfg).await?;

    // Cutover lock.
    let mut tx = pool.begin().await?;
    let lock_start = Instant::now();

    sqlx::query(&format!(
        "LOCK TABLE {} IN ACCESS EXCLUSIVE MODE",
        quote_identifier(source)
    ))
    .execute(tx.as_mut())
    .await?;

    // Disable trigger to stop mirroring.
    sqlx::query(&format!(
        "ALTER TABLE {} DISABLE TRIGGER {}",
        quote_identifier(source),
        quote_identifier(trigger_name)
    ))
    .execute(tx.as_mut())
    .await?;

    // Final drain inside lock (trigger disabled, so it must converge).
    let pk_q = quote_identifier(pk);
    let pk_cast = cast_type_for_param(pk_sql_type)?;

    let columns = col_exprs
        .iter()
        .map(|(c, _)| quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", ");

    let values = col_exprs
        .iter()
        .map(|(c, expr)| {
            if expr == "NULL" {
                "NULL".to_string()
            } else if expr.starts_with('"') {
                format!("src.{}", quote_identifier(c))
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

    for _round in 0..cfg.max_online_catchup_rounds {
        let pks: Vec<(String,)> = sqlx::query_as(&format!(
            "SELECT pk::text FROM {} ORDER BY pk::text LIMIT $1",
            quote_identifier(changelog)
        ))
        .bind(cfg.online_chunk_size)
        .fetch_all(tx.as_mut())
        .await?;
        if pks.is_empty() {
            break;
        }
        let pk_values: Vec<String> = pks.into_iter().map(|t| t.0).collect();
        apply_changelog_batch_tx(
            &mut tx,
            source,
            shadow,
            changelog,
            &pk_values,
            &pk_q,
            pk_cast,
            &columns,
            &values,
            &set_parts,
        )
        .await?;
    }

    // If still not empty, this violates the cutover safety contract.
    let remaining: (i64,) = sqlx::query_as(&format!(
        "SELECT COUNT(*)::BIGINT FROM {}",
        quote_identifier(changelog)
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

    // Drop mirror trigger on the backup table (now that the live table is swapped).
    sqlx::query(&format!(
        "DROP TRIGGER IF EXISTS {} ON {}",
        quote_identifier(trigger_name),
        quote_identifier(backup)
    ))
    .execute(tx.as_mut())
    .await?;

    // Cleanup trigger function and changelog outside of the old table (now backup).
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

    Ok(())
}

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

fn on_conflict_clause(pk_q: &str, set_parts: &str) -> String {
    if set_parts.trim().is_empty() {
        format!("ON CONFLICT ({pk_q}) DO NOTHING")
    } else {
        format!("ON CONFLICT ({pk_q}) DO UPDATE SET {set_parts}")
    }
}

async fn apply_changelog_batch(
    pool: &PgPool,
    source: &str,
    shadow: &str,
    changelog: &str,
    pk_values: &[String],
    pk_q: &str,
    pk_cast: &str,
    columns: &str,
    values: &str,
    set_parts: &str,
) -> Result<()> {
    let upsert = format!(
        "INSERT INTO {shadow} ({columns}) \
         SELECT {values} FROM {src} AS src \
         WHERE {pk_q} = ANY($1::{pk_cast}[]) \
         {on_conflict}",
        shadow = quote_identifier(shadow),
        src = quote_identifier(source),
        pk_q = pk_q,
        columns = columns,
        values = values,
        pk_cast = pk_cast,
        on_conflict = on_conflict_clause(pk_q, set_parts),
    );
    sqlx::query(&upsert)
        .bind(pk_values)
        .execute(pool)
        .await?;

    let delete_missing = format!(
        "DELETE FROM {shadow} s \
         WHERE s.{pk_q} = ANY($1::{pk_cast}[]) \
           AND NOT EXISTS (SELECT 1 FROM {src} o WHERE o.{pk_q} = s.{pk_q})",
        shadow = quote_identifier(shadow),
        src = quote_identifier(source),
        pk_q = pk_q,
        pk_cast = pk_cast,
    );
    sqlx::query(&delete_missing)
        .bind(pk_values)
        .execute(pool)
        .await?;

    let clear = format!(
        "DELETE FROM {} WHERE pk::text = ANY($1)",
        quote_identifier(changelog)
    );
    sqlx::query(&clear)
        .bind(pk_values)
        .execute(pool)
        .await?;
    Ok(())
}

async fn apply_changelog_batch_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source: &str,
    shadow: &str,
    changelog: &str,
    pk_values: &[String],
    pk_q: &str,
    pk_cast: &str,
    columns: &str,
    values: &str,
    set_parts: &str,
) -> Result<()> {
    let upsert = format!(
        "INSERT INTO {shadow} ({columns}) \
         SELECT {values} FROM {src} AS src \
         WHERE {pk_q} = ANY($1::{pk_cast}[]) \
         {on_conflict}",
        shadow = quote_identifier(shadow),
        src = quote_identifier(source),
        pk_q = pk_q,
        columns = columns,
        values = values,
        pk_cast = pk_cast,
        on_conflict = on_conflict_clause(pk_q, set_parts),
    );
    sqlx::query(&upsert)
        .bind(pk_values)
        .execute(tx.as_mut())
        .await?;

    let delete_missing = format!(
        "DELETE FROM {shadow} s \
         WHERE s.{pk_q} = ANY($1::{pk_cast}[]) \
           AND NOT EXISTS (SELECT 1 FROM {src} o WHERE o.{pk_q} = s.{pk_q})",
        shadow = quote_identifier(shadow),
        src = quote_identifier(source),
        pk_q = pk_q,
        pk_cast = pk_cast,
    );
    sqlx::query(&delete_missing)
        .bind(pk_values)
        .execute(tx.as_mut())
        .await?;

    let clear = format!(
        "DELETE FROM {} WHERE pk::text = ANY($1)",
        quote_identifier(changelog)
    );
    sqlx::query(&clear)
        .bind(pk_values)
        .execute(tx.as_mut())
        .await?;
    Ok(())
}

fn create_index_sql_concurrently(table_name: &str, index: &crate::IndexInfo) -> String {
    let unique = if index.unique { "UNIQUE " } else { "" };
    let cols = index
        .columns
        .iter()
        .map(|c| quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "CREATE {unique}INDEX CONCURRENTLY IF NOT EXISTS {} ON {} USING {} ({cols})",
        quote_identifier(index.name),
        quote_identifier(table_name),
        index.index_type.to_sql()
    )
}
