# `orsx-macros` — Inventory (DRAFT)

Protocol: `protocols/inventory_template.md`

This inventory lists what currently exists in the `orsx-macros/` proc-macro crate. It describes only implemented code (no legacy, no future plans).

---

## 0) Module documentation (artifacts)

- Inventory (this file): `docs/inventory.md`

## 1) Source Files

- `src/lib.rs`: proc-macros implementing `#[derive(OrsxMigrate)]`, `#[derive(OrsxColumnar)]`, and `#[orsx_flatten_module]` (plus attribute parsing + validation).

## 2) Public API Surface

Public macros:

- `#[derive(OrsxMigrate)]` with `#[orsx_table(...)]` and `#[orsx_column(...)]`
- `#[derive(OrsxColumnar)]` with `#[orsx_column(...)]`
- `#[derive(OrsxFlatten)]` (currently rejects derive and points users to `#[orsx_flatten_module]`)
- `#[orsx_flatten_module]` attribute macro for stable recursive flatten over an inline module

## 3) Workspace / `*_into` APIs

None (proc-macro crate; compile-time only).

## 4) Determinism Tier and Epsilon Policy

Tier: A (exact) for macro output: expansion depends only on the input token stream and parsed attributes.

## 5) Benchmarks (harness files)

None in this crate.

