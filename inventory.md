# `orsx-2` — Global Inventory (GENERATED; DO NOT EDIT)

Generated: 2026-02-14T07:57:13Z
Protocol: `protocols/inventory_template.md`

This file is generated from per-component inventories under `*/docs/inventory.md` (workspace crates) and optionally `crates/*/docs/inventory.md` / `services/*/docs/inventory.md`.
If a component does not have a top-level `docs/inventory.md`, this generator may also include module inventories under `<component>/src/*/docs/inventory.md` when present.
If a file purpose is missing in a component inventory, this file will mark it as `INVENTORY GAP`.

## Components

- `crate::orsx`: `orsx/docs/inventory.md`
- `crate::orsx-macros`: `orsx-macros/docs/inventory.md`

---

## `orsx`

### Artifacts

- Inventory: `orsx/docs/inventory.md`
- Spec: `orsx/docs/specs/orsx_FLATTENED_WIDE_SCHEMA_PROTOCOL_SPEC.md`
- Evidence: `orsx/docs/evidence/bench_results.md`
- Evidence: `orsx/docs/evidence/columnar_trials.md`
- Evidence: `orsx/docs/evidence/migration_trials.md`
- Tests: `orsx/tests`
- Benches: `orsx/benches/compression.rs`
- Benches: `orsx/benches/flatten.rs`
- Benches: `orsx/benches/planning.rs`

### Source Files

