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

# MATHEMATICAL REVIEW: `math::signal` (Deterministic Signal Processing Primitives)

**Document Type**: Mathematical Review (Module)  
**Status**: UNIT_TEST_VALIDATED (via module test suite)  
**Date**: 2026-01-12  
**Module**: `math::signal`  
**Implementation**:
- `v2/crates/math/src/signal/mod.rs`
- `v2/crates/math/src/signal/types.rs`
- `v2/crates/math/src/signal/detrending.rs`
- `v2/crates/math/src/signal/dfa.rs`
- `v2/crates/math/src/signal/wavelets.rs`
- `v2/crates/math/src/signal/filtering/mod.rs`
- `v2/crates/math/src/signal/filtering/savgol.rs`
- `v2/crates/math/src/signal/filtering/kalman.rs`
- `v2/crates/math/src/signal/filtering/biquad.rs`
- `v2/crates/math/src/signal/spectral/mod.rs`
- `v2/crates/math/src/signal/spectral/fft.rs`
- `v2/crates/math/src/signal/spectral/windows.rs`
- `v2/crates/math/src/signal/spectral/periodogram.rs`
- `v2/crates/math/src/signal/spectral/welch.rs`
- `v2/crates/math/src/signal/spectral/goertzel.rs`
- `v2/crates/math/src/signal/spectral/autocorrelation_fft.rs`
- `v2/crates/math/src/signal/spectral/coherence.rs`
- `v2/crates/math/src/signal/spectral/cross_spectrum.rs`
- `v2/crates/math/src/signal/spectral/time_varying.rs`
- `v2/crates/math/src/signal/spectral/hilbert.rs`
- `v2/crates/math/src/signal/spectral/dpss.rs`
- `v2/crates/math/src/signal/spectral/precomputed_dpss.rs`
- `v2/crates/math/src/signal/spectral/multitaper.rs`
- `v2/crates/math/src/signal/spectral/lomb_scargle.rs`
- `v2/crates/math/src/signal/spectral/phase_coherence.rs`
- `v2/crates/math/src/signal/multifractal/mod.rs`
- `v2/crates/math/src/signal/multifractal/mfdfa.rs`
- `v2/crates/math/src/signal/multifractal/wtmm.rs`
- `v2/crates/math/src/signal/entropy.rs`
- `v2/crates/math/src/signal/mse.rs`
- `v2/crates/math/src/signal/dcca.rs`
- `v2/crates/math/src/signal/rqa.rs`
- `v2/crates/math/src/signal/zero_crossing.rs`
- `v2/crates/math/src/signal/ssa.rs`
- `v2/crates/math/src/signal/wavelet_coherence.rs`
- `v2/crates/math/src/signal/shape.rs`
**Specification**: `v2/crates/math/src/signal/docs/SIGNAL_SPEC.md`  
**Scope Definition**: `v2/crates/math/src/signal/docs/scope.md`  
**Inventory**: `v2/crates/math/src/signal/docs/inventory.md`  
**Validation Source of Truth**: `v2/crates/math/src/signal/tests/*`  
**Benchmark Harness**: `v2/crates/math/benches/signal_*.rs` (signal-related benches)  
**Benchmark Results Log**: `v2/crates/math/src/signal/docs/signal_bench_results.md`  
**Protocol**: `v2/crates/math/docs/protocols/module_creation_protocol.md`  

This is a mathematical review of a library module, not a regime indicator.  
It is constrained to what is implemented and what is supported by the unit tests in `v2/crates/math/src/signal/tests/`.  
This document does not claim economic optimality, forecasting performance, or suitability of any assumptions for crypto data.  

---

## 1. OVERVIEW

### 1.1 Module purpose

**Purpose**: Provide deterministic, numerically safe, time-bounded signal/time-series processing primitives as reusable infrastructure.

This module is measurement-only infrastructure:
- it computes well-defined transforms/estimators under explicit contracts,
- it rejects invalid inputs explicitly (no silent NaN/Inf propagation by design),
- it provides workspace/`*_into` APIs to avoid repeated allocations where relevant.

### 1.2 Scope (implemented)

