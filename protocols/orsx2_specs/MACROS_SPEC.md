# ORSX2 — Macros Spec (TEMPLATE)

Status: DRAFT  
Owner: (Designer)

Decision point: do we ship `#[derive(Orsx2Migrate)]` (proc macro), or use an explicit builder/schema DSL?

## 1) Identification

- Purpose: derive schema metadata from Rust structs with minimal developer friction.
- Non-scope: ORM-like query generation.

## 2) Attribute grammar (must be formal)

Define exact supported attributes, examples, and rejection cases.

## 3) Type mapping rules

Define exact mapping from Rust types to Postgres types, including:

- `Option<T>` nullability
- arrays
- `pgvector`
- compressed types (stored as `BYTEA`, envelope format)

## 4) Security contract

- Identifier quoting policy
- No string-concatenated SQL generation without quoting

## 5) Failure contract

- Unsupported type:
- Unsupported attribute combination:
- Multiple primary keys:

## 6) Test plan mapping

- Snapshot-style tests for generated metadata:
- Compile-fail tests (if used):
- Integration tests (if SQL emitted changes):

