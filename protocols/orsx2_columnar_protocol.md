# ORSX2 — Columnar Retrieval Protocol (SPEC → IMPLEMENT → EVIDENCE → READY)

Version: 0.1-draft  
Scope: this repository only  
Applies to: `orsx2::columnar` (wide-table, mixed-type, NULL-safe columnar retrieval + binary transport)

## Purpose

Deliver a **performance-first**, **panic-free**, **deterministic-by-default** columnar retrieval system for Postgres that:

- can scan 10k..100k rows efficiently (and scale to 1M via batching/streaming),
- supports mixed scalar + varlen types and NULLs,
- can be serialized to a stable, versioned **binary envelope**,
- remains a thin layer over Postgres (no ORM; raw SQL stays first-class).

## Inderogable rules (restart if violated)

1. No silent corruption: decoded values must match Postgres semantics (or the operation must fail).
2. No panics in library code.
3. Deterministic output ordering: column order == query order, row order == query order.
4. Allocation discipline: hot paths must have workspace-based APIs and avoid per-value allocations.
5. Evidence required: performance claims must be recorded in append-only evidence logs.
6. No git operations: do not run `git` commands or modify git state unless explicitly requested.

---

# Phase 0 — Intake (NO CODE)

Stop if any item is missing:

- Postgres version(s) targeted.
- The expected query shapes:
  - row counts (typical + max),
  - column counts (typical + max),
  - dominant types (and NULL rates).
- Transport requirement:
  - is binary envelope required immediately, or can we start with in-process columnar only?
- Acceptance budgets:
  - target latency for 100k rows × 500 columns,
  - memory cap per batch (e.g., 256MB),
  - whether parallel decode is allowed (opt-in only).

Deliverable:
- a filled spec at `protocols/orsx2_specs/COLUMNAR_RETRIEVAL_SPEC.md`

---

# Phase 1 — Specification (NO CODE)

Fill (and freeze) `protocols/orsx2_specs/COLUMNAR_RETRIEVAL_SPEC.md`:

- supported type mapping,
- NULL model,
- binary envelope definition and validation rules,
- failure contract,
- determinism contract,
- workspace/zero-copy API shapes,
- performance budgets and evidence gates.

Design decision checkpoint:

- If Arrow IPC is chosen: justify the dependency footprint and confirm it meets perf/alloc constraints.
- If ORSXCOL envelope is chosen: ensure versioning + validation rules are explicit.

Exit condition:
- spec approved (no ambiguous “TBD” in contracts/budgets).

---

# Phase 2 — Implementation (CODE)

Implementation order (recommended):

1. Define in-memory column buffers + validity bitmaps + workspace.
2. Implement ORSXCOL v1 encoder/decoder + unit tests (no DB).
3. Implement Postgres **fast path**: `COPY (SELECT ...) TO STDOUT (FORMAT BINARY)` parse into columns.
4. Add row-wise fallback reader (optional) for debugging and equivalence testing.
5. Add integration tests against Postgres proving equality between fast path and row-wise `SELECT`.

Rules:

- No hidden allocations in per-row/per-value loops in steady state (workspace required).
- All size math uses `checked_*`.
- Any “raw SQL” APIs that accept strings must be explicitly labeled unsafe-by-contract unless inputs are fully validated/quoted.

---

# Phase 3 — Correctness validation (TEST)

Required test gates:

- Unit:
  - envelope round-trip for each supported type,
  - NULL semantics,
  - invalid envelope rejection (lengths/offsets/overflows).
- Integration (DB):
  - mixed fixed + varlen + NULLs,
  - empty result and boundary sizes,
  - ordering stability.

Exit condition:
- tests cover the full failure contract.

---

# Phase 4 — Performance validation (EVIDENCE, release)

Run perf trials and append results:

- Evidence log: `protocols/orsx2_evidence/columnar_trials.md` (append-only)
- Required scenarios:
  - 100k rows × 50 cols (mixed)
  - 100k rows × 500 cols (mixed)
  - 1M rows × 50 cols (mixed, streaming/batching)
- Compare:
  - fast path (COPY BINARY) vs row-wise decode
  - workspace reuse vs fresh allocations

Exit condition:
- budgets met (or explicit rejection with evidence).

---

# READY gate

The feature is READY only if:

- Spec is complete and accurate.
- All tests pass and cover the failure contract.
- Evidence log includes release-mode perf trials on the target hardware class.
- Public APIs are minimal and stable (no experimental surfaces without a feature flag).