Implemented scope follows `v2/crates/math/src/signal/docs/scope.md` and includes:
- detrending primitives (mean/linear/polynomial),
- DFA primitives,
- MODWT wavelet transforms and wavelet variance, plus deterministic denoising,
- deterministic spectral transforms (periodogram, Welch, multitaper/DPSS, Goertzel, FFT-ACF, coherence, cross-spectrum, STFT),
- Hilbert transform / analytic signal / phase primitives (unwrap + instantaneous frequency),
- irregular-sampling spectral transform (Lomb–Scargle),
- phase-derived dependence (PLV),
- multifractal core computations (MF-DFA, WTMM),
- bounded complexity measures (permutation entropy, sample entropy, MSE, RQA),
- DCCA primitives,
- fast structure measures (zero-crossing rate and sign run statistics),
- SSA rank-`r` reconstruction (bounded-rank decomposition primitive),
- MODWT-based wavelet coherence primitive,
- fast scalar shape measures (Hjorth, Teager–Kaiser energy, spectral flatness/crest/entropy reducers).

### 1.3 Non-scope (explicit)

This module does not implement:
- hypothesis tests / p-values / inference (belongs to `math::econometrics`),
- RNG-driven resampling or surrogate testing by default path (belongs to `math::stochastic` and must be explicit/seeded),
- crypto microstructure heuristics (belongs to `algos` / microstructure layer),
- online change-point detection (CUSUM/Page–Hinkley/BOCPD) (explicitly out of scope in `scope.md`).

---

## 2. INPUT/OUTPUT CONTRACTS (SAFETY + DETERMINISM)

### 2.1 Input validation (general)

Default rule across this module:
- APIs reject NaN/Inf inputs unless explicitly documented otherwise.
- length constraints are explicit (`InsufficientDataAlgo`, `InvalidParameter`, `InvalidData`).
- parameter constraints are explicit (window sizes, degrees, shapes, frequency bounds).

Examples:
- `signal::spectral::lomb_scargle` requires strictly increasing time stamps and strictly positive finite frequencies.
- MODWT functions require valid levels and (where applicable) power-of-two scale constraints.
- Kalman filters require finite observation sequences and finite noise parameters.

### 2.2 Output contracts (general)

For APIs returning `MathResult<f64>`:
- `Ok(out)` implies `out.is_finite()` and `out` is within any explicitly stated bounds (e.g. coherence measures are clamped/validated into `[0,1]`).

For APIs returning vectors:
- outputs are deterministic for a given input/configuration,
- outputs are fully written before returning `Ok`,
- on numerical breakdown, return `Err(MathError::...)` rather than silent NaNs.

### 2.3 Determinism (what is guaranteed)

The module is deterministic in the sense that:
- no RNG is used in default APIs,
- loop order is deterministic,
- any sorting/tie-breaking is deterministic by construction,
- some internal parallelism is permitted only when the reduction is provably associative over exact integers (example: SampEn counting uses integer counts; accumulation is deterministic).

Where floating reductions are involved, the module uses deterministic ordering by default.

---

## 3. ALGORITHMS AND FORMULAS

This section describes what is implemented, with formula-to-code binding by file/function identity.

### 3.1 Detrending (`signal::detrending`)

Files: `v2/crates/math/src/signal/detrending.rs`

#### 3.1.1 Mean detrend

Given `x[0..n)`, compute `mean = (1/n) Σ x_i` and return:
- `r_i = x_i - mean`

#### 3.1.2 Linear detrend (OLS)

Fit `x_i ≈ a + b i` by least squares and return residuals `r_i = x_i - (a + b i)`.

Implementation uses a numerically stable centered formulation and QR-based least squares for polynomial/general cases.

#### 3.1.3 Polynomial detrend (degree `d`)

Fit:

`x_i ≈ Σ_{j=0..d} β_j t_i^j`, with `t_i = i/(n-1)` (requires `n>=2`).

Compute coefficients via QR least squares, then residuals:
- `r_i = x_i - ŷ_i`

Workspace variants:
- `PolynomialDetrendWorkspace` caches buffers for repeated calls.
- `PolynomialDetrendPrecomputedWorkspace` caches QR decomposition for fixed `(n,d)` and reuses it across calls.

### 3.2 DFA primitives (`signal::dfa`)

File: `v2/crates/math/src/signal/dfa.rs`

#### 3.2.1 Profile integration

Mean-center then integrate:
- `y_k = Σ_{i=0..k} (x_i - mean(x))`

#### 3.2.2 Segment RMS fluctuation (linear detrend)

For a segment `y[0..m)`, fit a line `a + b i` and compute:
- `F = sqrt( (1/m) Σ (y_i - (a + b i))^2 )`

#### 3.2.3 Window size generation

