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

# `math::signal` — inventory (DRAFT)

This inventory lists what exists today in `v2/crates/math/src/signal/` (public API + workspaces + docs/benches).

Protocol: `v2/crates/math/docs/protocols/module_creation_protocol.md`

---

## 1) Module tree (source)

Top-level modules (see `v2/crates/math/src/signal/mod.rs`):
- `dfa`, `detrending`, `wavelets`, `filtering/*`, `spectral/*`, `multifractal/*`
- v1.3 additions: `entropy`, `rqa`, `dcca`, `mse`, `zero_crossing`
- v1.4 additions: `ssa`
- v1.4 additions (continued): `wavelet_coherence`
- v1.4 additions (continued): `shape`
- shared enums: `types`

---

## 2) Public API (by module)

All public APIs return `MathResult<T>` and reject non-finite inputs unless explicitly documented otherwise.

### 2.1 `signal::types`

File: `v2/crates/math/src/signal/types.rs`
- `DetrendMethod` (`None | RemoveMean | RemoveLinear | RemovePolynomial{degree}`)
- `WaveletFamily` (`ModwtD4 | ModwtD6 | ModwtD8 | Haar`)
- `WindowFunction` (`Rectangular | Hann | Hamming | Blackman`)

### 2.2 `signal::detrending`

File: `v2/crates/math/src/signal/detrending.rs`
- Core detrending:
  - `detrend(values, method) -> Vec<f64>`
  - `detrend_into(values, method, out)`
- Polynomial detrend (QR least squares on normalized `t=i/(n-1)`):
  - `PolynomialDetrendWorkspace`
  - `detrend_polynomial_into_with_workspace(values, degree, out, &mut PolynomialDetrendWorkspace)`
- Polynomial detrend (precomputed QR for fixed `(n, degree)`):
  - `PolynomialDetrendPrecomputedWorkspace`
  - `detrend_polynomial_precomputed_into_with_workspace(values, degree, out, &mut PolynomialDetrendPrecomputedWorkspace)`

### 2.3 `signal::dfa`

File: `v2/crates/math/src/signal/dfa.rs`
- `integrate_series(values) -> Vec<f64>` (mean-centered cumulative sum; delegates to `core::integration`)
- `segment_fluctuation_rms_linear(segment) -> f64` (OLS detrend RMS)
- `generate_window_sizes(n, min_size, max_size_factor) -> Vec<usize>` (geometric sequence)

### 2.4 `signal::wavelets`

File: `v2/crates/math/src/signal/wavelets.rs`
- MODWT detail coefficients (circular boundary):
  - `ModwtD4Workspace`
  - `modwt_d4_detail_level(values, level) -> Vec<f64>`
  - `modwt_d4_detail_level_into_with_workspace(values, level, out, &mut ModwtD4Workspace)`
  - generalized families:
    - `modwt_detail_level(values, family, level) -> Vec<f64>`
    - `modwt_detail_level_into_with_workspace(values, family, level, out, &mut ModwtD4Workspace)`
- Wavelet variance primitives:
  - `wavelet_variance(values, family, scale) -> f64`
  - `wavelet_variance_modwt(values, family, scale) -> f64`
  - `wavelet_variance_modwt_d4(values, scale) -> f64`
  - `wavelet_variance_haar(values, scale) -> f64`
- Denoising / thresholding:
  - `ThresholdKind` (`Hard | Soft`)
  - `threshold_coefficients_in_place(coeffs, threshold, kind)`
  - `universal_threshold(sigma, n) -> f64`
  - MODWT D4 denoise:
    - `ModwtD4DenoiseWorkspace`
    - `modwt_d4_denoise_into_with_workspace(values, levels, threshold, kind, out, &mut ModwtD4DenoiseWorkspace)`
    - `modwt_d4_denoise(values, levels, threshold, kind) -> Vec<f64>`
  - MODWT denoise (families):
    - `ModwtDenoiseWorkspace`
    - `modwt_denoise_into_with_workspace(values, family, levels, threshold, kind, out, &mut ModwtDenoiseWorkspace)`
    - `modwt_denoise(values, family, levels, threshold, kind) -> Vec<f64>`

### 2.5 `signal::filtering`

Folder: `v2/crates/math/src/signal/filtering/`

