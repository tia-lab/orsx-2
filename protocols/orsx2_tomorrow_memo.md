# ORSX2 — Handoff Memo (for next working session)

Date: 2026-01-14 (UTC)  
Repo: `orsx-2` (workspace: `orsx`, `orsx-macros`)  
Tone: operational / no marketing

## 0) Non‑negotiables (must follow)

1. **No git operations**: do not run `git` commands or modify git state.
2. **No panics in library code**: no `unwrap/expect/panic/todo/unreachable` outside tests/benches.
3. **Deterministic-by-default**:
   - migrations must produce stable plans/SQL for identical inputs (timestamps only for derived backup/shadow names),
   - columnar output order matches SQL query order.
4. **Zero-loss migrations** means: any rewrite keeps a **backup** table containing the original data.
5. **Evidence required for performance claims**: append-only logs under `protocols/orsx2_evidence/`.

## 1) What ORSX2 currently is (high-level)

The current ORSX2 implementation provides:

1. **Schema-driven migrations** via `#[derive(OrsxMigrate)]` (crate `orsx-macros`) and `orsx::Migrations` (crate `orsx`).
2. **Online rewrite path** for large/rewrite-class changes:
   - shadow table with target schema,
   - triggers record PK changes into a typed changelog table,
   - keyset-chunk backfill + catch-up,
   - short cutover lock (budgeted) to drain changelog and swap,
   - backup table is retained.
3. **Columnar retrieval** into `ColumnarBatch`:
   - fast path: `COPY (SELECT ...) TO STDOUT (FORMAT BINARY)` parsed into typed column buffers,
   - fallback path: row-wise `sqlx::Row` decode into the same `ColumnarBatch`,
   - **auto mode** picks COPY vs row-wise based on query shape (still returns `ColumnarBatch`).
4. **Binary transport** for columnar batches: ORSXCOL v1 (`encode_orsxcol_v1_into`, `decode_orsxcol_v1(_into)`).
5. **Numeric vector compression** stored as `BYTEA` with a small envelope: `Compressed<T>`.

Reference: `README.md:1` (repo-level usage + current evidence numbers).

## 2) Required protocol reading (order matters)

Read these first (repo-scoped process + gates):

1. `AGENTS.md:1`  
   - contains non-negotiables + repo standards.
2. `protocols/orsx2_rewrite_protocol.md:1`  
   - spec → implement → tests → evidence → ready gates for the rewrite.
3. `protocols/orsx2_columnar_protocol.md:1`  
   - spec → implement → tests → evidence → ready gates for columnar.

Then use these as the “contracts to satisfy”:

4. `protocols/orsx2_specs/MIGRATIONS_SPEC.md:1`
5. `protocols/orsx2_specs/COLUMNAR_RETRIEVAL_SPEC.md:1`
6. `protocols/orsx2_specs/TESTING_AND_BENCH_PLAN.md:1`

Evidence logs (append-only; never edit existing entries):

7. `protocols/orsx2_evidence/migration_trials.md:1`
8. `protocols/orsx2_evidence/columnar_trials.md:1`

Style / code-quality reference only (do not copy headers verbatim):

9. `protocols/examples/`
10. `protocols/module_creation_protocol.md:1`

## 3) Current state snapshot (important files)

### Migrations

- Main entry: `orsx/src/migrations/mod.rs`
- Planning + strictness: `orsx/src/migrations/planning.rs`
- Online rewrite: `orsx/src/migrations/online.rs`
- Introspection: `orsx/src/migrations/introspection.rs` (assumes `public` schema)
- Config: `orsx/src/migrations/config.rs` (`MigrationConfig`)

Key correctness tests:

- Strict/order/rename: `orsx/tests/migrations_strict_correctness.rs`
- Big-table rewrite: `orsx/tests/migrations_online_big_uuid.rs` (ignored; perf trials)
- Perf comparisons: `orsx/tests/migrations_big_strict_compare.rs` (ignored; perf trials)

### Columnar

- Columnar API: `orsx/src/columnar/mod.rs`
- Batch + readers: `orsx/src/columnar/types.rs`
  - `CopyBinaryBatchReader` (COPY BINARY)
  - `RowWiseBatchReader` (row-wise → ColumnarBatch)
  - `ColumnarBatchReader` + `ColumnarReaderMode::Auto(ColumnarAutoConfig)`
- ORSXCOL v1: `orsx/src/columnar/orsxcol.rs`

Key tests:

- DB correctness: `orsx/tests/columnar_copy_binary.rs`
- Perf harness: `orsx/tests/columnar_perf_trials.rs` (ignored; used for evidence)
- Derive schema: `orsx/tests/columnar_derive.rs` (`#[derive(orsx::OrsxColumnar)]`)

Evidence:

- `protocols/orsx2_evidence/columnar_trials.md`

### Macros

- `#[derive(OrsxMigrate)]`: `orsx-macros/src/lib.rs`
- `#[derive(OrsxColumnar)]`: `orsx-macros/src/lib.rs`

