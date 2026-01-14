# ORSX2 — Migrations Spec (TEMPLATE)

Status: DRAFT  
Owner: (Designer)  
Applies to: `orsx2` crate

## 1) Identification

- Component: `orsx2::migrations`
- Purpose: schema-driven, zero-loss migrations for PostgreSQL at large-table scale.
- Non-scope: ORM, query builder, cross-database support.

## 2) Inputs and bounds (mandatory)

- Postgres versions supported:
- Table size bounds (rows, row width):
- Index expectations:
- Max acceptable cutover lock time:
- Write traffic during migration (allowed / not allowed):

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
- Large table threshold(s):
- Online migration prerequisites:
  - required PK/index
  - required ability to dual-write or use triggers
  - requirements on application write behavior (if any)

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
- Dual-write mechanism (if any): triggers or application-side
- Backfill batching plan (chunk size, ordering, retry policy)
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

- planning budget at N columns / M indexes:
- offline migration budget thresholds:
- online migration backfill throughput targets:
- cutover lock budget:

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
