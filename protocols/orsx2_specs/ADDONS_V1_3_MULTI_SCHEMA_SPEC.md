# ORSX2 — Add-ons v1.3: Multi-schema Support Spec

Status: DRAFT  
Owner: (Designer)  
Applies to: `orsx2::migrations`, schema introspection, identifier quoting, macros

## 1) Purpose

Add first-class support for Postgres schemas (namespaces) so ORSX2 can:

- create/migrate tables in a specified schema (not only `public`),
- introspect schema objects in that schema deterministically,
- perform online rewrite (shadow/backup/changelog/trigger objects) inside the same schema,
- keep behavior safe and idempotent with correct identifier quoting.

This add-on must be implemented end-to-end (macro → spec → SQL generation → introspection → tests).

## 2) Non-negotiable contracts

1. **No SQL injection**: schema/table/column identifiers must be quoted correctly.
2. **Determinism**: planning and object naming must be stable for identical inputs.
3. **Isolation**: migrating `schema_a.table_x` must not affect `schema_b.table_x`.
4. **Online rewrite locality**: shadow/backup/changelog and triggers must be created in the same schema as the source table.
5. **Idempotency**: rerunning migrations must converge (no duplicate objects) per `(schema, table)`.

## 3) Naming model

### 3.1 Qualified table identity

Table identity becomes:

- `schema`: `&str` (default `"public"` only when explicitly chosen)
- `table`: `&str`

### 3.2 Identifier quoting

Introduce a schema-aware quoting API:

- `quote_ident(name: &str) -> String` => `"name"`
- `quote_qualified(schema: &str, table: &str) -> String` => `"schema"."table"`

Rules:

- Never accept pre-quoted strings.
- Always quote schema and table separately.
- For derived objects (backup/shadow/etc), treat them as table identifiers in the same schema:
  - `"schema"."table__orsx2_shadow_..."` etc.

## 4) Rust declaration format (macro-level)

### 4.1 Primary syntax (recommended)

Extend `#[orsx_table(...)]` to accept:

- `schema = "..."` (optional; default is `"public"` if omitted)
- `name = "..."` (required if schema is specified and the first arg is not used)

Examples:

```rust
#[derive(orsx::OrsxMigrate)]
#[orsx_table(schema = "app", name = "users")]
struct User { ... }
```

### 4.2 Backward compatible syntax

Preserve current shorthand:

```rust
#[orsx_table("users")]
```

Interpretation: `schema = "public"`, `name = "users"`.

### 4.3 Rejected syntax (v1.3)

Do **not** allow `"schema.table"` as a single string identifier in v1.3.

Reason:
- it encourages ambiguous parsing and makes safe quoting harder across the codebase.

## 5) Schema creation policy

Decision point:

- Either ORSX2 creates schemas when requested, or it requires schemas to exist.

Default proposal:

- If `schema != "public"`, run:
  - `CREATE SCHEMA IF NOT EXISTS "schema"`

This must be optional/controllable if you want strict separation of responsibilities.

## 6) Introspection changes

All introspection queries must take `(schema, table)` and filter by `n.nspname = $schema`.

Required introspection functions (schema-aware):

- `table_exists(pool, schema, table)`
- `read_table_schema(pool, schema, table)`
- `read_table_index_identities(pool, schema, table)`

Contract:

- queries must be fully ordered deterministically (explicit `ORDER BY`).

## 7) Migration planning changes

### 7.1 API surface

Migrations API must support per-table schema overrides:

- current: `(T, Option<&str>)` table name override
- v1.3 extension: `(T, Option<QualifiedTableName>)`

Where:

```rust
struct QualifiedTableName<'a> { schema: &'a str, table: &'a str }
```

Alternative: keep the override as `Option<&str>` for table name only and use the schema from `T::spec()`. This is simpler but prevents one struct from being applied to multiple schemas.

Decision point:

- Do you need “same struct spec applied to multiple schemas”?

### 7.2 DDL generation

All DDL must use `quote_qualified(schema, table)` and not assume `public`.

### 7.3 Online rewrite

Derived objects (shadow/backup/changelog/trigger) must be created in the same schema.

Cutover swap uses schema-qualified names:

- `ALTER TABLE "schema"."src" RENAME TO "backup"`
- `ALTER TABLE "schema"."shadow" RENAME TO "src"`

Note: `ALTER TABLE ... RENAME TO` renames within schema; it does not change schema. That is desired.

## 8) Index/uniqueness interactions

Index idempotency-by-semantics must be applied per `(schema, table)`.

Index names must be unique only within a schema, but ORSX2 should still generate deterministic names incorporating:

- table name (not schema name necessarily), and
- stable hash if needed.

## 9) Failure contract

Fail deterministically when:

- schema name is invalid/empty,
- schema does not exist and auto-create is disabled,
- introspection returns multiple matches (should not happen if schema/table are explicit),
- any SQL execution fails (include SQL and object identity in error).

## 10) Tests and evidence requirements

### 10.1 DB integration tests (required)

Add tests that:

1. Create two schemas: `s1` and `s2`.
2. Use the same struct spec (same table name) to migrate both schemas.
3. Verify:
   - both tables exist in their schemas,
   - changing one schema does not affect the other,
   - indexes/unique are created in the correct schema,
   - online rewrite creates shadow/backup/changelog in the correct schema.

### 10.2 Evidence

If multi-schema adds additional introspection overhead, record planning latency deltas in:

- `protocols/orsx2_evidence/migration_trials.md`

## 11) Implementation checklist (ordered)

1. Add schema-aware quoting helpers and update all SQL generation to use them.
2. Extend macro parsing for `#[orsx_table(schema=..., name=...)]`.
3. Make introspection schema-aware.
4. Thread `(schema, table)` through planning and migrations entry points.
5. Update online rewrite to create/operate on schema-qualified objects.
6. Add DB integration tests.
7. Update README with schema examples and explicit non-goals/limitations.