`signal::filtering::savgol` (`v2/crates/math/src/signal/filtering/savgol.rs`)
- `EdgeMode` (currently `Nearest`)
- `SavGolWorkspace` (caches coefficients)
- `savgol_apply_into_with_workspace(x, window_len, poly_order, deriv_order, delta, edge, out, &mut SavGolWorkspace)`
- `savgol_apply(x, window_len, poly_order, deriv_order, delta, edge) -> Vec<f64>`

`signal::filtering::kalman` (`v2/crates/math/src/signal/filtering/kalman.rs`)
- 1D local level:
  - `kalman_local_level_filter_into(y, r, q, init_mean, init_var, out_mean, out_var)`
  - `kalman_local_level_filter(y, r, q, init_mean, init_var) -> (Vec<f64>, Vec<f64>)`
- 2D local linear trend:
  - `kalman_local_linear_trend_filter_into(y, r, q_level, q_trend, init_level, init_trend, init_var_level, init_var_trend, out_level, out_trend, out_var_level, out_var_trend)`
  - `kalman_local_linear_trend_filter(...) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>)`

`signal::filtering::biquad` (`v2/crates/math/src/signal/filtering/biquad.rs`)
- `BiquadCoeffs` (+ `identity()`, `validate_stable()`)
- `BiquadDf2TState` (+ `reset()`)
- `BiquadDf2T` (+ `new()`, `process_sample()`, `apply_into()`, `apply_in_place()`)
- coefficient factories:
  - `biquad_lowpass_butterworth(f_cyc)`
  - `biquad_highpass_butterworth(f_cyc)`
  - `biquad_lowpass(f_cyc, q)`
  - `biquad_highpass(f_cyc, q)`

### 2.6 `signal::spectral`

Folder: `v2/crates/math/src/signal/spectral/` (re-exports from `v2/crates/math/src/signal/spectral/mod.rs`)

- Lomb–Scargle periodogram (irregular sampling):
  - `LombScargleNormalization` (`Unnormalized | ByVariance`)
  - `LombScargleConfig` (`normalization`, `center`)
  - `LombScargleWorkspace`
  - `lomb_scargle_power(t, x, freqs_hz, &cfg) -> Vec<f64>`
  - `lomb_scargle_power_into_with_workspace(t, x, freqs_hz, &cfg, out, &mut LombScargleWorkspace)`
- Phase locking value (PLV):
  - `PhaseLockingValueConfig` (`min_amplitude`, `min_samples`)
  - `PhaseLockingValueWorkspace`
  - `phase_locking_value_from_phase(phase_x, phase_y) -> f64`
  - `phase_locking_value_from_phase_with_config(phase_x, phase_y, &cfg) -> f64`
  - `phase_locking_value_from_signals(x, y, &cfg) -> f64`
  - `phase_locking_value_from_signals_with_workspace(x, y, &cfg, &mut PhaseLockingValueWorkspace) -> f64`

- Periodogram:
  - `PeriodogramWorkspace`
  - `calculate_periodogram(data, detrend) -> Vec<f64>`
  - `calculate_periodogram_into(data, detrend, out, &mut PeriodogramWorkspace)`
- Welch PSD:
  - `WelchWorkspace`
  - `calculate_welch_power_spectrum_into(data, window, step, detrend, out, &mut WelchWorkspace)`
- Multitaper PSD (DPSS):
  - `MultitaperWorkspace`, `DpssWorkspace`
  - `compute_dpss_tapers_flat_into_with_workspace(n, nw, k, out_tapers_flat, &mut DpssWorkspace)`
  - `calculate_multitaper_power_spectrum_into_with_workspace(data, nw, k, detrend, out, &mut MultitaperWorkspace)`
- Goertzel:
  - `GoertzelWorkspace`
  - `goertzel_power_bin(x, k) -> f64`
  - `goertzel_power_omega(x, omega) -> f64`
  - `goertzel_powers_bins_into_with_workspace(x, bins, out, &mut GoertzelWorkspace)`
- FFT autocorrelation:
  - `AutocorrelationFftWorkspace`
  - `calculate_autocorrelation_fft(data, max_lag, normalization) -> Vec<f64>`
  - `calculate_autocorrelation_fft_into(data, max_lag, normalization, out, &mut AutocorrelationFftWorkspace)`
