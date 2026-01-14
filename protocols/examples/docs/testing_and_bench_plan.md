--------------------------------------------------------------------------------
MATHILDE PROPRIETARY AND CONFIDENTIAL
Copyright (c) 2024 MATHILDE. All Rights Reserved.

This document contains trade secrets and confidential information owned
exclusively by MATHILDE, protected under Swiss law (URG, UWG, Art. 162 StGB).

PROHIBITED: Reproduction, copying, distribution, disclosure, or derivative
works without prior written authorization from MATHILDE.

ACCESS REQUIREMENT: Executed NDA with MATHILDE required. Unauthorized access
or possession violates Swiss law. Violations subject to civil remedies,
injunctive relief, damages, and criminal prosecution.

Legal Contact: massimo.nicora@wnlegal.ch
--------------------------------------------------------------------------------

# `math::signal` — testing and bench plan (DRAFT)

Protocol: `v2/crates/math/docs/protocols/module_creation_protocol.md`

This plan defines the minimum required testing and benchmarking artifacts for `math::signal`.

All tests and benches are part of the module’s correctness contract: math validity, failure behavior, determinism, and numerical stability.

---

## 1) Test location and naming

- All tests must live under `v2/crates/math/src/signal/tests/`.
- Every test file name must be `test_*.rs`.
- Tests must not rely on randomness; if pseudo-data is needed, it must be deterministic (fixed seeds or explicit sequences).

---

## 2) Test categories (mandatory for every new algorithm)

For each new public algorithm added to `math::signal`:

### 2.1 Mathematical correctness

Minimum:
- closed-form / exact cases (constants, linear, sinusoid where applicable),
- invariants (symmetry, non-negativity, normalization bounds, monotonicity if mathematically guaranteed),
- cross-check vs a reference implementation (either a slow internal reference or an identity/property-based proof path).

### 2.2 Edge cases / failure contract

Minimum:
- empty / too-short inputs (`n` constraints),
- parameter validation (bounds, shape mismatches),
- non-finite input rejection (NaN/Inf),
- documented error variants must be exercised (no dead error paths).

### 2.3 Numerical stability

Minimum:
- “large offset + small variation” inputs (catastrophic cancellation risk),
- near-degenerate cases (repeated values, near-zero denominators),
- stress ranges (e.g. values around `1e-12`, `1e12`) as relevant.

Acceptance:
- must not produce NaNs/Infs on valid input ranges,
- must return `Err(...)` (not silent NaN) on numerical breakdown.

### 2.4 Determinism

Minimum:
- repeatability: same input → same output within a strict tolerance,
- tie-breaking: any sorting/grouping step must be deterministic and tested.

### 2.5 Panic safety (library contract)

Minimum:
- representative invalid-input calls must not panic (use `catch_unwind` in tests),
- public APIs must return `MathError` on invalid parameters rather than panicking.

---

## 3) Bench location and requirements

- Benches live under `v2/crates/math/benches/`.
- Benchmark standard sizes are mandatory: `n ∈ {100, 1000, 10000}`.
- Naming:
  - `signal_<topic>.rs` for broad benches (e.g. `signal_spectral.rs`),
  - `signal_<algo>.rs` for focused benches (e.g. `signal_dcca.rs`).

Bench content (minimum):
- include the allocating convenience API benchmark (if it exists),
- include the allocation-disciplined path benchmark (workspace reuse inside the timed loop) where the algorithm has a workspace API,
- avoid measuring setup unless explicitly stated; separate setup cost from steady-state when meaningful (e.g. “cold vs warm” for cached DPSS).

---

## 4) Bench log policy (append-only)

- All benchmark runs that change code must be appended to:
  - `v2/crates/math/src/signal/docs/signal_bench_results.md`
- The log entry must include:
  - timestamp,
  - command used (`cargo bench ...`),
  - relevant parameters (e.g. `m,tau,r` for SampEn; `nw,k` for multitaper),
  - key results for `n=100/1000/10000`,
  - raw stdout file path if captured.

---

## 5) v1.4-specific acceptance (planned)

When implementing v1.4 algorithms (SSA, Lomb–Scargle, PLV/phase coherence, wavelet coherence, shape measures):
- provide at least one internal slow reference (or invariant proof path) for equivalence tests,
- benchmark both “reference” and “optimized/workspace” paths when both exist,
- document any numeric conventions that materially affect outputs (normalizations, windowing, unwrap conventions).
