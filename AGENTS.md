# Agent Guidelines for `orsx`

This repository is a Rust workspace implementing a thin PostgreSQL layer over `sqlx`, focused on:

- **Zero-loss schema migrations** driven from Rust structs (`#[derive(OrsxMigrate)]`).
- **Transparent compression** for numeric vectors stored as `BYTEA` (`Compressed<T>` using `cydec`).

These guidelines exist to keep changes **safe**, **auditable**, and **non-surprising** for production databases.

## Protocols (must-follow)

Rewrite work for `orsx2` must follow these documents:

- `protocols/orsx2_rewrite_protocol.md:1` (end-to-end process, gates, evidence logs)
- Spec templates to fill before implementation:
  - `protocols/orsx2_specs/MIGRATIONS_SPEC.md:1`
  - `protocols/orsx2_specs/SCHEMA_INTROSPECTION_SPEC.md:1`
  - `protocols/orsx2_specs/COMPRESSION_SPEC.md:1`
  - `protocols/orsx2_specs/MACROS_SPEC.md:1`
  - `protocols/orsx2_specs/TESTING_AND_BENCH_PLAN.md:1`
- Append-only evidence logs (required for perf/safety claims):
  - `protocols/orsx2_evidence/bench_results.md:1`
  - `protocols/orsx2_evidence/migration_trials.md:1`

Legacy protocol/examples used as style reference (do not copy their legal headers into this repo):

- `protocols/module_creation_protocol.md:1`
- `protocols/examples/`

## Repo map

- `Cargo.toml`: workspace root.
- `orsx/`: main library crate (public API, migrations, compression, traits).
- `orsx-macros/`: proc-macro crate that implements `#[derive(OrsxMigrate)]`.
- `tests/`: integration tests (many require a running PostgreSQL).
- `examples/`, `benches/`: usage demos and performance benchmarks.

## Non-negotiables (safety + correctness)

- **No silent data loss**: migration changes must preserve existing data (backup + verification) or fail loudly.
- **No silent corruption**: do not introduce “convenient” conversions that produce values that will later fail to decode.
- **No SQL injection**: never concatenate untrusted strings into SQL without strict identifier quoting.
- **No panics in library code**: avoid `unwrap/expect/panic/todo/unreachable` outside tests/benches.
- **Determinism**: same inputs + schema should produce the same migration plan and SQL (ignoring timestamps in backup names).

## Code standards (mandatory)

These are required patterns for `orsx2` code (modeled after `protocols/examples/`):

- **Zero-copy APIs**: provide `*_into(...)` variants for hot paths and allocating wrappers only as convenience.
- **Workspace pattern**: for repeated calls, provide `*_into_with_workspace(..., ws: &mut ...)` and a `Workspace::{with_capacity, prepare}` API that reuses buffers deterministically.
- **Checked arithmetic**: any size math uses `checked_*` (fail with deterministic error on overflow).
- **Deterministic ordering**: never rely on hash iteration order; define explicit order keys + tie-breaks.
- **Allocation discipline**: pre-allocate with `Vec::with_capacity`; avoid `clone/to_vec/collect` in per-row loops unless justified + benchmarked.
- **Parallelism**: default path deterministic; any parallel path is opt-in and must document its determinism contract.

## When changing migrations (`orsx/src/migrations/*`)

- Keep the **zero-loss algorithm** invariant:
  - create temp table, copy data, rename old -> backup, rename temp -> original, verify.
- Any schema inference or comparison change must come with:
  - a **unit test** for the comparison/inference logic (no DB), and
  - an **integration test** for the behavior against PostgreSQL when applicable.
- Be explicit about **type conversions**:
  - If a conversion cannot be guaranteed correct, fail the migration with a clear error.
  - Do not treat arbitrary bytes/UTF-8 JSON as a valid compressed payload.

## When changing compression (`orsx/src/types/compressed.rs`)

- Maintain these invariants:
  - Encode: `Vec<T>` → cydec-compressed bytes
  - Decode: cydec bytes → original `Vec<T>` (round-trip exactness for supported types)
- Add tests for:
  - round-trip correctness (`INSERT` then `SELECT`) and
  - failure behavior on invalid payloads (must return an error, not panic).

## When changing the derive macro (`orsx-macros/src/lib.rs`)

- Changes must be validated by:
  - `cargo test --test core_functionality` (macro output expectations), and
  - DB integration tests if SQL emitted/consumed changes.
- Avoid parsing attributes via string matching when correctness matters; prefer structured parsing.
- Do not add new “magic defaults” without documenting them in `README.md`.

## Testing / running

- Non-DB tests:
  - `cargo test --test core_functionality`
- DB tests (requires PostgreSQL):
  - set `TEST_DATABASE_URL`
  - `cargo test --test integration_tests -- --ignored`
  - `cargo test --test batch_operations`

## Documentation expectations

- If you change public behavior, update `README.md` with:
  - what changed,
  - migration/compat impact,
  - how to test it locally.

## Scope discipline

- Keep changes **surgical**: do not refactor unrelated code while fixing a specific behavior.
- Prefer improving the **root cause** (schema inference/comparison/conversion safety) over adding workarounds.