Deterministic geometric sequence under explicit bounds (`min_size`, `max_size_factor`) to define scales for DFA/MF-DFA consumers.

### 3.3 Wavelets (`signal::wavelets`)

File: `v2/crates/math/src/signal/wavelets.rs`

#### 3.3.1 MODWT detail coefficients (Daubechies families)

For a wavelet family with filter taps `h[k]`, `g[k]` (scaled by `1/sqrt(2)` per MODWT convention), compute detail coefficients at level `j` with dilation `2^{j-1}` under circular boundary.

The module supports families:
- `ModwtD4`, `ModwtD6`, `ModwtD8` (explicit tap tables in code),
- `Haar` is treated separately where applicable.

#### 3.3.2 Wavelet variance

At a given scale/level, wavelet variance is computed as the biased variance of the detail coefficients:
- `WV = mean( w_j(t)^2 )` (after any explicit conventions in the file).

#### 3.3.3 Wavelet denoising (MODWT)

Pipeline:
- compute detail coefficients across levels,
- apply thresholding (hard/soft) with explicit threshold input,
- reconstruct via inverse MODWT (bounded, deterministic).

Thresholding:
- hard: `w <- sign(w) * max(|w| - 0, 0)` with explicit rule
- soft: `w <- sign(w) * max(|w| - threshold, 0)`

Universal threshold helper:
- `threshold = sigma * sqrt(2 ln(n))` (with explicit validation and deterministic behavior).

### 3.4 Filtering (`signal::filtering`)

Folder: `v2/crates/math/src/signal/filtering/`

#### 3.4.1 Savitzky–Golay filter (`filtering::savgol`)

File: `v2/crates/math/src/signal/filtering/savgol.rs`

Implements polynomial least-squares smoothing/derivative estimation on a sliding window:
- window length `W` (odd, explicit constraints),
- polynomial order `p`,
- derivative order `d`,
- output computed by precomputed convolution coefficients (cached in `SavGolWorkspace`).

Edge handling is explicit (`EdgeMode`, currently `Nearest`).

#### 3.4.2 Kalman filters (`filtering::kalman`)

File: `v2/crates/math/src/signal/filtering/kalman.rs`

Implements deterministic linear Gaussian Kalman recursions for:
- local level model (1D state),
- local linear trend model (2D state).

State update is the standard prediction/update recursion:
- `x_{t|t-1} = A x_{t-1|t-1}`
- `P_{t|t-1} = A P_{t-1|t-1} A^T + Q`
- `K_t = P_{t|t-1} H^T (H P_{t|t-1} H^T + R)^{-1}`
- `x_{t|t} = x_{t|t-1} + K_t (y_t - H x_{t|t-1})`
- `P_{t|t} = (I - K_t H) P_{t|t-1}`

All parameters are explicit; invalid or numerically unstable configurations are rejected via `MathError`.

#### 3.4.3 Biquad filtering (`filtering::biquad`)

File: `v2/crates/math/src/signal/filtering/biquad.rs`

Implements deterministic IIR filtering using DF2T form with explicit coefficients and stability checks.
Provides coefficient factories for low/high-pass designs (Butterworth and general Q).

### 3.5 Spectral transforms (`signal::spectral`)

Folder: `v2/crates/math/src/signal/spectral/`

This section is restricted to transform-level definitions; inference is out of scope.

#### 3.5.1 FFT primitive

File: `v2/crates/math/src/signal/spectral/fft.rs`

Provides `FftWorkspace` wrapping `rustfft` plans for deterministic repeated use.

#### 3.5.2 Window functions

File: `v2/crates/math/src/signal/spectral/windows.rs`

Implements deterministic window coefficient generation (`Rectangular`, `Hann`, `Hamming`, `Blackman`) and application.

#### 3.5.3 Periodogram

File: `v2/crates/math/src/signal/spectral/periodogram.rs`

Computes:
- optional detrend,
- FFT magnitude-squared (one-sided conventions as implemented),
- returns deterministic per-bin power.

#### 3.5.4 Welch PSD

File: `v2/crates/math/src/signal/spectral/welch.rs`

Computes averaged periodogram across overlapping windows:
- segment length, step, window choice, detrend policy are explicit,
- uses a workspace for repeated calls.

#### 3.5.5 Goertzel power at selected bins

File: `v2/crates/math/src/signal/spectral/goertzel.rs`

Computes power at selected DFT bins without full FFT, via deterministic Goertzel recurrence:
- for a bin frequency `ω`, the recurrence accumulates a second-order filter and yields power.

