# ORSX2 — Testing and Bench Plan (TEMPLATE)

Status: DRAFT  
Owner: (Validator)

## 1) Test categories (mandatory)

### 1.1 Planning determinism (no DB)

- Same schema input repeated ⇒ identical plan output (stable ordering).

### 1.2 Failure contract coverage (no DB + DB)

- Every documented rejection path has a test.

### 1.3 DB integration (requires Postgres)

- Schema create
- Each supported migration class
- Online path (if supported)
- Compression `BYTEA` read/write

### 1.3.1 Strict enforcement correctness (DB)

- `enforce_column_order=true` forces rewrite when order mismatched; post-migration physical order equals spec.
- `enforce_exact_columns=true` fails unless `allow_destructive_drops=true` when DB has extra columns.
- With `allow_destructive_drops=true`, live table drops extras but backup table retains data.
- `rename_from` performs `ALTER TABLE ... RENAME COLUMN ...` (when enabled) and preserves data.

### 1.3.2 Online rewrite correctness under load (DB)

- Concurrent inserts during rewrite.
- Concurrent inserts + updates + deletes during rewrite.
- Validate:
  - `new NOT NULL` + `default_sql` honored
  - backup table exists
  - cutover lock budget not exceeded

### 1.4 Panic safety

- Ensure invalid inputs never panic (use `catch_unwind` where appropriate).

## 2) Bench categories (mandatory)

- Compression encode/decode throughput.
- Planning time vs schema size.
- DB-facing throughput (if feasible): bulk insert/read; online backfill.

## 2.2 Migration perf trials (DB, release)

Append results to `protocols/orsx2_evidence/migration_trials.md`:

- Default vs strict compare:
  - 200k rows, ~50 cols
  - 1M rows, ~50 cols
- Worst-case writer:
  - 1M rows, inserts+updates+deletes during rewrite
- Optimization A/B runs:
  - rerun the same workload after each high-impact optimization to quantify deltas
  - include A/B for `adaptive_chunk` on writer-heavy workloads (expect catch-up improvements; no change on backfill-only workloads)
  - include A/B for `parallel_backfill` on BIGINT PK workloads (expect backfill improvements)
  - include A/B for `synchronous_commit_off` (expect storage-dependent improvements)

## 2.1 Allocation discipline checks (mandatory)

- At least one benchmark must run repeated calls with a workspace to prove no per-iteration allocations.
- Where applicable, add a `#[global_allocator]` counting allocator in benches (or a feature-gated allocator counter) to detect regressions.

## 3) Append-only evidence logs (mandatory)

- `protocols/orsx2_evidence/bench_results.md`
- `protocols/orsx2_evidence/migration_trials.md`

Each entry must include:

- timestamp, machine info, command, profile, results.
