// Backup retention management - cleanup old migration backups
use crate::{Error, Result};
use sqlx::PgPool;
use tracing::info;

// Backup table metadata
#[derive(Debug)]
struct BackupTableInfo {
    name: String,
    timestamp: u64,
}

// Clean up old backup tables based on retention policy
pub async fn cleanup_old_backups(
    pool: &PgPool,
    table_name: &str,
    backup_suffix: &str,
    max_backups: u8,
    retention_days: u8,
) -> Result<usize> {
    let backup_tables = get_backup_tables(pool, table_name, backup_suffix).await?;

    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Sort by timestamp (newest first)
    let mut sorted_tables = backup_tables;
    sorted_tables.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let mut deleted_count = 0;

    for (index, backup) in sorted_tables.iter().enumerate() {
        let age_seconds = current_time - backup.timestamp;
        let age_days = age_seconds / 86400; // Convert to days

        let should_delete =
            // Delete if we exceed max backups (keep only the most recent ones)
            index >= max_backups as usize ||
            // OR delete if older than retention policy
            age_days > retention_days as u64;

        if should_delete {
            sqlx::query(&format!("DROP TABLE IF EXISTS \"{}\" CASCADE", backup.name))
                .execute(pool)
                .await
                .map_err(|e| Error::Migration {
                    message: format!("Failed to drop old backup table: {}", e),
                    sql: None,
                    context: Some("drop_backup_table".to_string()),
                })?;

            info!(
                "Cleaned up old backup table: {} (age: {} days, index: {})",
                backup.name, age_days, index
            );

            deleted_count += 1;
        }
    }

    Ok(deleted_count)
}

// Get all backup tables for a given base table
async fn get_backup_tables(
    pool: &PgPool,
    base_table: &str,
    suffix: &str,
) -> Result<Vec<BackupTableInfo>> {
    let pattern = format!("{}_{}_", base_table, suffix);

    let table_names: Vec<(String,)> = sqlx::query_as(
        "SELECT table_name FROM information_schema.tables
         WHERE table_schema = 'public' AND table_name LIKE $1",
    )
    .bind(format!("{}%", pattern))
    .fetch_all(pool)
    .await?;

    let mut backup_tables = Vec::new();

    for (table_name,) in table_names {
        // Extract timestamp from table name like "table_migration_1234567890"
        let suffix_pattern = format!("_{}_", suffix);
        if let Some(timestamp_str) = table_name.split(&suffix_pattern).nth(1) {
            if let Ok(timestamp) = timestamp_str.parse::<u64>() {
                backup_tables.push(BackupTableInfo {
                    name: table_name,
                    timestamp,
                });
            }
        }
    }

    Ok(backup_tables)
}

// Migration configuration
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    pub max_backups_per_table: u8,
    pub backup_retention_days: u8,
    pub backup_suffix: String,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            max_backups_per_table: 5,
            backup_retention_days: 30,
            backup_suffix: "migration".to_string(),
        }
    }
}

impl MigrationConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_backups(mut self, max_backups: u8) -> Self {
        self.max_backups_per_table = max_backups;
        self
    }

    pub fn with_retention_days(mut self, days: u8) -> Self {
        self.backup_retention_days = days;
        self
    }

    pub fn with_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.backup_suffix = suffix.into();
        self
    }
}