- Coherence and cross-spectrum:
  - `CoherenceWorkspace`
  - `magnitude_squared_coherence(x, y, detrend) -> Vec<f64>`
  - `magnitude_squared_coherence_into(x, y, detrend, out, &mut CoherenceWorkspace)`
  - `CrossSpectrumWorkspace`, `CrossPhaseWorkspace`
  - `calculate_cross_spectrum_yx_into(y, x, detrend, out, &mut CrossSpectrumWorkspace)`
  - `cross_phase_and_group_delay_from_spectrum_yx_into(spectrum_yx, fs, out_phase, out_gd)`
  - `calculate_cross_phase_and_group_delay_yx_into_with_workspace(y, x, fs, detrend, out_phase, out_gd, &mut CrossPhaseWorkspace)`
- STFT / time-varying periodograms:
  - `StftPeriodogramWorkspace`
  - `stft_periodograms(...)`, `stft_periodograms_windowed(...)` (allocating convenience)
  - `stft_periodograms_with_workspace(...)`
  - `stft_periodograms_windowed_with_workspace(...)`
  - `_flat_into_with_workspace` variants for allocation discipline:
    - `stft_periodograms_flat_into_with_workspace(...)`
    - `stft_periodograms_windowed_flat_into_with_workspace(...)`
- Hilbert / analytic signal and phase derivatives:
  - `HilbertWorkspace`, `InstantaneousFrequencyWorkspace`
  - `calculate_analytic_signal(_into_with_workspace)`
  - `calculate_hilbert_transform(_into_with_workspace)`
  - `analytic_signal_amplitude_phase_into_with_workspace`
  - `unwrap_phase(_into)`
  - `instantaneous_angular_frequency_into_from_unwrapped_phase`
  - `instantaneous_frequency_hz_into_from_phase_with_workspace`
  - `instantaneous_frequency_hz_from_phase`
- Window functions:
  - `window_coefficients_into`, `window_coefficients`
  - `apply_window_into`

### 2.7 `signal::multifractal`

Folder: `v2/crates/math/src/signal/multifractal/`
- MF-DFA:
  - `MfDfaWorkspace`, `MfDfaOutput`
  - `calculate_mfdfa(data, scales, q, poly_degree) -> MfDfaOutput`
  - `calculate_mfdfa_into_with_workspace(...)`
  - `calculate_mfdfa_with_workspace(...)`
- WTMM:
  - `WtmmWorkspace`
  - `calculate_wtmm_partition_functions(data, scales, q) -> Vec<f64>`
  - `calculate_wtmm_partition_functions_into_with_workspace(...)`
  - `calculate_wtmm_partition_functions_with_workspace(...)`

### 2.8 `signal::entropy`

File: `v2/crates/math/src/signal/entropy.rs`
- Permutation entropy:
  - `PermutationEntropyWorkspace`
  - `permutation_entropy_into_with_workspace(data, m, tau, &mut PermutationEntropyWorkspace) -> (H_nats, H_norm)`
  - `permutation_entropy(data, m, tau) -> (H_nats, H_norm)`
- Sample entropy (SampEn), Chebyshev metric:
  - `SampleEntropyWorkspace`
  - `sample_entropy_chebyshev(data, m, tau, r, &mut SampleEntropyWorkspace) -> f64` (exact; auto-selects best exact method)
  - Exact variants (benchmarking/cross-check support): `sample_entropy_chebyshev_exact_sorted_window`, `sample_entropy_chebyshev_exact_grid`

### 2.9 `signal::rqa`

File: `v2/crates/math/src/signal/rqa.rs`
- `RqaSampling` (`All | DeterministicSubsample{max_templates}`)
- `RqaConfig` (embedding, epsilon, min line lengths, RR diagonal inclusion, sampling)
- `RqaMetrics` (RR, DET, LAM, line stats, template count)
- `RqaWorkspace`
- `rqa_metrics(values, &cfg) -> RqaMetrics`
- `rqa_metrics_with_workspace(values, &cfg, &mut RqaWorkspace) -> RqaMetrics`

### 2.10 `signal::dcca`

File: `v2/crates/math/src/signal/dcca.rs`
- `DccaWorkspace`
- `dcca_rho_into_with_workspace(x, y, scales, out_rho, &mut DccaWorkspace)`
- `dcca_rho(x, y, scales) -> Vec<f64>`

### 2.11 `signal::mse`