## 4) How we work (workflow + gates)  **VERY IMPORTANT**

This repo is run under a strict process. Do not “just implement”.

### Phase A — Intake (NO CODE)

1. Restate the target (what change, what is in-scope/out-of-scope).
2. Identify constraints:
   - correctness invariants,
   - determinism requirements,
   - performance budgets + target table/query shapes.
3. Identify acceptance tests and perf evidence that must be produced.

### Phase B — Spec (NO CODE)

1. Update/finish the relevant spec file(s) under `protocols/orsx2_specs/`.
2. Ensure the spec has:
   - supported change set/type mapping,
   - failure contract,
   - determinism contract,
   - performance budgets + evidence gates.
3. No “TBD” on contracts/budgets at exit.

### Phase C — Implementation (CODE)

1. Implement the smallest viable slice that satisfies the spec.
2. Keep the public API minimal.
3. Keep hot paths allocation-disciplined:
   - `*_into(...)` variants,
   - workspace reuse where relevant.

### Phase D — Correctness tests (REQUIRED)

1. Unit tests where possible (no DB).
2. DB integration tests for Postgres semantics:
   - migrations: correctness of schema and data preservation,
   - columnar: value equality between COPY vs row-wise decode and NULL semantics.

### Phase E — Performance evidence (REQUIRED, release)

1. Run the perf harness(es) in `--release`.
2. Append results to the relevant evidence log:
   - migrations: `protocols/orsx2_evidence/migration_trials.md`
   - columnar: `protocols/orsx2_evidence/columnar_trials.md`
3. Never delete/modify old entries, even if “bad”.

### Phase F — READY gate

Only mark a feature “ready” when:

- spec is complete,
- tests cover the failure contract,
- evidence exists for target workloads,
- behavior is documented in `README.md`.

## 5) Environment / running

### Postgres

Tests use:

- `ORSX_TEST_DATABASE_URL` (default: `postgresql://orsx:orsx@localhost:15432/orsx2_test`)

Some migration perf tests create:

- `CREATE EXTENSION IF NOT EXISTS "uuid-ossp"` (needed for `uuid_generate_v1mc()`).

### Common commands

Non-DB unit tests:

- `cargo test -p orsx --lib`

DB correctness:

- `cargo test -p orsx --test columnar_copy_binary --release`
- `cargo test -p orsx --test migrations_strict_correctness`

Ignored perf harnesses (run only when producing evidence):

- Columnar:
  - `ORSX_COL_ROWS=100000 ORSX_COL_COLS=500 cargo test -p orsx --test columnar_perf_trials --release -- --ignored --nocapture`
- Migrations (big-table):
  - `ORSX_BIG_ROWS=1000000 ORSX_BIG_WRITER_ROWS=100000 cargo test -p orsx --release --test migrations_online_big_uuid -- --ignored --nocapture`

## 6) Key known gaps / next work (tomorrow)

The hot topic is **primary keys and unique constraints** for online rewrite robustness.

### Current limitation

Online rewrite currently requires exactly one primary key column. Composite PKs and multi-column uniqueness are not first-class in the online algorithm.

### Proposed direction (preferred)

Adopt a **single migration key** for online rewrite:

- If table has a single-column PK suitable for chunking, use it.
- Otherwise, introduce an internal column (example) `__orsx_mig_id` (UUID or BIGINT identity) with a unique index, and use it for:
  - changelog entries,
  - keyset chunking,
  - join/apply in catch-up.

Keep logical constraints (composite PK / unique) as constraints/indexes on the final table, but keep the online algorithm keyed to one column.

### Decisions required before implementation

1. Is adding an internal `__orsx_mig_id` allowed for tables without a single-column PK?
2. For uniqueness enforcement on large tables:
   - acceptable to use `CREATE UNIQUE INDEX CONCURRENTLY` as the primary mechanism?
   - is a named `UNIQUE CONSTRAINT` required, or is a unique index enough?
3. Are DEFERRABLE constraints required? (If yes, online path becomes harder; may require offline/lock-heavy operations.)

### Deliverables (if greenlit)

- Update spec: `protocols/orsx2_specs/MIGRATIONS_SPEC.md` (supported change set + strategy selection for PK/unique).
- Implement:
  - migration-key detection/injection,
  - online rewrite keyed to migration key,
  - tests for composite PK table (algorithm still works via migration key),
  - evidence entry in `protocols/orsx2_evidence/migration_trials.md`.

## 7) Performance evidence already available (do not restate without citing logs)

Use the append-only evidence logs as the source of truth:

- Columnar: `protocols/orsx2_evidence/columnar_trials.md`
  - includes 100k×50, 100k×500, and 1M×50 release numbers.
- Migrations: `protocols/orsx2_evidence/migration_trials.md`
  - includes 1M-row UUID PK online rewrite under write load and strictness comparisons.

If new changes affect performance, add new entries; do not edit old ones.

