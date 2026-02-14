# `orsx` — Inventory (DRAFT)

Protocol: `protocols/inventory_template.md`

This inventory lists what currently exists in the `orsx/` crate. It describes only implemented code (no legacy, no future plans).

---

## 0) Module documentation (artifacts)

- Inventory (this file): `docs/inventory.md`
- Evidence logs (append-only): `docs/evidence/`
  - `docs/evidence/bench_results.md`
  - `docs/evidence/migration_trials.md`
  - `docs/evidence/columnar_trials.md`
- Specs: `docs/specs/`
  - `docs/specs/orsx_FLATTENED_WIDE_SCHEMA_PROTOCOL_SPEC.md`

## 1) Source Files

- `src/lib.rs`: crate entrypoint (module exports + public re-exports; `prelude`; `quote_identifier`).
- `src/error.rs`: crate error type (`Error`) and `Result<T>` alias.
- `src/config.rs`: local test configuration helper (`ORSX_TEST_DATABASE_URL` defaulting).
- `src/indexes.rs`: index metadata types (`IndexInfo`, `IndexType`) used by migration planning.
- `src/schema.rs`: table/column specs (`TableSpec`, `ColumnSpec`) and `OrsxMigrate` trait (`create_table_sql`).
- `src/types.rs`: `FieldType` enum and deterministic SQL type mapping (`to_sql()`).
- `src/compression.rs`: versioned compression envelope format (magic/version/type ids + CRC32) and parse/build helpers.
- `src/compressed.rs`: `Compressed<T>` vector wrapper + workspace reuse + sqlx `BYTEA` encode/decode using cydec.
- `src/flatten.rs`: flatten visitor trait (`OrsxValueVisitor`) and sqlx binder adapter (`PgArgumentsVisitor`).
- `src/columnar/mod.rs`: columnar module entrypoint (re-exports + `OrsxColumnar` trait).
- `src/columnar/types.rs`: columnar batch representation + COPY BINARY and row-wise batch readers (buffered, wide-table oriented).
- `src/columnar/orsxcol.rs`: ORSXCOL v1 binary encoding/decoding for `ColumnarBatch` (validity + fixed/var columns).
- `src/migrations/mod.rs`: migrations orchestrator (create-table + safe alters + optional online rewrite + index enforcement).
- `src/migrations/config.rs`: migration tuning and safety/determinism knobs (`MigrationConfig`).
- `src/migrations/introspection.rs`: schema/index introspection helpers (table existence, columns, constraints, index identities).
- `src/migrations/planning.rs`: deterministic schema diffing + strictness checks + safe alter/index planning.
- `src/migrations/online.rs`: online rewrite engine (shadow table + changelog trigger + backfill/catch-up + cutover swap).

- `benches/compression.rs`: criterion benchmark for `Compressed<T>` envelope encode/decode throughput.
- `benches/flatten.rs`: criterion benchmark for flatten visitor order + sqlx argument binding adapter overhead.
- `benches/planning.rs`: criterion benchmark for schema diff/planning cost on wide synthetic specs.

