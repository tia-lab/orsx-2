# ORSX2 — Testing and Bench Plan (TEMPLATE)

Status: DRAFT  
Owner: (Validator)

## 1) Test categories (mandatory)

### 1.1 Planning determinism (no DB)

- Same schema input repeated ⇒ identical plan output (stable ordering).

### 1.2 Failure contract coverage (no DB + DB)

- Every documented rejection path has a test.

### 1.3 DB integration (requires Postgres)

- Schema create
- Each supported migration class
- Online path (if supported)
- Compression `BYTEA` read/write

### 1.4 Panic safety

- Ensure invalid inputs never panic (use `catch_unwind` where appropriate).

## 2) Bench categories (mandatory)

- Compression encode/decode throughput.
- Planning time vs schema size.
- DB-facing throughput (if feasible): bulk insert/read; online backfill.

## 2.1 Allocation discipline checks (mandatory)

- At least one benchmark must run repeated calls with a workspace to prove no per-iteration allocations.
- Where applicable, add a `#[global_allocator]` counting allocator in benches (or a feature-gated allocator counter) to detect regressions.

## 3) Append-only evidence logs (mandatory)

- `protocols/orsx2_evidence/bench_results.md`
- `protocols/orsx2_evidence/migration_trials.md`

Each entry must include:

- timestamp, machine info, command, profile, results.