#### 3.5.6 FFT-based autocorrelation

File: `v2/crates/math/src/signal/spectral/autocorrelation_fft.rs`

Computes autocorrelation using:
- FFT of signal,
- power spectrum magnitude,
- inverse FFT to correlation sequence,
with explicit normalization conventions and bounded output lags.

#### 3.5.7 Coherence (magnitude-squared)

File: `v2/crates/math/src/signal/spectral/coherence.rs`

Definition per frequency bin `k`:
- `C_xy[k] = |S_xy[k]|^2 / (S_xx[k] S_yy[k])`
where `S` terms are the (single-segment) cross/auto spectra from FFTs, under explicit detrend policy.

Outputs are validated/clamped into `[0,1]` by explicit checks.

#### 3.5.8 Cross-spectrum, phase, group delay

File: `v2/crates/math/src/signal/spectral/cross_spectrum.rs`

Computes:
- cross spectrum `S_yx[k]`,
- phase `φ[k] = arg(S_yx[k])`,
- group delay from discrete phase slope under explicit sample-rate input `fs`.

#### 3.5.9 STFT / time-varying periodograms

File: `v2/crates/math/src/signal/spectral/time_varying.rs`

Computes windowed short-time periodograms with explicit:
- window length, step, detrend policy,
- optional window functions,
- workspace variants and flat-output variants for allocation discipline.

#### 3.5.10 Hilbert transform / analytic signal / phase derivatives

File: `v2/crates/math/src/signal/spectral/hilbert.rs`

Analytic signal construction:
- FFT of real signal,
- frequency-domain multiplier:
  - DC unchanged,
  - positive frequencies doubled,
  - negative frequencies zeroed,
  - Nyquist handled explicitly for even `n`,
- inverse FFT with `1/n` normalization.

Amplitude and phase:
- `amp = sqrt(re^2 + im^2)`
- `phase = atan2(im, re)` in `[-pi, pi]`

Phase unwrap convention:
- if `delta > pi`, subtract `2*pi`,
- if `delta < -pi`, add `2*pi`,
accumulating a deterministic offset.

Instantaneous frequency:
- from unwrapped phase via finite differences (central interior, forward/backward at endpoints),
- converted to cycles/time by dividing by `2*pi`.

#### 3.5.11 DPSS tapers and multitaper PSD

Files:
- `v2/crates/math/src/signal/spectral/dpss.rs`
- `v2/crates/math/src/signal/spectral/multitaper.rs`
- `v2/crates/math/src/signal/spectral/precomputed_dpss.rs`

DPSS tapers are computed deterministically via a bounded Lanczos method and small symmetric eigendecomposition (with explicit tolerances and caps), with:
- deterministic sign conventions for eigenvectors,
- optional deterministic caching keyed by `(n, nw, k)` including a precomputed asset for `n=10000` where available.

Multitaper PSD:
- apply each taper to the detrended signal,
- compute periodograms and average (with deterministic normalization).

#### 3.5.12 Lomb–Scargle (irregular sampling)

File: `v2/crates/math/src/signal/spectral/lomb_scargle.rs`

Implements classic time-shift Lomb–Scargle power for irregular time stamps `t_i` and values `x_i`.

Define `ω = 2*pi*f`.

Compute:
- `tau(ω) = (1/(2ω)) * atan2( Σ sin(2ω t_i), Σ cos(2ω t_i) )`
- shifted terms:
  - `cos(ω(t_i-τ))`, `sin(ω(t_i-τ))`
- accumulators:
  - `C = Σ x_i cos(ω(t_i-τ))`
  - `S = Σ x_i sin(ω(t_i-τ))`
  - `CC = Σ cos^2(ω(t_i-τ))`
  - `SS = Σ sin^2(ω(t_i-τ))`
- power:
  - `P(ω) = 0.5 * (C^2/CC + S^2/SS)` when denominators are positive; otherwise `0`.

Normalization option:
- unnormalized: return `P(ω)`,
- by variance: return `P(ω) / var(x_centered)` with the module’s explicit variance convention.

#### 3.5.13 Phase locking value (PLV)

File: `v2/crates/math/src/signal/spectral/phase_coherence.rs`

PLV from phases:
- `PLV = | (1/M) Σ exp(i*(phi_x - phi_y)) |`

