# ORSX2 — Schema Introspection Spec (TEMPLATE)

Status: DRAFT  
Owner: (Designer)

## 1) Identification

- Purpose: read Postgres schema reliably for migration planning.
- Non-scope: supporting non-public schemas unless explicitly required.

## 2) Objects introspected

- Tables:
- Columns (type, nullability, defaults):
- Constraints (PK/unique/FK):
- Indexes:
- Extensions required (if any):

## 3) Canonicalization rules

Define how types and metadata are normalized so comparisons are correct:

- Type aliases mapping (e.g. `TIMESTAMPTZ`):
- Array types:
- `pgvector` dimensions:
- Default expressions normalization:

## 4) Determinism contract

Queries must:

- return ordered results deterministically (explicit ORDER BY),
- produce stable representations for diffing.

## 5) Performance budgets

- Introspection latency budgets per table:
- Caching policy (if any):

## 5.1 Allocation plan (mandatory)

- How result ordering is enforced (explicit ORDER BY).
- How strings are normalized/canonicalized without excessive allocations.

## 6) Test plan mapping

- Unit tests for canonicalization:
- Integration tests against real Postgres:
