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

# SIGNAL — SPEC (DRAFT)

Protocol: `v2/crates/math/docs/protocols/module_creation_protocol.md`

Spec version: 1.4-draft  
Module path: `v2/crates/math/src/signal/`  
Status:
- v1 primitives: implemented (code + tests + benches exist)
- v1.1–v1.3: implemented (code + tests + benches exist, with append-only bench logs)
- v1.4: planned (spec-only until approved)
- Documentation artifacts (Phase 7I): maintained incrementally, finalized after v1.4 scope is stable

This spec is the single source of truth for what belongs to `math::signal` and what does not.

---

## 1) Identification

Name: `math::signal`

What it computes (measurement-only): deterministic signal-processing transforms and estimators for scalar time series, intended as reusable infrastructure (2+ consumers) under strict failure contracts.

Primary consumers (planned):
- `math::hurst` (later): Hurst estimators using DFA/wavelet/spectral primitives.
- `math::econometrics`: uses signal primitives (ACF/periodogram) but owns hypothesis testing.
- indicators under `v2/crates/algos/` (trend/volatility/regime processors) via `math` crate.

Input bounds (default module targets):
- Typical `n`: `100..10_000` (bench standard sizes).
- All public APIs operate on `&[f64]` and require finite inputs unless explicitly specified otherwise.

---

## 2) Purpose and institutional value (as infrastructure)

`math::signal` exists to:
- centralize audited, deterministic signal-processing building blocks,
- enforce explicit invalid-input and non-convergence behavior (`MathResult`, no silent NaNs),
- provide allocation-disciplined APIs (`*_into` and workspaces) for repeated cron workloads.

This module does not provide trading “signals”; it provides signal-processing primitives used to measure market state.

---

## 3) Mathematical specification (notation, algorithm, complexity)

Notation:
- `x[0..n)` is a finite real-valued sequence (`f64`).
- Index normalization uses `t = i/(n-1)` where stated (requires `n>=2`).

### 3.1 v1 — detrending (implemented)

Files: `v2/crates/math/src/signal/detrending.rs`

Detrending is defined as `r_i = x_i - \hat{x}_i` where `\hat{x}` is a fitted trend.

Methods:
- mean: `\hat{x}_i = mean(x)`
- linear: `\hat{x}_i = a + b*i` (OLS, centered formulation)
- polynomial degree `d`: fit `\hat{x}(t) = Σ_{j=0..d} β_j t^j` by QR least squares on normalized `t`

Complexity:
- mean/linear: `O(n)`
- polynomial (per call, QR): `O(n*d^2)` dominated by QR factorization
- polynomial (precomputed-QR workspace): `O(n*d)` per call after `O(n*d^2)` one-time setup for fixed `(n,d)`

### 3.2 v1 — wavelet primitives (implemented)

Files: `v2/crates/math/src/signal/wavelets.rs`

Implemented families:
- Haar (decimated coefficient variance proxy)
- MODWT D4 (circular boundary) detail coefficients at a given level

Complexity: `O(n)` per level/scale, allocation-disciplined via workspace variants where applicable.

### 3.3 v1 — DFA primitives (implemented)

Files: `v2/crates/math/src/signal/dfa.rs`

Implemented primitives:
- profile integration (centered cumulative sum)
- per-segment linear detrend RMS fluctuation
- deterministic window-size generation

Complexity:
- integrate: `O(n)`
- segment fluctuation: `O(m)` for segment length `m`

---

## 4) Theoretical foundation (citations, assumptions, limitations)

v1 scope is limited to primitives with direct definitions; any higher-level estimator must provide its own math review.

Wavelet and DFA primitives are standard in time-series analysis; formal citations will be added in the module math review once v1.1 is stable.

Limitations (v1):
- no FFT-based accelerations yet in `math::signal` (planned in v1.1),
- no bootstrap/surrogate testing (explicitly out of scope; see Section 7/Non-scope).

---

## 5) Crypto adaptation (where assumptions are violated by crypto microstructure)

This module remains domain-agnostic. Crypto-specific choices (exchange microstructure, noise correction heuristics, jump filters) are not part of `math::signal` and must remain in `algos` or a dedicated future microstructure layer.

The crypto-relevant constraints enforced here are:
- determinism for cron execution,
- explicit bounds and failure contracts,
- numerical safety under heavy tails and outliers (reject non-finite; avoid undefined operations).

---

## 6) Determinism contract (explicit)

Default path:
- no RNG,
- no nondeterministic parallel reductions,
- deterministic ordering/tie-breaking when sorting is required.

Epsilon-based determinism for floating outputs (default):
- Comparison: `abs <= abs_eps OR abs <= rel_eps * max(1, |expected|)`
- `abs_eps = 1e-12`
- `rel_eps = 1e-12`

