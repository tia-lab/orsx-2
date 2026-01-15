# ORSX2 — Add-ons v1.2 Spec Bundle

Status: DRAFT  
Owner: (Designer)  
Scope: optional features ("add-ons") that are **not required** for the current baseline, but can be promoted into core scope by explicit decision.

This document groups multiple add-ons so they can be tracked as a single “v1.2” expansion set.

## 0) Baseline assumptions (what exists without add-ons)

Baseline ORSX2 (v1.0-ish) already provides:

- Migrations with an online rewrite path keyed by **exactly one** primary key column.
- Introspection and planning for the `public` schema.
- Index ensuring (single-column + multi-column via table-level `index(columns(...))`) with semantic idempotency: `(method, unique, ordered columns)`.
- Columnar retrieval for the current `ColumnarType` set (bool/i16/i32/i64/f32/f64/uuid/timestamptz/text/bytea) via COPY BINARY and row-wise, including auto selection.

## 1) Add-on A — Online rewrite without single-column PK (migration key)

### 1.1 Motivation

Online rewrite currently requires exactly one PK column. Many real schemas have:

- composite primary keys, or
- no primary key (but still need online migrations), or
- a “logical key” that is not suitable for keyset chunking.

### 1.2 Proposal

Introduce a **migration key** concept used only for online rewrite mechanics:

- If a table has a suitable single-column PK, use it.
- Otherwise, create and use an internal key column (example name):
  - `__orsx_mig_id BIGINT` (no Postgres extensions; preferred for this repo)

Chosen (v1.2): BIGINT, no extensions.

The migration key must have:

- a unique btree index (or be declared as a primary key on the shadow),
- stable values (never regenerated during rewrite),
- deterministic name and type.

### 1.3 Strategy rules

For online rewrite:

- Changelog stores the migration key (not composite PK tuples).
- Backfill chunking uses migration key keyset ranges.
- Catch-up applies by joining on migration key.

The final table can still keep:

- a composite PK,
- composite unique indexes,
- other constraints (as supported).

### 1.4 Failure contract

Fail deterministically if:

- the migration key cannot be added safely (policy decision),
- the key is nullable or not indexed when required,
- key values cannot be generated (missing extension/function).

### 1.5 Required tests/evidence

- DB integration: online rewrite works for a table with composite PK (using migration key).
- DB integration: table with no PK can be rewritten online (using migration key).
- Idempotency: re-running migrations does not create duplicate migration key indexes.
- Evidence: append a trial to `protocols/orsx2_evidence/migration_trials.md` with:
  - 200k and 1M row shapes, including write load.

### 1.6 Decision points

- Is adding an internal `__orsx_mig_id` allowed?
- Which type is preferred?
  - v1.2 choice: `BIGINT` (no extensions)
- Is it acceptable to keep the migration key permanently (recommended), or must it be removable?

## 2) Add-on B — Multi-schema support (beyond `public`)  (MOVED TO v1.3)

This add-on is moved to:

- `protocols/orsx2_specs/ADDONS_V1_3_MULTI_SCHEMA_SPEC.md:1`

Do not implement multi-schema under v1.2.

## 7) References

- v1.3 multi-schema spec: `protocols/orsx2_specs/ADDONS_V1_3_MULTI_SCHEMA_SPEC.md:1`

## 3) Add-on C — Advanced index matching (partial / expression / INCLUDE / opclass / collation)

### 3.1 Motivation

Baseline semantic matching uses `(method, unique, ordered columns)` only.
Real systems may use:

- partial indexes: `... WHERE ...`
- expression indexes: `((lower(email)))`
- included columns: `INCLUDE (...)`
- operator classes/collations

Without modeling these, ORSX2 cannot safely claim “equivalent index exists”.

### 3.2 Proposal

Extend “canonical index identity” to include, as configured:

- predicate (normalized text) for partial indexes,
- expression signature for expression indexes,
- included columns list,
- operator class/collation (optional, may be deferred).

### 3.2.1 v1.2 safety subset (implemented first)

Before adding any new Rust declaration syntax, ORSX2 must tighten idempotency matching so it does
not accidentally treat a partial or expression index as equivalent to a plain “index on columns(...)”
spec request.

Rules:

- A partial index (`WHERE ...`) is never considered equivalent to a non-partial index request.
- An expression index (`((...))`) is never considered equivalent to a plain column index request.

This may create redundant indexes in rare cases (e.g. predicate is tautological), but it avoids
silently weakening uniqueness semantics.

### 3.3 Scope control

This add-on should be gated behind an explicit config flag, because it increases complexity.

### 3.4 Required tests/evidence

- DB integration: existing partial index should not be treated as equivalent to a non-partial index.
- DB integration: expression index equivalence detection.

### 3.5 Decision points

- Which sub-features must be supported first: partial vs expression vs include?
- Do we want strict matching (exact predicate text) or normalized matching?

## 4) Add-on D — Expanded columnar type support

### 4.1 Motivation

Current columnar types are limited to common scalar + varlen.
Common requests include:

- `NUMERIC` (arbitrary precision)
- `JSONB`
- arrays

### 4.2 Proposal (v1.2 candidate list)

Pick a minimal next set:

- `JSONB` → `Bytes` (raw JSONB bytes) or `Utf8` (text) (must be explicit).
- `NUMERIC` → either:
  - reject (remain unsupported), or
  - encode as canonical string bytes (slower, but deterministic), or
  - encode as Postgres binary numeric (complex; likely future).
- Arrays: start with `FLOAT8[]` / `INT[]` if needed, encoded as a varlen column with a defined sub-format.

### 4.3 Required tests/evidence

- Unit: ORSXCOL encode/decode round-trips each new type with NULLs.
- DB: COPY vs row-wise equality for each new type.
- Evidence: columnar perf trial entry if new types are expected in large scans.

### 4.4 Decision points

- Do we need these types for columnar *now*, or can they stay “unsupported”?
- For JSONB: do we treat it as opaque bytes or UTF-8 text?

## 5) Add-on E — Row-wise strict preflight for columnar reads

### 5.1 Motivation

Row-wise columnar reader currently fails when `try_get` fails, but it does not provide
an explicit “preflight” error describing schema mismatches before scanning.

### 5.2 Proposal

Add an opt-in strict preflight mode for row-wise reads:

- validate `row.columns().len() == schema.len()`,
- optionally validate returned column names match `ColumnarField.name` when provided,
- optionally validate that the returned SQL types are compatible with the schema’s decode types,
- run once, on the first observed row (prevents per-row overhead).

Implementation surface:

- `RowWiseBatchReaderConfig { validate_column_count: bool, validate_column_names: bool }`
  - default: all `false` (opt-in)
- `RowWiseBatchReader::with_config(cfg)` to enable.
- `ColumnarBatchReader` must provide a constructor that can pass a `RowWiseBatchReaderConfig`
  through when `RowWise` is chosen (including `Auto(...)` choosing row-wise).

Preflight behavior:

- If both flags are `false`, preflight is a no-op.
- If `validate_column_count` is enabled and counts differ, return a deterministic error string:
  - `row-wise preflight failed: column count mismatch (expected N, got M)`
- If `validate_column_names` is enabled:
  - only fields with `ColumnarField.name = Some(...)` are checked,
  - mismatch returns:
    - `row-wise preflight failed: column name mismatch at index i (expected `x`, got `y`)`
- If `validate_type_compatible` is enabled:
  - use `sqlx::Type<Postgres>::compatible(...)` against the returned column type info,
  - mismatch returns:
    - `row-wise preflight failed: type mismatch at index i (expected <hint>, got <pg type name>)`

Limitations:

- Empty result sets cannot be preflighted (no first row), so the preflight does not run.
- Type mismatches are still detected by `try_get` during decoding (not preflighted).
  - If `validate_type_compatible` is enabled, type mismatches should be rejected earlier, but
    the final authority remains `try_get` (driver-level decode rules).

### 5.3 Required tests

- DB integration: mismatch column count triggers deterministic “preflight failed” error.
- DB integration: name mismatch triggers deterministic error when name checking is enabled.
  - Tests should avoid shared table names across parallel execution.

### 5.4 Decision points

- Is this required for production, or purely a debugging mode?

## 6) Promotion rules (when an add-on becomes “core”)

An add-on is promoted into core scope only when:

- its section here is moved/merged into the relevant core spec file(s),
- tests exist and pass,
- evidence is appended when performance is involved,
- README documents behavior and limitations.