File: `v2/crates/math/src/signal/mse.rs`
- `MseWorkspace` (reuses coarse buffer + SampEn workspace)
- `coarse_grain_mean_into(data, scale, out)`
- `coarse_grain_mean(data, scale) -> Vec<f64>`
- `multiscale_sample_entropy_into_with_workspace(data, m, tau, r, max_scale, max_scale_cap, out, &mut MseWorkspace)`
- `multiscale_sample_entropy(data, m, tau, r, max_scale, max_scale_cap) -> Vec<f64>`

### 2.12 `signal::zero_crossing`

File: `v2/crates/math/src/signal/zero_crossing.rs`
- `ZeroHandling` (`AsZero | CarryForward | MapToPositive`)
- `SignRunStats`
- `zero_crossing_rate(data, zero_handling) -> f64`
- `sign_run_stats(data, zero_handling) -> SignRunStats`

### 2.13 `signal::ssa`

File: `v2/crates/math/src/signal/ssa.rs`
- `SsaConfig` (`window_len`, `rank`, `center`)
- `SsaWorkspace`
- `ssa_reconstruct_rank_r(values, cfg) -> Vec<f64>`
- `ssa_reconstruct_rank_r_into_with_workspace(values, cfg, out, &mut SsaWorkspace)`

### 2.14 `signal::wavelet_coherence`

File: `v2/crates/math/src/signal/wavelet_coherence.rs`
- `WaveletCoherenceConfig` (`family`, `level`, `smooth_window`)
- `WaveletCoherenceWorkspace`
- `wavelet_coherence_modwt_level_mean(x, y, &cfg) -> f64`
- `wavelet_coherence_modwt_level_mean_with_workspace(x, y, &cfg, &mut WaveletCoherenceWorkspace) -> f64`
- `wavelet_coherence_modwt_level_series_into_with_workspace(x, y, &cfg, out, &mut WaveletCoherenceWorkspace)`

### 2.15 `signal::shape`

File: `v2/crates/math/src/signal/shape.rs`
- `HjorthParameters` (`activity`, `mobility`, `complexity`)
- `hjorth_parameters(x) -> HjorthParameters`
- `teager_kaiser_energy_mean(x) -> f64`
- `spectral_flatness_from_periodogram(p, eps) -> f64`
- `spectral_crest_from_periodogram(p) -> f64`
- `spectral_entropy_from_periodogram(p, eps) -> f64`
- Convenience:
  - `spectral_flatness(x, eps) -> f64` (periodogram with mean detrend, then flatness)

---

## 3) Determinism and parallelism

- Default behavior is deterministic (no RNG).
- Some heavy computations use Rayon internally (e.g. SampEn baseline counting), but results are deterministic because they are pure integer counts combined by associative summation.
- Any tie-breaking (e.g. permutation entropy) is deterministic by construction.

---

## 4) Allocation discipline (workspaces)

General rule: prefer the `_into_*with_workspace` variants in cron workloads.

Workspaces:
- `PolynomialDetrendWorkspace`, `PolynomialDetrendPrecomputedWorkspace`
- `ModwtD4Workspace`, `ModwtD4DenoiseWorkspace`, `ModwtDenoiseWorkspace`
- `SavGolWorkspace`
- `HilbertWorkspace`, `InstantaneousFrequencyWorkspace`
- `PeriodogramWorkspace`, `WelchWorkspace`, `MultitaperWorkspace`, `DpssWorkspace`, `GoertzelWorkspace`, `AutocorrelationFftWorkspace`, `CoherenceWorkspace`, `CrossSpectrumWorkspace`, `CrossPhaseWorkspace`, `StftPeriodogramWorkspace`
- `LombScargleWorkspace`
- `PhaseLockingValueWorkspace`
- `PermutationEntropyWorkspace`, `SampleEntropyWorkspace`
- `RqaWorkspace`, `DccaWorkspace`, `MseWorkspace`
- `SsaWorkspace`
- `WaveletCoherenceWorkspace`

---

## 5) Benchmarks and logs

- Benches live in `v2/crates/math/benches/` and use the standard sizes `n ∈ {100, 1000, 10000}`.
- Append-only benchmark log: `v2/crates/math/src/signal/docs/signal_bench_results.md`.

---

## 6) Tests

- Tests live in `v2/crates/math/src/signal/tests/` (files named `test_*.rs`).
- Coverage includes: math correctness, edge cases, numerical stability, failure contracts, panic-safety, determinism, and performance sanity (via benches + append-only logs).
