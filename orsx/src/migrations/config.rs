#[derive(Debug, Clone)]
pub struct MigrationConfig {
    pub offline_row_threshold: i64,
    pub online_chunk_size: i64,
    pub online_sleep_ms: u64,
    pub max_online_catchup_rounds: u32,
    pub cutover_lock_budget_ms: u64,
    /// Enforce that physical column order in Postgres matches the spec (by rewriting when needed).
    pub enforce_column_order: bool,
    /// Enforce that the DB has exactly the same columns as the spec (no extras).
    pub enforce_exact_columns: bool,
    /// Allow online rewrite to remove extra DB columns from the live table (backup table is kept).
    pub allow_destructive_drops: bool,
    /// Allow `#[orsx_column(rename_from = \"...\")]` to rename columns using `ALTER TABLE ... RENAME COLUMN ...`.
    pub allow_column_renames: bool,

    /// Opt-in adaptive chunk sizing for online changelog catch-up.
    /// This trades execution determinism for throughput (chunk sizes depend on runtime timings).
    pub adaptive_chunk: bool,
    pub online_chunk_size_min: i64,
    pub online_chunk_size_max: i64,
    pub online_target_batch_ms: u64,
    pub online_max_batch_ms: u64,

    /// Opt-in: set `synchronous_commit=off` for backfill/catch-up sessions.
    /// This can improve throughput on latency-bound storage but trades away per-transaction durability
    /// during those phases. Cutover still enforces correctness and uses an explicit lock budget.
    pub synchronous_commit_off: bool,

    /// Opt-in: parallelize backfill for BIGINT PK tables (copy phase only).
    /// Default path remains single-threaded and deterministic.
    pub parallel_backfill: bool,
    pub parallel_backfill_workers: usize,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            offline_row_threshold: 1_000_000,
            online_chunk_size: 10_000,
            online_sleep_ms: 0,
            max_online_catchup_rounds: 50,
            cutover_lock_budget_ms: 5_000,
            enforce_column_order: false,
            enforce_exact_columns: false,
            allow_destructive_drops: false,
            allow_column_renames: true,
            adaptive_chunk: false,
            online_chunk_size_min: 10_000,
            online_chunk_size_max: 200_000,
            online_target_batch_ms: 250,
            online_max_batch_ms: 2_000,
            synchronous_commit_off: false,
            parallel_backfill: false,
            parallel_backfill_workers: 4,
        }
    }
}