PLV from signals:
- extract analytic signal amplitude/phase via Hilbert transform,
- optionally gate samples where either amplitude is below `min_amplitude`,
- compute PLV over remaining samples.

Output range: `[0,1]` (validated/clamped).

### 3.6 Multifractal (`signal::multifractal`)

Folder: `v2/crates/math/src/signal/multifractal/`

#### 3.6.1 MF-DFA core

File: `v2/crates/math/src/signal/multifractal/mfdfa.rs`

Implements MF-DFA core computation:
- build profile (integrated demeaned series),
- split into segments at each scale,
- detrend each segment by polynomial of degree `p`,
- compute fluctuation function `F_q(s)` over `q` values.

All bounds (scales, q array sizes, degrees) are explicit and validated.

#### 3.6.2 WTMM core

File: `v2/crates/math/src/signal/multifractal/wtmm.rs`

Implements WTMM partition functions using a wavelet (Mexican hat in the implementation) via FFT convolution:
- compute wavelet transform across scales,
- extract modulus maxima and compute partition sums for `q`.

This is transform-level computation only; significance testing is out of scope.

### 3.7 Entropy and complexity (`signal::entropy`, `signal::mse`, `signal::rqa`)

#### 3.7.1 Permutation entropy

File: `v2/crates/math/src/signal/entropy.rs`

Compute ordinal patterns of embedding dimension `m` and delay `tau`, count pattern frequencies, and compute:
- entropy in nats: `H = - Σ p_i ln p_i`
- normalized entropy: `H_norm = H / ln(m!)`

Determinism requires explicit tie handling; the implementation uses deterministic ranking/tie-break conventions.

#### 3.7.2 Sample entropy (SampEn), Chebyshev metric

File: `v2/crates/math/src/signal/entropy.rs`

Given embedding dimension `m`, delay `tau`, tolerance `r`, define template vectors `u_i` and count matches:
- Chebyshev distance `d(u_i, u_j) = max_k |u_i[k] - u_j[k]|`
- let `A` be number of matches of length `m+1`, `B` matches of length `m`
- `SampEn = -ln(A/B)`

Implementation provides:
- baseline exact method and optimized exact method(s),
- deterministic counting and explicit failure when `A==0` or `B==0` as per the module’s failure contract.

#### 3.7.3 Multiscale entropy (MSE)

File: `v2/crates/math/src/signal/mse.rs`

Coarse-graining by scale `s` (mean aggregation) then apply SampEn at each scale under explicit caps:
- `cg_s[k] = mean( x[k*s .. (k+1)*s) )`
- `MSE[s] = SampEn(cg_s)`

#### 3.7.4 RQA primitives

File: `v2/crates/math/src/signal/rqa.rs`

Build recurrence relations on embedded vectors under epsilon threshold and compute:
- recurrence rate,
- determinism, laminarity,
- diagonal and vertical line statistics,
under explicit sampling bounds (`RqaSampling`) for time-bounded execution.

### 3.8 DCCA (`signal::dcca`)

File: `v2/crates/math/src/signal/dcca.rs`

Implements DCCA correlation coefficient across scales:
- build integrated profiles,
- compute detrended covariance and variances over windows,
- output `rho_DCCA(s)` per scale `s`.

The optimized implementation uses prefix sums to avoid repeated window rescans while preserving the estimator definition up to floating rounding; equivalence tests are part of the module test suite.

### 3.9 Zero-crossing and sign runs (`signal::zero_crossing`)

File: `v2/crates/math/src/signal/zero_crossing.rs`

Zero-crossing rate:
- count sign changes under explicit `ZeroHandling` convention and normalize by `(n-1)`.

Sign run statistics:
- count run lengths of sign segments under explicit `ZeroHandling` and return summary statistics.

### 3.10 SSA (`signal::ssa`)

File: `v2/crates/math/src/signal/ssa.rs`

Implements SSA rank-`r` reconstruction using:
- trajectory/Hankel embedding matrix `X` with window length `L`,
- covariance `S = X X^T`,
- symmetric eigendecomposition of `S`,
- select top `r` eigenvectors and reconstruct by diagonal averaging (Hankelization).

Centering is optional (`center` flag), explicitly defined as subtracting/adding back the sample mean.

### 3.11 MODWT wavelet coherence (`signal::wavelet_coherence`)

File: `v2/crates/math/src/signal/wavelet_coherence.rs`

