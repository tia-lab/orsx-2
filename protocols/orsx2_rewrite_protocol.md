# ORSX2 — Rewrite Protocol (SPEC → IMPLEMENT → TEST → READY)

Version: 0.1-draft  
Scope: this repository only  
Applies to: the new `orsx2` workspace to be created at repo root (separate from the current implementation)

## Purpose

Create a **high-performance**, **high-reliability**, Postgres-specific Rust library (thin over `sqlx`) that provides:

1. **Schema-driven “zero-loss” migrations** designed for **very large tables**.
2. **Versioned, auditable compression** for large numeric vectors stored in `BYTEA`.
3. A minimal API that keeps queries as **raw SQL** (prefer `sqlx::query!`/`query_as!` when table name is static).

This protocol exists to prevent:

- silent data loss,
- silent corruption (especially around compressed payloads),
- unbounded migrations that lock production tables unexpectedly,
- “works on small tables” designs that fail at scale.

## Inderogable rules (restart if violated)

1. **No silent data loss**: every migration must preserve data or fail loudly before committing.
2. **No silent corruption**: bytes written must be parseable by the declared format/version; migrations must not generate undecodable payloads.
3. **Panic-free library**: no `unwrap/expect/panic/todo/unreachable` in non-test code.
4. **Deterministic planning**: given the same inputs and DB state, migration planning output must be identical (timestamps/logging excluded).
5. **Lock discipline**: any operation that may block writes must be explicitly budgeted and tested.
6. **Clarity over cleverness**: if correctness cannot be proved or validated, the feature is rejected.
7. **Raw SQL remains first-class**: do not build an ORM or query builder; helpers may exist only where required for correctness/safety.

## Definitions

- **Zero-loss migration**: after migration, all original rows remain logically present in the active table (or in an explicitly retained backup table), with an auditable mapping from old schema to new schema.
- **Online migration**: migration strategy designed to avoid long `ACCESS EXCLUSIVE` locks for large tables (may require multi-step backfill / dual-write).
- **Cutover**: the final step where the new schema becomes authoritative (rename/swap/trigger removal), typically requiring a short lock.
- **Failure contract**: a precise list of invalid inputs/unsupported changes, and how the system fails (error variant + rollback behavior).
- **Compression envelope**: a versioned binary header + codec id + checksum + payload, stored in `BYTEA`.

## Roles (single-agent execution allowed)

- **Designer**: writes specs, constraints, and budgets (“what + why + bounds”).
- **Implementer**: writes Rust/SQL exactly matching specs (“how”).
- **Validator**: runs tests/benches, verifies evidence, rejects ambiguous behavior.

One person may perform all roles, but the deliverables must still exist.

---

# PART A — RESEARCH + INTAKE (NO CODE)

## Phase 0: Rewrite intake (mandatory inputs)

The requester must provide:

- Target Postgres versions (minimum supported).
- “Very large table” definition in this environment:
  - typical row counts, largest row counts,
  - typical row width,
  - primary key type and expected indexes,
  - acceptable cutover lock budget (e.g. 1s / 5s / 30s).
- Expected write pattern during migrations (read-only window allowed or not).
- Compression use-cases:
  - vector element type(s), typical lengths, max lengths,
  - whether existing stored bytes must be readable (for this rewrite: **not required**, new versioned envelope recommended).

**Stop if any item is missing.**

## Phase 1: Reuse search (mandatory)

Before writing new code, search the current repo for reusable parts (concepts, tests, SQL snippets), but do not copy without justification.

Minimum:

- Search migration planning, introspection, index management, compression, and trait/macro patterns.
- Produce a **reuse report** (template below).

---

# PART B — SPECIFICATION (NO CODE)

## Required specs (each mandatory)

Create the following spec documents before implementation:

1. `protocols/orsx2_specs/MIGRATIONS_SPEC.md`
2. `protocols/orsx2_specs/SCHEMA_INTROSPECTION_SPEC.md`
3. `protocols/orsx2_specs/COMPRESSION_SPEC.md`
4. `protocols/orsx2_specs/MACROS_SPEC.md` (or an explicit “no macros” decision + alternative schema description)
5. `protocols/orsx2_specs/TESTING_AND_BENCH_PLAN.md`
6. `protocols/orsx2_specs/COLUMNAR_RETRIEVAL_SPEC.md` (required only if `orsx2::columnar` is in scope)

Each spec must include:

- **Identification** (name, scope, non-scope)
- **API contract** (public surfaces only)
- **Failure contract** (unsupported operations must be listed)
- **Determinism contract**
- **Performance budget** (explicit bounds + ms budgets)
- **Security contract** (SQL injection, identifier quoting, unsafe SQL features)
- **Evidence plan** (which tests/benches validate which claims)

### Migration spec (extra mandatory sections)

The migrations spec must define:

- Supported change set (add column, drop column, type change, nullability, index changes, PK changes).
- Strategy selection rules:
  - small table vs large table path,
  - online vs offline path,
  - when cutover is permitted.
