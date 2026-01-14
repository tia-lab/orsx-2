// Zero-loss migration execution - V1 proven algorithm
use super::{comparison::SchemaComparison, introspection::ColumnInfo};
use crate::{Error, Result};
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{debug, info};

// Execute zero-loss migration with backup creation
pub async fn execute_zero_loss_migration(
    pool: &PgPool,
    table_name: &str,
    comparison: &SchemaComparison,
    backup_suffix: &str,
) -> Result<MigrationResult> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let temp_table = format!("{}_temp_{}", table_name, timestamp);
    let backup_table = format!("{}_{}_{}", table_name, backup_suffix, timestamp);

    info!("Starting zero-loss migration for table: {}", table_name);
    debug!("Temp table: {}, Backup table: {}", temp_table, backup_table);

    // Step 1: Create temp table with new schema
    let create_sql = generate_create_table_sql(&temp_table, &comparison.expected_columns);
    sqlx::query(&create_sql)
        .execute(pool)
        .await
        .map_err(|e| Error::Migration {
            message: format!("Failed to create temp table: {}", e),
            sql: Some(create_sql.clone()),
            context: Some("create_temp_table".to_string()),
        })?;

    debug!("Created temp table with new schema");

    // Step 2: Migrate data from old table to temp table
    let migrate_sql = generate_data_migration_sql(
        table_name,
        &temp_table,
        &comparison.current_columns,
        &comparison.expected_columns,
    );

    let rows_migrated = sqlx::query(&migrate_sql)
        .execute(pool)
        .await
        .map_err(|e| Error::Migration {
            message: format!("Failed to migrate data: {}", e),
            sql: Some(migrate_sql.clone()),
            context: Some("migrate_data".to_string()),
        })?
        .rows_affected();

    debug!("Migrated {} rows to temp table", rows_migrated);

    // Step 3: Rename original table to backup
    let rename_to_backup = format!(
        "ALTER TABLE \"{}\" RENAME TO \"{}\"",
        table_name, backup_table
    );
    sqlx::query(&rename_to_backup)
        .execute(pool)
        .await
        .map_err(|e| Error::Migration {
            message: format!("Failed to create backup: {}", e),
            sql: Some(rename_to_backup.clone()),
            context: Some("create_backup".to_string()),
        })?;

    debug!("Renamed original table to backup: {}", backup_table);

    // Step 4: Rename temp table to original name
    let rename_to_original = format!(
        "ALTER TABLE \"{}\" RENAME TO \"{}\"",
        temp_table, table_name
    );
    sqlx::query(&rename_to_original)
        .execute(pool)
        .await
        .map_err(|e| Error::Migration {
            message: format!("Failed to rename temp table: {}", e),
            sql: Some(rename_to_original.clone()),
            context: Some("rename_temp_table".to_string()),
        })?;

    debug!("Renamed temp table to active table: {}", table_name);

    // Step 5: Verify migration success
    let verify_count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM \"{}\"", table_name))
        .fetch_one(pool)
        .await
        .map_err(|e| Error::Migration {
            message: format!("Failed to verify migration: {}", e),
            sql: None,
            context: Some("verify_migration".to_string()),
        })?;

    let final_count = verify_count.0 as u64;
    if final_count != rows_migrated {
        return Err(Error::Migration {
            message: format!(
                "Row count mismatch after migration: expected {}, got {}",
                rows_migrated, final_count
            ),
            sql: None,
            context: Some("verify_row_count".to_string()),
        });
    }

    info!(
        "Zero-loss migration completed: {} rows migrated, backup: {}",
        rows_migrated, backup_table
    );

    Ok(MigrationResult {
        action: MigrationAction::DataMigrated {
            from: backup_table.clone(),
            to: table_name.to_string(),
        },
        backup_table: Some(backup_table),
        rows_migrated,
        schema_changes: comparison
            .differences
            .iter()
            .map(|d| d.describe())
            .collect(),
    })
}

// Generate CREATE TABLE SQL from column metadata
fn generate_create_table_sql(table_name: &str, columns: &[ColumnInfo]) -> String {
    let mut column_defs = Vec::new();
    let mut table_constraints = Vec::new();

    for column in columns {
        let mut def = format!("\"{}\" {}", column.name, column.sql_type);

        if !column.nullable {
            def.push_str(" NOT NULL");
        }

        // Primary key constraint
        if column.is_primary_key {
            def.push_str(" PRIMARY KEY");
        }

        // Add default values
        if column.has_default {
            if column.is_primary_key && column.sql_type == "TEXT" {
                def.push_str(" DEFAULT gen_random_uuid()::text");
            } else if column.name == "created_at" || column.name == "updated_at" {
                def.push_str(" DEFAULT NOW()");
            }
        }

        column_defs.push(def);

        // Add unique constraints as table-level constraints
        if column.is_unique && !column.is_primary_key {
            table_constraints.push(format!("UNIQUE (\"{}\")", column.name));
        }
    }

    // Add table-level constraints
    column_defs.extend(table_constraints);

    format!(
        "CREATE TABLE IF NOT EXISTS \"{}\" (\n  {}\n)",
        table_name,
        column_defs.join(",\n  ")
    )
}

