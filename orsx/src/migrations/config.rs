#[derive(Debug, Clone)]
pub struct MigrationConfig {
    pub offline_row_threshold: i64,
    pub online_chunk_size: i64,
    pub online_sleep_ms: u64,
    pub max_online_catchup_rounds: u32,
    pub cutover_lock_budget_ms: u64,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            offline_row_threshold: 1_000_000,
            online_chunk_size: 10_000,
            online_sleep_ms: 0,
            max_online_catchup_rounds: 50,
            cutover_lock_budget_ms: 5_000,
        }
    }
}