- Lock budget policy:
  - expected lock level per step,
  - maximum time each step is allowed to hold blocking locks.
- “No undecodable bytes” rule for compressed columns:
  - migrations must never create `BYTEA` that the new decoder rejects.

### Compression spec (extra mandatory sections)

The compression spec must define:

- A **versioned envelope**:
  - magic bytes,
  - format version,
  - codec id,
  - element type id,
  - uncompressed length,
  - checksum (mandatory),
  - payload.
- Forward/backward compatibility policy.
- Error behavior on invalid payloads (must be deterministic; must not panic).

---

# PART C — IMPLEMENTATION (CODE)

## Phase 2: Repository restructuring (planned, do not execute without explicit request)

Target structure:

- `orsx2/` (new library crate)
- `orsx2-macros/` (optional, if derive macro exists)
- `deprecated/orsx_v0/` and `deprecated/orsx-macros_v0/` (current implementation moved here)

Rule: do not break existing `orsx` consumers until `orsx2` passes READY gate.

## Phase 3: Implementation rules

- No panics in non-test code.
- No implicit defaults that change database behavior (extensions, triggers, etc.) without explicit config.
- All SQL that includes identifiers must use a single, audited quoting function.
- Migration execution must be transactional where feasible; where infeasible (online multi-step), the protocol must define resumability and idempotence.

## Phase 3.1: Mandatory code patterns (performance + reliability)

These are enforced patterns (modeled after the reference examples in `protocols/examples/`).

### 3.1.1 Zero-copy / minimal-allocation APIs

For any hot-path component (compression, plan building, backfill batching):

- Provide an allocating convenience API: `fn foo(...) -> Result<T>`.
- Provide a non-allocating API: `fn foo_into(..., out: &mut ...) -> Result<()>`.
- If repeated calls are expected, provide a workspace API:
  - `fn foo_into_with_workspace(..., out: &mut ..., ws: &mut FooWorkspace) -> Result<()>`.

### 3.1.2 Workspace types are mandatory for hot paths

Workspace types must:

- have `with_capacity(...)` constructors,
- have `prepare(...)` methods that `clear()` + `resize()` buffers deterministically,
- avoid heap allocations inside per-row/per-item loops in the steady-state path.

### 3.1.3 Explicit validation and bounds

All public functions must:

- validate sizes, shapes, and invariants up-front (lengths, required minimums),
- validate finiteness where relevant (for any float-like computations),
- use checked arithmetic for size computations (`checked_add`, `checked_mul`) and return deterministic errors on overflow,
- define explicit caps where input-driven loops could become unbounded.

### 3.1.4 Deterministic ordering and tie-breaks

Any sorting/ordering used for planning, diffing, or deterministic output must:

- use an explicit stable ordering key,
- define a deterministic tie-break (e.g. `(key, index)`),
- never rely on hash map iteration order.

### 3.1.5 No hidden allocations in tight loops

In non-test code:

- No `collect()`/`to_vec()`/`clone()` in per-row loops unless justified and benchmarked.
- Pre-allocate with `Vec::with_capacity` and reuse buffers.

### 3.1.6 Parallelism policy

- Default path is deterministic and single-threaded unless proven otherwise.
- Any parallel path must be opt-in and must state its determinism contract and performance benefit.

---

# PART D — TESTING + BENCHMARKS (MANDATORY)

## Phase 4: Testing requirements

### 4.1 Non-DB tests (mandatory)

- Planning determinism: same input → identical plan.
- Schema diff correctness: known schemas → expected differences.
- Envelope round-trip: compress → decompress equals original.
- Invalid envelope: decoder returns an error (no panic).

### 4.2 DB integration tests (mandatory)

Must run against real Postgres:

- Migration correctness for supported operations.
- Online migration path (if implemented): backfill correctness + short cutover.
- Index creation behavior and safety.
- Large-payload compression read/write.

### 4.3 Failure contract coverage (mandatory)

For every documented unsupported change, add a test that proves the system rejects it deterministically.

## Phase 5: Benchmark requirements

Benchmarks must be reproducible and report:

- command, profile, machine info,
- dataset sizes and shapes,
- medians and dispersion.

At minimum:

- Compression encode/decode throughput for representative vector sizes.
- Migration planning time vs schema size.
- DB-facing benchmarks (if feasible): bulk insert/read throughput, and online migration backfill throughput.

## Phase 6: Append-only evidence logs

Maintain append-only logs:

- `protocols/orsx2_evidence/bench_results.md`
- `protocols/orsx2_evidence/migration_trials.md`

Each entry must include command lines and results.

---

# PART E — READY GATE (BLOCKING)

`orsx2` is READY only if:

- All required specs exist and are complete.
- All tests pass (non-DB + DB).
- Benchmarks meet the declared budgets for declared bounds (or the budgets are revised with explicit justification).
- Failure contracts are fully covered by tests.
- No panics in non-test code (enforced by CI/lints or explicit checks).

---

# Templates

See:

- `protocols/orsx2_specs/` for spec templates.
- `protocols/orsx2_evidence/` for append-only log templates.
