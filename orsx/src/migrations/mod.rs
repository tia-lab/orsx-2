// Migration system with zero-loss schema changes
use crate::{OrsxMigrate, Result};
use sqlx::PgPool;
use tracing::info;

pub mod comparison;
pub mod introspection;
pub mod retention;
pub mod zero_loss;

// Re-exports
pub use comparison::{SchemaComparison, SchemaDifference};
pub use introspection::{ColumnInfo, TableSchema};
pub use retention::MigrationConfig;
pub use zero_loss::{MigrationAction, MigrationResult};

// Main migrations API - preserves V1 interface
pub struct Migrations;

impl Migrations {
    // Initialize migrations with default config
    pub async fn init<T: OrsxMigrate>(
        pool: &PgPool,
        migrations: &[(T, Option<&str>)],
    ) -> Result<Vec<MigrationResult>> {
        Self::init_with_config(pool, migrations, &MigrationConfig::default()).await
    }

    // Initialize migrations with custom config
    pub async fn init_with_config<T: OrsxMigrate>(
        pool: &PgPool,
        migrations: &[(T, Option<&str>)],
        config: &MigrationConfig,
    ) -> Result<Vec<MigrationResult>> {
        let mut results = Vec::new();

        for (_instance, custom_table_name) in migrations {
            let table_name = match custom_table_name {
                Some(name) => name,
                None => T::table_name(),
            };

            let result = ensure_table::<T>(pool, table_name, config).await?;
            results.push(result);
        }

        Ok(results)
    }
}

// Ensure table exists with correct schema
async fn ensure_table<T: OrsxMigrate>(
    pool: &PgPool,
    table_name: &str,
    config: &MigrationConfig,
) -> Result<MigrationResult> {
    // Step 1: Check if table exists
    let table_exists = introspection::table_exists(pool, table_name).await?;

    if !table_exists {
        // Create new table
        let create_sql = generate_create_table_sql::<T>(table_name);

        sqlx::query(&create_sql)
            .execute(pool)
            .await
            .map_err(|e| crate::Error::Migration {
                message: format!("Failed to create table '{}': {}", table_name, e),
                sql: Some(create_sql.clone()),
                context: Some("create_table".to_string()),
            })?;

        info!("Created table '{}' from schema", table_name);

        // Ensure indexes are created for new table
        let indexes = T::table_indexes();
        if !indexes.is_empty() {
            crate::indexes::ensure_indexes(pool, table_name, &indexes).await?;
            info!(
                "Created {} indexes for table '{}'",
                indexes.len(),
                table_name
            );
        }

        return Ok(MigrationResult {
            action: MigrationAction::TableCreated,
            backup_table: None,
            rows_migrated: 0,
            schema_changes: vec![format!("Created table {}", table_name)],
        });
    }

    // Step 2: Compare schemas
    let current_schema = introspection::read_table_schema(pool, table_name).await?;
    let expected_schema = infer_schema_from_trait::<T>();
    let comparison = comparison::compare_schemas(&current_schema.columns, &expected_schema);

    if !comparison.needs_migration {
        info!("Table '{}' schema matches expected schema", table_name);
        return Ok(MigrationResult {
            action: MigrationAction::SchemaMatched,
            backup_table: None,
            rows_migrated: 0,
            schema_changes: vec![],
        });
    }

    info!(
        "Table '{}' needs migration: {} differences detected",
        table_name,
        comparison.differences.len()
    );

    // Step 3: Perform zero-loss migration
    let result = zero_loss::execute_zero_loss_migration(
        pool,
        table_name,
        &comparison,
        &config.backup_suffix,
    )
    .await?;

    // Ensure indexes are created after migration
    let indexes = T::table_indexes();
    if !indexes.is_empty() {
        crate::indexes::ensure_indexes(pool, table_name, &indexes).await?;
        info!("Re-created {} indexes after migration", indexes.len());
    }

    // Step 4: Clean up old backups
    let deleted = retention::cleanup_old_backups(
        pool,
        table_name,
        &config.backup_suffix,
        config.max_backups_per_table,
        config.backup_retention_days,
    )
    .await?;

    if deleted > 0 {
        info!("Cleaned up {} old backup tables", deleted);
    }

    Ok(result)
}

// Infer expected schema from OrsxMigrate trait
fn infer_schema_from_trait<T: OrsxMigrate>() -> Vec<ColumnInfo> {
    let field_names = T::field_names();
    let field_types = T::field_types();
    let field_nullable = T::field_nullable();
    let primary_key = T::primary_key_field();

    let mut columns = Vec::new();

    for (i, ((name, field_type), nullable)) in field_names
        .iter()
        .zip(field_types.iter())
        .zip(field_nullable.iter())
        .enumerate()
    {
        let is_primary_key = *name == primary_key;
        let sql_type = field_type.to_sql();

        // Determine if this field has a default value
        let has_default = (is_primary_key && sql_type == "TEXT")
            || (*name == "created_at" || *name == "updated_at");

        columns.push(ColumnInfo {
            name: name.to_string(),
            sql_type,
            nullable: if is_primary_key { false } else { *nullable },
            position: i as i32,
            is_unique: is_primary_key, // Primary keys are implicitly unique
            is_primary_key,
            foreign_key_reference: None,
            has_default,
            is_compressed: field_type == &crate::types::FieldType::Bytea,
        });
    }

    columns
}

// Generate CREATE TABLE SQL from OrsxMigrate trait
fn generate_create_table_sql<T: OrsxMigrate>(table_name: &str) -> String {
    // Get original migration SQL and replace table name
    let original_sql = T::migration_sql();
    let original_table_name = T::table_name();

    // Replace the table name in the SQL
    let replacements = [
        (
            format!("CREATE TABLE {}", original_table_name),
            format!("CREATE TABLE {}", table_name),
        ),
        (
            format!("CREATE TABLE \"{}\"", original_table_name),
            format!("CREATE TABLE \"{}\"", table_name),
        ),
        (
            format!("CREATE TABLE IF NOT EXISTS {}", original_table_name),
            format!("CREATE TABLE IF NOT EXISTS {}", table_name),
        ),
        (
            format!("CREATE TABLE IF NOT EXISTS \"{}\"", original_table_name),
            format!("CREATE TABLE IF NOT EXISTS \"{}\"", table_name),
        ),
    ];

    let mut modified_sql = original_sql;
    for (from, to) in replacements {
        modified_sql = modified_sql.replace(&from, &to);
    }

    modified_sql
}
