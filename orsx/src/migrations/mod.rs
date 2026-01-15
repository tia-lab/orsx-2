use crate::{Error, OrsxMigrate, Result};
use sqlx::PgPool;
use std::time::Instant;

pub mod config;
pub mod introspection;
pub mod online;
pub mod planning;

pub struct Migrations;

impl Migrations {
    pub async fn init<T: OrsxMigrate>(
        pool: &PgPool,
        migrations: &[(T, Option<&str>)],
    ) -> Result<()> {
        Self::init_with_config::<T>(pool, migrations, &config::MigrationConfig::default()).await
    }

    pub async fn init_with_config<T: OrsxMigrate>(
        pool: &PgPool,
        migrations: &[(T, Option<&str>)],
        cfg: &config::MigrationConfig,
    ) -> Result<()> {
        for (_instance, custom_name) in migrations {
            let spec = T::spec();
            let table_name = custom_name.unwrap_or(spec.table_name);

            if !introspection::table_exists(pool, table_name).await? {
                let start = Instant::now();
                let create_sql = T::create_table_sql(Some(table_name));
                sqlx::query(&create_sql).execute(pool).await?;

                // Create indexes (best-effort; deterministic SQL).
                for idx in spec.indexes {
                    let sql = planning::create_index_sql(table_name, idx);
                    sqlx::query(&sql).execute(pool).await?;
                }

                tracing::info!(
                    table = table_name,
                    elapsed_ms = start.elapsed().as_millis() as u64,
                    "created table"
                );
                continue;
            }

            let start = Instant::now();
            let expected = planning::expected_schema_from_spec(table_name, &spec);

            // Fast-path, non-rewrite migrations (must remain safe at scale).
            // Iterate because some safe operations (e.g. ADD COLUMN) can reveal
            // follow-up diffs (e.g. add uniqueness) that are also safe.
            let mut iterations = 0u8;
            loop {
                iterations = iterations.saturating_add(1);
                if iterations > 5 {
                    break;
                }

                let current0 = introspection::read_table_schema(pool, table_name).await?;
                let current = if let Some(updated) =
                    planning::apply_safe_renames(pool, table_name, &spec, cfg, &current0).await?
                {
                    updated
                } else {
                    current0
                };
                let diff = planning::filter_ignored_diffs(
                    cfg,
                    planning::diff_schema(&current, &expected),
                );
                planning::validate_strictness(cfg, &diff)?;
                if diff.is_empty() {
                    break;
                }

                planning::apply_safe_alters(pool, table_name, &spec, cfg, &current, &expected, &diff)
                    .await?;
            }

            let after = introspection::read_table_schema(pool, table_name).await?;
            let after_diff = planning::filter_ignored_diffs(cfg, planning::diff_schema(&after, &expected));
            planning::validate_strictness(cfg, &after_diff)?;

            if !after_diff.is_empty() {
                // Online rewrite path for remaining diffs (large-table safe).
                online::online_rewrite_table(pool, table_name, &spec, &after, &expected, cfg).await?;

                let final_schema = introspection::read_table_schema(pool, table_name).await?;
                let final_diff = planning::filter_ignored_diffs(cfg, planning::diff_schema(
                    &final_schema,
                    &expected,
                ));
                planning::validate_strictness(cfg, &final_diff)?;
                if !final_diff.is_empty() {
                    return Err(Error::MigrationNeeded(format!(
                        "table {table_name} still differs after online rewrite: {final_diff:?}"
                    )));
                }
            }

            // Ensure indexes exist even when there were no schema diffs. This must be done after
            // any rewrite, but also for externally-created tables that match column schema.
            planning::ensure_indexes_concurrently(pool, table_name, &spec).await?;

            tracing::info!(
                table = table_name,
                elapsed_ms = start.elapsed().as_millis() as u64,
                "applied safe alters"
            );
        }

        Ok(())
    }
}
