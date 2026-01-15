# ORSX2 — Indexes and Uniqueness Spec

Status: DRAFT  
Owner: (Designer)  
Applies to: `orsx2::migrations` + `orsx-macros` + schema introspection/planning

## 1) Purpose

Define how ORSX2:

- declares indexes/uniqueness in Rust (struct attributes),
- introspects existing indexes/uniqueness from Postgres,
- plans and applies changes **idempotently**,
- chooses safe/online strategies for large tables,
- keeps behavior deterministic and auditable.

This spec is focused on operational correctness and repeatability (rerunning migrations should not create duplicate indexes or oscillate strategies).

## 2) Definitions

- **Uniqueness semantics**: “the database enforces uniqueness for a set of columns”.
  - In Postgres, this can be provided by a `UNIQUE INDEX` or a `UNIQUE CONSTRAINT`.
- **Canonical index identity** (for idempotency): `(table, method, is_unique, columns[])` where:
  - `method` is the access method (`btree`, `gin`, `gist`, `hash`),
  - `columns[]` is the ordered column list as stored in the index definition.
- **Composite unique**: uniqueness on multiple columns, e.g. `UNIQUE (tenant_id, email)`.

## 3) Non-negotiable contracts

1. **Idempotency**: running `Migrations::init*` repeatedly must not create duplicate indexes and must converge to a stable schema.
2. **Determinism**: given the same DB state + Rust spec, the planned actions must be stable (ordering and chosen strategy).
3. **Large-table safety**:
   - index creation on existing tables defaults to `CONCURRENTLY` to avoid long write blocks,
   - avoid `ALTER TABLE ... ADD CONSTRAINT` on hot/large tables unless explicitly configured.
4. **No silent weakening**: if the spec requests uniqueness, ORSX2 must either:
   - ensure uniqueness semantics exist, or
   - fail with a deterministic error explaining why (e.g. duplicate data).
5. **No SQL injection**: all identifiers are quoted using the single audited quoting function.

## 4) Current implementation (baseline)

Baseline behavior in the current code (to preserve unless intentionally changed):

- Single-column uniqueness declared via `#[orsx_column(unique)]` is enforced on existing tables using:
  - `CREATE UNIQUE INDEX CONCURRENTLY IF NOT EXISTS orsx_uq_{table}_{column} ...`
  - This treats “unique index” as the operational primitive.
- `IndexInfo` entries from the spec are ensured with:
  - `CREATE [UNIQUE] INDEX CONCURRENTLY IF NOT EXISTS ...` on existing tables
  - `CREATE [UNIQUE] INDEX IF NOT EXISTS ...` on new tables

Reference: `orsx/src/migrations/planning.rs`.

## 5) Desired capability additions (next work)

### 5.1 Table-level composite indexes and composite unique

Add a table-level declaration mechanism so a struct can declare:

- multi-column unique indexes (composite unique),
- multi-column non-unique indexes,
- method selection (btree/gin/gist/hash),
- optional explicit name (or deterministic auto-name).

This is required for cases like:

- `UNIQUE(tenant_id, email)`
- `INDEX(tenant_id, created_at)`

### 5.2 Idempotency by semantics (not only by name)

When applying indexes, ORSX2 must avoid duplicating an already-existing equivalent index even if the name differs.

Required behavior:

- If an equivalent index exists (same method, uniqueness, columns), treat the spec as satisfied.
- If not, create the index.

This is especially important for dynamic table-name overrides (`Migrations::init(..., Some("table_name"))`) where index names must be stable per table, but the database may already contain operator-created indexes.

## 6) Rust declaration format (spec)

### 6.1 Column-level attributes (supported)

- `#[orsx_column(primary_key)]`  
- `#[orsx_column(unique)]` (single-column uniqueness semantics)
- `#[orsx_column(index)]`
- `#[orsx_column(index(unique))]`
- `#[orsx_column(index(type = "gin"|"gist"|"hash"|"btree"))]`

Column-level `unique` and `index(unique)` are allowed simultaneously but redundant; the planner must treat them as a single uniqueness request.

### 6.2 Table-level index declarations (new)

Add a table-level attribute grammar. One acceptable form (exact syntax can change, but contracts cannot):

- `#[orsx_table("table_name", index(...), index(...))]`

Each `index(...)` must carry:

- `columns("c1", "c2", ...)` (>=1 columns; order matters)
- optional `unique`
- optional `type = "btree"|"gin"|"gist"|"hash"` (default `btree`)
- optional `name = "..."` (if omitted, deterministic auto-name is used)

Rejection cases:

- empty columns list,
- duplicate column names inside an index declaration,
- unknown index method,
- name longer than Postgres identifier limit after quoting rules are applied (see 7.2).