Any opt-in parallel path (v1.1 or later) must be explicitly separated from deterministic APIs and benchmarked separately.

---

## 7) API contract (inputs, outputs, parameters)

Public surface (v1, current):
- `signal::detrending`: `detrend`, `detrend_into`, `detrend_polynomial_into_with_workspace`, `detrend_polynomial_precomputed_into_with_workspace`
- `signal::wavelets`: `modwt_d4_detail_level`, `wavelet_variance` (+ workspace variants)
- `signal::dfa`: DFA primitives (integration, segment RMS, window sizes)
- `signal::types`: `DetrendMethod`, `WaveletFamily`

Rules:
- All `Ok(...)` outputs must be finite.
- Workspace APIs are the preferred production path for repeated calls.
- Allocation-producing convenience APIs may exist, but must have a corresponding `*_into`/workspace API when performance-relevant.

---

## 8) Failure contract (invalid inputs, non-convergence, fallback)

Mandatory (all public APIs):
- reject empty inputs where the math requires `n>=1`,
- reject non-finite inputs (NaN/Inf),
- reject invalid parameters (scale/degree bounds, shape mismatches),
- return `Err(...)` on numerical breakdown (non-finite intermediates, rank deficiency) instead of silent sentinels.

No fallback behavior is allowed unless explicitly specified per function and covered by tests.

---

## 9) Testing plan (phases, counts, acceptance)

All tests live under `v2/crates/math/src/signal/tests/` and are named `test_*.rs`.

v1 minimum coverage:
- determinism: same inputs => same outputs within epsilon
- mathematical correctness: constant/linear/quadratic exact cases
- numerical stability: large offsets + small variation (where relevant), conditioning checks for polynomial detrend
- failure contract: invalid scale/degree/shape, non-finite inputs
- panic-safety: representative error paths wrapped with `catch_unwind`

v1.1 planned additions must add their own test expansions (spectral and multifractal core invariants).

---

## 10) Performance budget and benchmark plan

Benchmark sizes (standard, enforced):
- `n ∈ {100, 1000, 10000}`

Bench requirements:
- include at least one allocation-discipline benchmark (workspace reuse in the timed loop),
- record results append-only in `v2/crates/math/src/signal/docs/signal_bench_results.md`.

---

## 11) Reuse report (Phase 2 output)

Legacy modules scanned:
- `v2/crates/algos/src/shared/hurst/`
- `v2/crates/algos/src/shared/multifractal/`
- `v2/crates/algos/src/shared/spectral/`
- `v2/crates/algos/src/shared/clustering/`

Decisions:
- `hurst`: only generic DFA/wavelet primitives belong here; Hurst estimators remain in a dedicated module (`math::hurst`) later.
- `multifractal`: MF-DFA and WTMM *core computation* belongs here only if we keep multifractal; all significance tests and RNG surrogates do not.
- `spectral`: signal transforms belong here (periodogram/FFT-ACF/coherence/time-varying spectrum) but hypothesis tests do not.
- `clustering`: not signal; belongs to a separate ML/optimization module (or stays in `algos`).

---

## 12) References (DOI preferred)

Pending: to be finalized in the `signal` math review once v1.4 scope is implemented and stable.

---

## v1.1 roadmap (planned rewrites to `math::signal`)

These are in-scope candidates for `math::signal` only if rewritten to the protocol (deterministic default, strict failure contracts, workspace APIs, benchmarks + logged history):

### v1.1 — spectral (signal transforms only)

From `v2/crates/algos/src/shared/spectral/`:
- periodogram and detrended periodogram
- FFT-based autocorrelation (accelerated ACF)
- coherence and time-varying spectrum (transform side only)

Explicit non-scope (must NOT be moved into `math::signal`):
- `hypothesis.rs` (Ljung–Box, Fisher-g, Bartlett bounds, F-tests) → `math::econometrics`
- `microstructure.rs` (exchange heuristics) → keep in `algos` / microstructure layer

### v1.1 — multifractal (core computation only)

From `v2/crates/algos/src/shared/multifractal/`:
- MF-DFA computation core (generalized fluctuation functions, H(q))
- WTMM computation core (CWT pipeline + modulus maxima extraction)

Explicit non-scope:
- `phase_randomization.rs` (RNG surrogates) → `math::stochastic` (explicit seed; non-default path)
- `statistical_tests.rs` (significance/hypothesis) → `math::econometrics`
- `crypto_adaptations.rs` (domain heuristics) → keep in `algos`

---

## v1.2 roadmap (planned additions to `math::signal`)

v1.2 is focused on **high-value, reusable signal-processing primitives** that expand consumer capability while keeping:
- determinism (default path),
- explicit bounds and failure contracts,
- workspace/`*_into` APIs for repeated calls,
- benchmark coverage and append-only logs.

