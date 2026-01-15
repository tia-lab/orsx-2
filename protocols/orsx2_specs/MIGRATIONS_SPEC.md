# ORSX2 — Migrations Spec (TEMPLATE)

Status: DRAFT  
Owner: (Designer)  
Applies to: `orsx2` crate

## 1) Identification

- Component: `orsx2::migrations`
- Purpose: schema-driven, zero-loss migrations for PostgreSQL at large-table scale.
- Non-scope:
  - ORM, query builder, cross-database support.
  - Columnar retrieval / transport (specified separately in `protocols/orsx2_specs/COLUMNAR_RETRIEVAL_SPEC.md`).

## 2) Inputs and bounds (mandatory)

- Postgres versions supported: 16..17
- Table size bounds (rows, row width): up to 1e8 rows; 200..2000 bytes/row
- Index expectations: 5..30 indexes typical; btree common; gin possible
- Max acceptable cutover lock time: 5 seconds
- Write traffic during migration (allowed / not allowed): allowed; must continue
- Triggers allowed during migration: yes (required for online path)

## 3) Supported change set (must be explicit)

For each item below, mark one: **SUPPORTED (offline)** / **SUPPORTED (online)** / **REJECTED**.

- Add column
- Drop column
- Rename column
- Type change (safe cast)
- Type change (requires rewrite)
- Nullability change
- Default change
- Primary key change
- Unique constraint change
- Foreign keys
- Index add/remove/change (btree/gin/gist/hash)
- Partitioned tables (if any)
- Materialized views / views (if any)

## 3.1 Strict schema enforcement (opt-in)

Strict mode is opt-in via `MigrationConfig` and affects **planning** (which diffs are ignored) and **strategy selection** (when a rewrite is forced).

Flags:

- `enforce_column_order` (default: false)
  - When true, physical column order in Postgres must match the Rust spec order.
  - Any order mismatch becomes rewrite-required (online/offline depending on size).
- `enforce_exact_columns` (default: false)
  - When true, the live table must contain exactly the columns in the spec (no extras).
  - Extra DB columns become rewrite-required.
- `allow_destructive_drops` (default: false)
  - When false, strict-exact will **fail** if extras exist (to avoid unintentional column removal).
  - When true, extras are removed from the **live** table via rewrite, but the backup table retains the original data (zero-loss-by-backup).

Guarantee (strict enabled + allowed):
- After migration, the **live** table matches the spec:
  - column set, column order, types, nullability, constraints (as supported).
  - any “extra” columns are present only in the retained backup table.

## 4) Strategy selection rules

Define deterministic rules to choose a migration strategy:

- Small table threshold(s):
  - default: up to 1,000,000 rows → offline may be allowed (configurable)
- Large table threshold(s):
  - above threshold → online migration required for any rewrite-class change
- Online migration prerequisites:
  - stable primary key or unique key suitable for chunked backfill
  - triggers permitted (or explicit app dual-write contract)
  - ability to run backfill batches without exceeding lock budget

## 4.1 Online rewrite performance levers (opt-in)

These are opt-in knobs; defaults are conservative and deterministic.

- Adaptive batching:
  - `adaptive_chunk` (default: false)
  - `online_chunk_size_min`, `online_chunk_size_max`, `online_target_batch_ms`
  - Current scope: applies to **changelog catch-up** batching (not backfill) to keep backfill performance stable and predictable.
- Session tuning (must be explicit and documented):
  - `synchronous_commit_off` (default: false; applies only to backfill/catchup work, never cutover lock validation)
  - Contract: backfill/catch-up commits may acknowledge before WAL flush; cutover still enforces correctness and lock-budget constraints.
  - Note: this is storage/load dependent and may not improve throughput; must be validated per deployment.
- Parallel backfill (future / optional):
  - `parallel_backfill` (default: false) + `parallel_backfill_workers`
  - Supported initially for `BIGINT` primary keys (range partitioning); UUID parallel range partitioning is deferred.
  - Requirement: pool max connections should be sized to at least `parallel_backfill_workers + 2` to avoid queueing.