## 7) Naming rules (deterministic, safe for table-name overrides)

### 7.1 Goals

- Stable per table (override table names must result in different index names).
- Under Postgres 63-byte identifier limit.
- Deterministic and collision-resistant.

### 7.2 Required naming scheme

If `name` is explicitly provided, use it verbatim (after identifier quoting).

If auto-generated:

- prefix:
  - `orsx_uq_` for unique indexes
  - `orsx_ix_` for non-unique indexes
- include (in order):
  - table name
  - method (optional if non-btree)
  - joined column list

If the computed name exceeds 63 bytes:

- shorten deterministically by:
  - truncating the human-readable prefix, and
  - appending a stable hex hash suffix derived from the full canonical identity.

Contract:

- name generation must be deterministic and must not rely on RNG or timestamps.

## 8) Introspection requirements

To support idempotency-by-semantics and composite unique:

- introspection must read index definitions including:
  - uniqueness (`indisunique`),
  - method (btree/gin/gist/hash),
  - ordered key columns (for `indkey`),
  - table name and schema.
- the “canonical identity” for comparison must ignore:
  - the index name,
  - operator class details (for v1; can be extended later),
  - included columns (v1: not supported).

The initial scope may be “public schema only”, but must be documented as such.

## 9) Migration planning and application rules

### 9.1 New table creation

For a table that does not exist:

- `CREATE TABLE ...` first.
- Then create indexes without `CONCURRENTLY` (table is empty):
  - `CREATE [UNIQUE] INDEX IF NOT EXISTS ...`

### 9.2 Existing table — safe path (preferred)

For an existing table:

- Ensure requested indexes using:
  - `CREATE [UNIQUE] INDEX CONCURRENTLY IF NOT EXISTS ...`
- Ensure requested uniqueness semantics using:
  - unique index concurrently (not `ALTER TABLE ADD CONSTRAINT`) by default.

### 9.3 Existing table — rewrite path interaction

If a rewrite is required for other schema diffs:

Preferred (robust) sequence:

1. Create shadow table schema (columns and PK only; defer heavy indexes).
2. Backfill + catch-up.
3. Create required indexes **CONCURRENTLY on the shadow**:
   - unique indexes included here; if duplicates exist, index build fails deterministically and migration aborts before cutover.
4. Cutover swap.

This keeps uniqueness validation out of the cutover lock window.

### 9.4 “Constraint objects” policy

By default, ORSX2 treats a `UNIQUE INDEX` as sufficient for uniqueness semantics.

Optional future mode (explicit config) may attach named constraints offline:

- `ALTER TABLE ... ADD CONSTRAINT ... UNIQUE USING INDEX ...`

But this is not required for correctness and may not be large-table safe.

## 10) Failure contract (must be explicit)

ORSX2 must fail deterministically when:

- A requested unique index cannot be created due to duplicate rows.
- Index method is unsupported or does not match the spec.
- Identifier naming cannot produce a valid name (should be rare; treat as bug/error).
- The migration attempts an operation not supported in the chosen strategy (e.g. constraint attach under strict lock budget).

Errors must:

- identify the table,
- identify the index/columns,
- include the SQL that failed (when safe to include).

## 11) Determinism contract

- Index creation order is stable:
  - sort by `(unique first?, method, columns joined, name)` or an equivalent explicit key.
- Index names are deterministic for auto-generated names.
- If “equivalent index exists” checks are used, the matching logic must be deterministic and not depend on iteration order.

## 12) Test plan mapping (required)

### 12.1 DB integration tests

Add tests for:

1. **Single-column unique add**:
   - start without unique,
   - add `#[orsx_column(unique)]`,
   - run migration,
   - verify uniqueness semantics exist (unique index exists),
   - run migration again and verify no additional unique indexes are created.

2. **Composite unique (table-level)**:
   - declare `UNIQUE(a,b)` via table-level index declaration,
   - verify a unique index exists on `(a,b)`,
   - rerun migration and verify idempotency by semantics.

3. **Existing equivalent index with different name**:
   - create a unique index manually with a different name,
   - ensure ORSX2 does not create a duplicate when spec requests the same uniqueness.

4. **Table name override**:
   - apply the same struct spec to two different table names,
   - verify indexes/uniques are created on both and names do not collide.

### 12.2 Negative tests

- Attempt to add uniqueness when duplicates exist → must fail deterministically.

## 13) Performance and evidence requirements

If composite index support or semantic matching changes query patterns (extra introspection queries):

- measure planning latency impact for a table with:
  - ~500 columns
  - ~30 indexes
- record results in `protocols/orsx2_evidence/migration_trials.md` (append-only).