// Generate data migration SQL with type conversions
fn generate_data_migration_sql(
    source_table: &str,
    target_table: &str,
    source_columns: &[ColumnInfo],
    target_columns: &[ColumnInfo],
) -> String {
    let source_map: HashMap<String, &ColumnInfo> =
        source_columns.iter().map(|c| (c.name.clone(), c)).collect();

    let mut select_columns = Vec::new();

    for target_col in target_columns {
        if let Some(source_col) = source_map.get(&target_col.name) {
            // Column exists in both tables
            if source_col.sql_type == target_col.sql_type {
                // Same type - direct copy
                select_columns.push(format!("\"{}\"", target_col.name));
            } else {
                // Different type - apply conversion
                let conversion = generate_type_conversion(
                    &source_col.sql_type,
                    &target_col.sql_type,
                    &target_col.name,
                );
                select_columns.push(conversion);
            }
        } else {
            // Column doesn't exist in source - use default value
            if target_col.nullable {
                select_columns.push("NULL".to_string());
            } else {
                // Provide sensible defaults for NOT NULL columns
                match target_col.sql_type.as_str() {
                    "TEXT" => select_columns.push("''::TEXT".to_string()),
                    "INTEGER" => select_columns.push("0::INTEGER".to_string()),
                    "BIGINT" => select_columns.push("0::BIGINT".to_string()),
                    "REAL" | "DOUBLE PRECISION" => {
                        select_columns.push("0.0::DOUBLE PRECISION".to_string())
                    }
                    "BOOLEAN" => select_columns.push("false::BOOLEAN".to_string()),
                    _ => select_columns.push("NULL".to_string()),
                }
            }
        }
    }

    let target_column_names: Vec<String> = target_columns
        .iter()
        .map(|c| format!("\"{}\"", c.name))
        .collect();

    format!(
        "INSERT INTO \"{}\" ({}) SELECT {} FROM \"{}\"",
        target_table,
        target_column_names.join(", "),
        select_columns.join(", "),
        source_table
    )
}

// Generate type conversion SQL (PostgreSQL-specific)
fn generate_type_conversion(source_type: &str, target_type: &str, column_name: &str) -> String {
    match (source_type, target_type) {
        // Array to BYTEA conversions (for compression)
        ("BIGINT[]", "BYTEA")
        | ("INTEGER[]", "BYTEA")
        | ("DOUBLE PRECISION[]", "BYTEA")
        | ("TEXT[]", "BYTEA") => {
            format!(
                "CASE WHEN \"{}\" IS NULL THEN NULL ELSE convert_to(array_to_json(\"{}\")::text, 'UTF8') END",
                column_name, column_name
            )
        }
        // TEXT to array conversions (JSON array strings)
        ("TEXT", "BIGINT[]") => {
            format!(
                "CASE WHEN \"{}\" LIKE '[%]' THEN \
                 (SELECT ARRAY_AGG(elem::BIGINT) FROM jsonb_array_elements_text(\"{}\"::jsonb) AS elem) \
                 ELSE NULL::BIGINT[] END",
                column_name, column_name
            )
        }
        ("TEXT", "INTEGER[]") => {
            format!(
                "CASE WHEN \"{}\" LIKE '[%]' THEN \
                 (SELECT ARRAY_AGG(elem::INTEGER) FROM jsonb_array_elements_text(\"{}\"::jsonb) AS elem) \
                 ELSE NULL::INTEGER[] END",
                column_name, column_name
            )
        }
        ("TEXT", "DOUBLE PRECISION[]") => {
            format!(
                "CASE WHEN \"{}\" LIKE '[%]' THEN \
                 (SELECT ARRAY_AGG(elem::DOUBLE PRECISION) FROM jsonb_array_elements_text(\"{}\"::jsonb) AS elem) \
                 ELSE NULL::DOUBLE PRECISION[] END",
                column_name, column_name
            )
        }
        ("TEXT", "TEXT[]") => {
            format!(
                "CASE WHEN \"{}\" LIKE '[%]' THEN \
                 (SELECT ARRAY_AGG(elem::TEXT) FROM jsonb_array_elements_text(\"{}\"::jsonb) AS elem) \
                 ELSE NULL::TEXT[] END",
                column_name, column_name
            )
        }
        // TEXT to BYTEA (for compression migration)
        ("TEXT", "BYTEA") => format!("convert_to(\"{}\", 'UTF8')", column_name),
        // Default: try direct cast
        _ => format!("\"{}\"::{}", column_name, target_type),
    }
}

// Migration result
#[derive(Debug, Clone)]
pub struct MigrationResult {
    pub action: MigrationAction,
    pub backup_table: Option<String>,
    pub rows_migrated: u64,
    pub schema_changes: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum MigrationAction {
    TableCreated,
    SchemaMatched,
    DataMigrated { from: String, to: String },
}