## 5) Algorithms (must be auditable)

### 5.1 Offline zero-loss (table copy + swap)

Define:

- Steps (SQL per step)
- Locks expected per step
- Verification method (row counts + optional checksums)
- Backup retention policy
- Rollback behavior

### 5.2 Online zero-loss (backfill + short cutover)

Define:

- Shadow table creation
- Dual-write mechanism (if any): triggers preferred (default); app dual-write optional
- Backfill batching plan (chunk size, ordering, retry policy): deterministic chunking by PK order; retry with backoff; throttleable
- Verification and reconciliation
- Cutover sequence (minimal lock)
- Failure recovery and resumability

## 5.3 Online rewrite algorithm (current implementation)

Key properties:

- Shadow table is created with spec schema (order is spec order).
- Trigger writes a typed PK changelog (no shadow writes in trigger to avoid deadlocks).
- Backfill uses **range-boundary keyset** batches (`pk > lo AND pk <= hi`) to minimize round trips and allocations.
- Catch-up applies changelog in **range** batches (same boundary approach).
- Cutover:
  - acquire `ACCESS EXCLUSIVE` on the source table
  - stop changelog writes (drop/disable trigger)
  - drain changelog to empty within lock budget
  - swap tables and keep backup table

## 6) Type conversion contract (mandatory)

- Define “safe cast” rules (what is allowed).
- For each unsupported conversion, define error behavior.
- Compressed columns:
  - The system must never create undecodable bytes.
  - Any legacy/non-envelope `BYTEA` must be treated as **unknown** unless explicitly supported by spec.

## 7) Determinism contract

- Planning determinism: identical inputs ⇒ identical plan output.
- Execution determinism: same DB state ⇒ same SQL sequence (timestamps excluded).

## 8) Failure contract

List deterministic error variants and when they occur.

## 9) Performance budgets

Define:

- planning budget at N columns / M indexes: <= 50ms at 1,000 columns and 200 indexes (target)
- offline migration budget thresholds: only for <= 1,000,000 rows by default; configurable
- online migration backfill throughput targets: >= 50k rows/sec typical; throttleable
- cutover lock budget: <= 5 seconds

## 9.1 Workspace / allocation plan (mandatory)

List hot paths and how they avoid allocations:

- Plan building:
- SQL generation:
- Backfill chunking:
- Verification:

## 9.2 Optimization roadmap (tracked)

High-impact improvements (in priority order):

1. Typed changelog + typed ordering (no `ORDER BY pk::text`)
2. Range-based catch-up (avoid `ANY($1::uuid[])` lists)
3. Reduce per-round DB round-trips (fuse boundary selection + apply + clear without heavy `RETURNING`)
4. Adaptive chunk sizing (opt-in)
5. Optional session tuning: `synchronous_commit=off` during backfill/catch-up (opt-in)
6. Parallel backfill (opt-in; numeric PK first; UUID partitioning deferred)

Each item must include:
- correctness test(s)
- perf trial entry in `protocols/orsx2_evidence/migration_trials.md`

## 10) Test plan mapping

For each supported change, list:

- unit test(s) (no DB)
- integration test(s) (requires Postgres)
- benchmark(s) (if applicable)

## 11) Reuse report

- Keywords:
- Paths searched:
- Candidates:
  - path → what it provides → reuse/extend/reject → justification

## Appendix — Indexes and uniqueness

Indexes (including composite unique semantics) are specified separately in:

- `protocols/orsx2_specs/INDEXES_AND_UNIQUENESS_SPEC.md:1`

## Appendix — Add-ons v1.2

Optional migration-related expansions (migration key for composite/no-PK tables, multi-schema, advanced index features) are grouped in:

- `protocols/orsx2_specs/ADDONS_V1_2_SPEC.md:1`