### Spectral / frequency

- Welch PSD (segmented/averaged periodogram): — DONE
  - deterministic segmentation policy,
  - explicit window function choice,
  - time-bounded by max segments.
- Multitaper PSD (DPSS): — DONE
  - deterministic DPSS computation or deterministic cached tapers by `(n, nw, k)`,
  - hard caps on `(n, k)` and any eigensolve iterations.
- Goertzel / targeted DFT bins: — DONE
  - compute power at a fixed set of frequencies without full FFT,
  - deterministic and allocation-free for repeated bins.
- Cross-spectrum + phase / group delay: — DONE
  - complements coherence with phase lead/lag measurement,
  - transform-level only (no inference/hypothesis).

### Filtering / smoothing

- Savitzky–Golay filter (poly smoothing + derivatives): — DONE
  - deterministic coefficients for `(window_len, poly_degree, deriv_order)`,
  - workspace to apply across sliding windows.
- Kalman filter primitives (1D + local level/trend models): — DONE
  - deterministic state recursion, innovation variance,
  - strict handling of non-finite observations and covariance underflow (explicit counters or errors),
  - no domain-specific defaults embedded in the core API.
- One-pass IIR low/high-pass (biquad) with fixed coefficients: — DONE
  - deterministic filtering in one pass,
  - explicit coefficient parameterization and stability checks.

### Time–frequency / local features

- Window functions + windowed STFT (Hann/Hamming/Blackman): — DONE
  - explicit window definitions and normalization,
  - deterministic application and bounded window counts.
- Hilbert transform (FFT-based analytic signal): — DONE
  - deterministic analytic signal construction,
  - amplitude envelope and instantaneous phase primitives.

### Fractal / multiscale

- Wavelet denoising / thresholding primitives (soft/hard thresholds): — DONE
  - deterministic threshold selection only if fully specified; otherwise threshold value must be explicit input,
  - explicit handling of boundary conditions.
- Generalized MODWT families (beyond D4): — DONE
  - additional wavelet families with documented filter taps,
  - workspace APIs for repeated per-level computations.
  - implemented families: D4, D6, D8

---

## v1.3 roadmap (deferred)

Complexity / nonlinear signal measures are deferred to v1.3 because they require careful bounds and stronger correctness evidence:

- Permutation entropy / sample entropy (bounded): implemented in `signal::entropy` — DONE
- Recurrence quantification analysis (RQA) primitives: implemented in `signal::rqa` — DONE

Additional high-value candidates (must remain deterministic and explicitly bounded):
- Instantaneous frequency stability primitives derived from Hilbert phase (unwrap + derivative conventions): implemented in `signal::spectral::hilbert` — DONE
- Detrended cross-correlation analysis (DCCA) primitives (scale-based dependence without hypothesis tests): implemented in `signal::dcca` — DONE
- Multiscale entropy (MSE) as a bounded wrapper over sample entropy (explicit scale caps): implemented in `signal::mse` — DONE
- Zero-crossing rate and run-length statistics for sign changes (fast structure/chop measures): implemented in `signal::zero_crossing` — DONE

---

## v1.4 roadmap (planned additions)

v1.4 adds a small set of high-value measurement primitives that still fit the `math::signal` scope:
- deterministic defaults,
- strict failure contracts (`MathResult`, no silent NaNs),
- allocation discipline (workspace APIs where relevant),
- benchmark coverage and append-only bench logs.

### Decomposition / structure

- SSA (Singular Spectrum Analysis) primitives: — DONE
  - bounded-rank decomposition for trend/seasonality/noise measurement,
  - deterministic numerical strategy (explicit algorithm choice; no hidden randomness),
  - API must separate “core numeric primitive” from any model-selection heuristics.

### Spectral for irregular sampling

- Lomb–Scargle periodogram: — DONE
  - supports irregular time stamps or missing observations,
  - explicit normalization choice and numeric safeguards.

### Phase-derived dependence

- PLV / phase coherence primitives based on Hilbert phase: — DONE
  - define phase extraction convention, unwrap policy, and handling of near-zero amplitude.

### Multiscale dependence (wavelet-domain)

- Wavelet cross-spectrum / wavelet coherence primitives: — DONE
  - multiscale dependency measurement,
  - no significance tests / no hypothesis/p-values (belongs to `math::econometrics`).

### Small “signal shape” measures (fast)

Candidates (planned; only if they remain strictly specified and deterministic): — DONE
- Hjorth parameters (activity/mobility/complexity): — DONE
- Teager–Kaiser energy operator: — DONE
- spectral flatness / crest factor / spectral entropy reducers (must specify PSD estimate choice): — DONE