This primitive defines a real (non-complex) multiscale dependence measure using MODWT detail coefficients:
- compute detail coefficients `w_x(t)`, `w_y(t)` at `(family, level)`,
- define centered-window means of products and squares,
- coherence per time:
  - `C(t) = (S_xy(t)^2) / (S_xx(t) S_yy(t))` if denominator positive else `0`,
- mean coherence aggregates over time, and the series variant returns `C(t)` for each `t`.

The smoothing is time-bounded and deterministic and is implemented in `O(n)` time using prefix sums.

### 3.12 Signal shape measures (`signal::shape`)

File: `v2/crates/math/src/signal/shape.rs`

Hjorth parameters:
- activity = variance of `x`,
- mobility = sqrt(var(dx)/var(x)),
- complexity = mobility(dx)/mobility(x).

Teager–Kaiser energy operator:
- `psi[x_i] = x_i^2 - x_{i-1} x_{i+1}`,
- `teager_kaiser_energy_mean` returns average `psi` over interior points.

Spectral reducers (from a non-negative spectrum `p[k]`):
- spectral flatness:
  - `flatness = exp(mean(ln(max(p, eps)))) / mean(max(p, eps))`
- crest factor:
  - `crest = max(p) / mean(p)` with explicit `mean==0` convention
- spectral entropy:
  - normalize `p` to probabilities and compute `H = -Σ p ln p`, normalized by `ln(n_bins)`.

The periodogram-based convenience `spectral_flatness(x, eps)` uses mean-detrended periodogram, then applies the reducer.

---

## 4. NUMERICAL STABILITY AND EDGE CASES

This module’s numerical safety policy is enforced primarily by:
- rejecting non-finite inputs,
- explicit parameter bounds,
- explicit handling of near-zero denominators (returning `Err` or a documented finite convention),
- avoiding uncontrolled growth (bounded iteration counts where iterative methods exist, e.g. DPSS).

Examples of explicit safeguards:
- Lomb–Scargle returns `0` when a denominator accumulator collapses to non-positive due to sampling/frequency degeneracy.
- PLV gating prevents undefined phase use when analytic-signal amplitude is below a threshold (user-controlled).
- DPSS construction validates orthonormality/residuals and rejects unstable eigensolutions.
- Wavelet coherence clamps outputs to `[0,1]` and avoids division by non-positive denominators.

Numerical stability tests exist in `v2/crates/math/src/signal/tests/` for representative cases (large offsets, determinism, workspace equivalence).

---

## 5. PERFORMANCE AND ALLOCATION DISCIPLINE

Performance requirements and bench standards are defined by protocol:
- standard sizes: `n ∈ {100, 1000, 10000}`,
- workspace APIs must exist for repeated cron workloads where relevant,
- bench results are append-only in `v2/crates/math/src/signal/docs/signal_bench_results.md`.

This module provides workspace APIs for the performance-sensitive components:
- detrending polynomial workspaces,
- MODWT workspaces,
- SavGol, Hilbert/FFT, DPSS/multitaper, Lomb–Scargle, PLV, SSA, wavelet coherence, and other internal buffers.

This review does not claim “globally optimal” performance; it only asserts:
- complexity class matches the implemented algorithm,
- there are no accidental superlinear paths in the default implementations beyond what the algorithm requires (and where a bug existed, it was fixed and benchmarked with evidence in the bench log).

---

## 6. WHAT IS PROVED VS WHAT IS TESTED

This module is not a theorem prover; correctness evidence is:
- definitions documented here (mathematical object defined explicitly),
- deterministic implementation mapping,
- unit tests validating:
  - exact cases and invariants,
  - workspace equivalence,
  - determinism,
  - failure contracts,
  - numerical sanity on stress inputs,
- benchmark harness + append-only logs for performance discipline.

Where the algorithms are standard, the review states the definition and the implementation mapping.  
Where a proof of correctness would require a full numerical analysis (e.g. DPSS Lanczos convergence properties), the module relies on bounded residual checks and explicit rejection on instability rather than claiming unconditional correctness.

---

## 7. CONCLUSION (MODULE READINESS)

Within the declared scope:
- the mathematical objects implemented by `math::signal` are explicitly defined,
- the failure contracts are explicit and enforced by code,
- determinism is enforced by design,
- test coverage exists for correctness, edge cases, and numerical stability,
- benchmark harnesses exist and results are logged append-only.

Any extension beyond the current scope must be specified first in `SIGNAL_SPEC.md` and `scope.md`, and must follow the protocol (including tests, benches, and an updated review where needed).
