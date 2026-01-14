# ORSX2 — Migrations Spec (TEMPLATE)

Status: DRAFT  
Owner: (Designer)  
Applies to: `orsx2` crate

## 1) Identification

- Component: `orsx2::migrations`
- Purpose: schema-driven, zero-loss migrations for PostgreSQL at large-table scale.
- Non-scope: ORM, query builder, cross-database support.

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