- `orsx/benches/compression.rs`: criterion benchmark for `Compressed<T>` envelope encode/decode throughput.
- `orsx/benches/flatten.rs`: criterion benchmark for flatten visitor order + sqlx argument binding adapter overhead.
- `orsx/benches/planning.rs`: criterion benchmark for schema diff/planning cost on wide synthetic specs.
- `orsx/src/columnar/mod.rs`: columnar module entrypoint (re-exports + `OrsxColumnar` trait).
- `orsx/src/columnar/orsxcol.rs`: ORSXCOL v1 binary encoding/decoding for `ColumnarBatch` (validity + fixed/var columns).
- `orsx/src/columnar/types.rs`: columnar batch representation + COPY BINARY and row-wise batch readers (buffered, wide-table oriented).
- `orsx/src/compressed.rs`: `Compressed<T>` vector wrapper + workspace reuse + sqlx `BYTEA` encode/decode using cydec.
- `orsx/src/compression.rs`: versioned compression envelope format (magic/version/type ids + CRC32) and parse/build helpers.
- `orsx/src/config.rs`: local test configuration helper (`ORSX_TEST_DATABASE_URL` defaulting).
- `orsx/src/error.rs`: crate error type (`Error`) and `Result<T>` alias.
- `orsx/src/flatten.rs`: flatten visitor trait (`OrsxValueVisitor`) and sqlx binder adapter (`PgArgumentsVisitor`).
- `orsx/src/indexes.rs`: index metadata types (`IndexInfo`, `IndexType`) used by migration planning.
- `orsx/src/lib.rs`: crate entrypoint (module exports + public re-exports; `prelude`; `quote_identifier`).
- `orsx/src/migrations/config.rs`: migration tuning and safety/determinism knobs (`MigrationConfig`).
- `orsx/src/migrations/introspection.rs`: schema/index introspection helpers (table existence, columns, constraints, index identities).
- `orsx/src/migrations/mod.rs`: migrations orchestrator (create-table + safe alters + optional online rewrite + index enforcement).
- `orsx/src/migrations/online.rs`: online rewrite engine (shadow table + changelog trigger + backfill/catch-up + cutover swap).
- `orsx/src/migrations/planning.rs`: deterministic schema diffing + strictness checks + safe alter/index planning.
- `orsx/src/schema.rs`: table/column specs (`TableSpec`, `ColumnSpec`) and `OrsxMigrate` trait (`create_table_sql`).
- `orsx/src/types.rs`: `FieldType` enum and deterministic SQL type mapping (`to_sql()`).
- `orsx/tests/columnar_copy_binary.rs`: integration test comparing COPY BINARY decoding vs row-wise decoding on mixed types.
- `orsx/tests/columnar_derive.rs`: unit test for `#[derive(OrsxColumnar)]` schema generation and stable index constants.
- `orsx/tests/columnar_jsonb.rs`: integration test for JSONB text handling parity between COPY BINARY and row-wise paths.
- `orsx/tests/columnar_perf_trials.rs`: ignored perf trial harness for COPY BINARY vs row-wise (operator-run; env sized).
- `orsx/tests/columnar_row_wise_preflight.rs`: integration tests for row-wise reader preflight (count/name/type validation).
- `orsx/tests/compression_db_roundtrip.rs`: DB round-trip test for `Compressed<f64>` stored as `BYTEA` via sqlx.
- `orsx/tests/compression_envelope.rs`: unit tests for compression envelope round-trip and corruption rejection.
- `orsx/tests/db_smoke.rs`: DB connectivity smoke test against `ORSX_TEST_DATABASE_URL`.
- `orsx/tests/flatten_db_integration.rs`: DB integration test for flatten-generated schema, migrations, and insert binding order.
- `orsx/tests/flatten_runtime.rs`: unit tests for flattened column order, schema hash, and binder arity via visitor.
- `orsx/tests/flatten_trybuild.rs`: trybuild harness for flatten macro compile-pass/fail UI tests.
- `orsx/tests/migrations_add_nullable_columns.rs`: migration test for safe `ADD COLUMN` and uniqueness via concurrent unique index.
- `orsx/tests/migrations_big_strict_compare.rs`: integration harness for strict compare/order behavior on wide schemas (env-tunable).
- `orsx/tests/migrations_create_table.rs`: migration smoke test for create-table + index creation from `#[derive(OrsxMigrate)]`.
- `orsx/tests/migrations_indexes_idempotency.rs`: migration tests for unique/composite index creation and idempotency.
- `orsx/tests/migrations_online_big_bigint.rs`: integration harness for online rewrite on a large BIGINT-PK table (chunking/catch-up).
- `orsx/tests/migrations_online_big_uuid.rs`: integration harness for online rewrite on a large UUID-PK table (chunking/catch-up).
- `orsx/tests/migrations_online_no_pk_migration_key.rs`: integration harness for online rewrite using `__orsx_mig_id` when spec has no single PK.
- `orsx/tests/migrations_online_parallel_bigint.rs`: integration harness for opt-in parallel backfill path on BIGINT-PK tables.
- `orsx/tests/migrations_online_rewrite.rs`: migration test for online rewrite when adding NOT NULL with default under concurrent writes.
- `orsx/tests/migrations_strict_correctness.rs`: migration tests for strict column order/exact columns and safe rename behavior.
- `orsx/tests/ui/flatten_fail_invalid_family_prefix.rs`: trybuild compile-fail for invalid `#[orsx_family(prefix = ...)]` values.
- `orsx/tests/ui/flatten_fail_invalid_processor_id.rs`: trybuild compile-fail for invalid `#[orsx_processor_id(...)]` values.
- `orsx/tests/ui/flatten_fail_metric_collision.rs`: trybuild compile-fail for flattened metric column name collisions.
- `orsx/tests/ui/flatten_fail_optional_struct_family.rs`: trybuild compile-fail for unsupported optional struct families.
- `orsx/tests/ui/flatten_fail_provenance_metric_collision.rs`: trybuild compile-fail for provenance/metric collision detection.
- `orsx/tests/ui/flatten_fail_unsupported_leaf_type.rs`: trybuild compile-fail for unsupported flattened leaf field types.
- `orsx/tests/ui/flatten_ok.rs`: trybuild success case for `#[orsx_flatten_module]` schema generation.

---

## `orsx-macros`

### Artifacts

- Inventory: `orsx-macros/docs/inventory.md`

### Source Files

- `orsx-macros/src/lib.rs`: proc-macros implementing `#[derive(OrsxMigrate)]`, `#[derive(OrsxColumnar)]`, and `#[orsx_flatten_module]` (plus attribute parsing + validation).

---