- `tests/db_smoke.rs`: DB connectivity smoke test against `ORSX_TEST_DATABASE_URL`.
- `tests/compression_envelope.rs`: unit tests for compression envelope round-trip and corruption rejection.
- `tests/compression_db_roundtrip.rs`: DB round-trip test for `Compressed<f64>` stored as `BYTEA` via sqlx.
- `tests/columnar_derive.rs`: unit test for `#[derive(OrsxColumnar)]` schema generation and stable index constants.
- `tests/columnar_copy_binary.rs`: integration test comparing COPY BINARY decoding vs row-wise decoding on mixed types.
- `tests/columnar_jsonb.rs`: integration test for JSONB text handling parity between COPY BINARY and row-wise paths.
- `tests/columnar_row_wise_preflight.rs`: integration tests for row-wise reader preflight (count/name/type validation).
- `tests/columnar_perf_trials.rs`: ignored perf trial harness for COPY BINARY vs row-wise (operator-run; env sized).
- `tests/flatten_runtime.rs`: unit tests for flattened column order, schema hash, and binder arity via visitor.
- `tests/flatten_trybuild.rs`: trybuild harness for flatten macro compile-pass/fail UI tests.
- `tests/flatten_db_integration.rs`: DB integration test for flatten-generated schema, migrations, and insert binding order.
- `tests/migrations_create_table.rs`: migration smoke test for create-table + index creation from `#[derive(OrsxMigrate)]`.
- `tests/migrations_add_nullable_columns.rs`: migration test for safe `ADD COLUMN` and uniqueness via concurrent unique index.
- `tests/migrations_indexes_idempotency.rs`: migration tests for unique/composite index creation and idempotency.
- `tests/migrations_strict_correctness.rs`: migration tests for strict column order/exact columns and safe rename behavior.
- `tests/migrations_online_rewrite.rs`: migration test for online rewrite when adding NOT NULL with default under concurrent writes.
- `tests/migrations_online_big_bigint.rs`: integration harness for online rewrite on a large BIGINT-PK table (chunking/catch-up).
- `tests/migrations_online_big_uuid.rs`: integration harness for online rewrite on a large UUID-PK table (chunking/catch-up).
- `tests/migrations_online_parallel_bigint.rs`: integration harness for opt-in parallel backfill path on BIGINT-PK tables.
- `tests/migrations_online_no_pk_migration_key.rs`: integration harness for online rewrite using `__orsx_mig_id` when spec has no single PK.
- `tests/migrations_big_strict_compare.rs`: integration harness for strict compare/order behavior on wide schemas (env-tunable).

- `tests/ui/flatten_ok.rs`: trybuild success case for `#[orsx_flatten_module]` schema generation.
- `tests/ui/flatten_fail_invalid_family_prefix.rs`: trybuild compile-fail for invalid `#[orsx_family(prefix = ...)]` values.
- `tests/ui/flatten_fail_invalid_processor_id.rs`: trybuild compile-fail for invalid `#[orsx_processor_id(...)]` values.
- `tests/ui/flatten_fail_metric_collision.rs`: trybuild compile-fail for flattened metric column name collisions.
- `tests/ui/flatten_fail_optional_struct_family.rs`: trybuild compile-fail for unsupported optional struct families.
- `tests/ui/flatten_fail_provenance_metric_collision.rs`: trybuild compile-fail for provenance/metric collision detection.
- `tests/ui/flatten_fail_unsupported_leaf_type.rs`: trybuild compile-fail for unsupported flattened leaf field types.

## 2) Public API Surface

The public API surface is defined by the exports in:

- `src/lib.rs`

Primary entrypoints include:

- migrations: `Migrations::init`, `Migrations::init_with_config`, `OrsxMigrate`, `TableSpec`, `ColumnSpec`
- compression: `Compressed<T>`, `CompressedWorkspace`
- columnar: `ColumnarSchema`, `ColumnarBatch`, `CopyBinaryBatchReader`, `RowWiseBatchReader`
- flatten: `OrsxValueVisitor`, `PgArgumentsVisitor`, `orsx_flatten_module`

## 3) Workspace / `*_into` APIs

Workspace/buffer-reuse APIs intended for hot paths include:

- `CompressedWorkspace` + `Compressed<T>::encode_envelope_into`
- columnar decode/encode `*_into` variants (orsxcol) and `ColumnarBatchReader::next_batch_into`
- `PgArgumentsVisitor` as a zero-surprise adapter over `sqlx::postgres::PgArguments`

## 4) Determinism Tier and Epsilon Policy

Tier: A (exact) by default for:

- compression envelope bytes (versioned format + CRC32; no lossy numeric conversions)
- schema diff planning (explicit stable ordering)
- columnar decoding and ORSXCOL batch encoding

Opt-in nondeterminism (explicitly configured) exists for throughput-only paths:

- adaptive online chunk sizing and parallel backfill in migrations (`MigrationConfig`)

## 5) Benchmarks (harness files)

- criterion benches: `benches/compression.rs`, `benches/flatten.rs`, `benches/planning.rs`
- operator-run perf harness: `tests/columnar_perf_trials.rs` (ignored by default)

