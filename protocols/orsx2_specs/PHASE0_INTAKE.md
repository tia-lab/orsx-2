# ORSX2 — Phase 0 Intake (FILL THIS)

This document is blocking input for implementation. Fill values explicitly; do not leave “TBD”.

## A) Platform

- Target Postgres versions (min..max): 16..17
- Extensions allowed/required (e.g. `pgcrypto`, `uuid-ossp`, `pg_stat_statements`): allowed: `pgcrypto` (for `gen_random_uuid()` if used); otherwise avoid hard requirements
- Deployment topology:
  - single primary only / primary + replicas: primary + replicas possible
  - synchronous replication enabled (yes/no): unknown; assume may be enabled in production

## B) Migration constraints (very large tables)

- Largest table row count to support: up to 100,000,000 (1e8)
- Typical table row count: 1,000,000..20,000,000
- Approx row width (bytes) or example `pg_relation_size`: 200..2000 bytes/row (high variance)
- Typical index count per table: 5..30
- Index types used (btree/gin/gist/hash): btree common; gin possible; others rare
- Partitioning used (yes/no; if yes, how): unknown; assume may be used (range partitioning by time)
- Write traffic during migration:
  - must continue (yes/no): yes
  - peak writes/sec: 1,000..50,000 (workload-dependent)
  - acceptable degraded mode (yes/no; describe): yes; allow throttled backfill and extended wall-clock migration while preserving correctness
- Cutover lock budget (seconds): 5
- Maintenance window available (yes/no; duration): no guaranteed window

## C) Schema model

- Primary key type(s) (text/uuid/bigint/compound): uuid or bigint (single-column), sometimes text
- Typical schema churn:
  - add columns frequently (yes/no):
  - type changes happen (yes/no; which types):
  - nullability changes happen (yes/no):
- Required supported operations (list must-match):

## D) Compression model

- Element types to support (f32/f64/i32/i64/u32/u64): f32/f64/i32/i64/u32/u64
- Typical vector lengths: 384..20,000
- Max vector length: 1,000,000 (rare; must remain safe)
- Expected compression ratio target (rough): workload-dependent; target 30%..90% space savings when data is compressible
- Data must remain readable across versions (yes/no; for now we assume yes within envelope versions):

## E) Performance targets (hard budgets)

- Planning (schema diff) budget:
  - max columns: 1,000
  - max indexes: 200
  - time budget (ms): 50
- Offline migration budget (if used):
  - max table size where offline is allowed: <= 1,000,000 rows (default), configurable
  - time budget: best-effort; must still obey cutover lock budget and safety invariants
- Online migration budget:
  - backfill throughput target (rows/sec or MB/s): >= 50,000 rows/sec on typical hardware; throttleable
  - cutover time target (seconds): <= 5
- Compression throughput:
  - encode MB/s (typical sizes): >= 200 MB/s at 10k elements
  - decode MB/s (typical sizes): >= 200 MB/s at 10k elements

## F) Safety requirements

- Verification strength:
  - row count only / row count + checksum / row count + sampled checksum: row count + sampled checksum (default); configurable
- Backup retention:
  - keep N backups:
  - retention days:
