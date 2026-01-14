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

# `math::signal` — benchmark results (append-only)

Append-only log. Do not rewrite historical entries.

---

## 2026-01-11T16:04:52Z — baseline benches (codex-cli)

Environment:
- OS: `Linux tia 5.15.0-156-generic x86_64`
- CPU: `Intel(R) Xeon(R) W-2295 CPU @ 3.00GHz` (36 threads)
- Toolchain: `rustc 1.90.0`, `cargo 1.90.0`
- Profile: `bench` (optimized, Criterion)

### Command: `cargo bench -p math --bench signal_dfa`

Raw stdout: `v2/tmp_local_math_codex/signal_dfa_bench_stdout.txt`

Key results:
- `signal_dfa_segment_fluctuation_n100`: `[352.17 ns 355.37 ns 359.13 ns]`
- `signal_dfa_segment_fluctuation_n1000`: `[3.1571 µs 3.1610 µs 3.1650 µs]`
- `signal_dfa_segment_fluctuation_n10000`: `[31.529 µs 31.569 µs 31.606 µs]`
- `signal_dfa_generate_window_sizes_n100k`: `[425.82 ns 426.83 ns 428.00 ns]` (non-standard size; legacy bench)

### Command: `cargo bench -p math --bench signal_detrending`

Raw stdout: `v2/tmp_local_math_codex/signal_detrending_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- Mean detrend:
  - `signal_detrend_mean_n100`: `[289.89 ns 290.24 ns 290.60 ns]`
  - `signal_detrend_mean_n1000`: `[2.1233 µs 2.1271 µs 2.1320 µs]`
  - `signal_detrend_mean_n10000`: `[21.687 µs 21.725 µs 21.771 µs]`
- Linear detrend:
  - `signal_detrend_linear_n100`: `[440.23 ns 440.97 ns 441.74 ns]`
  - `signal_detrend_linear_n1000`: `[3.5774 µs 3.5819 µs 3.5863 µs]`
  - `signal_detrend_linear_n10000`: `[36.059 µs 36.132 µs 36.219 µs]`
- Polynomial detrend (degree 2):
  - `signal_detrend_poly2_n100`: `[4.3707 µs 4.3817 µs 4.3957 µs]`
  - `signal_detrend_poly2_n1000`: `[44.803 µs 44.919 µs 45.068 µs]`
  - `signal_detrend_poly2_n10000`: `[385.68 µs 388.59 µs 393.09 µs]`

### Command: `cargo bench -p math --bench signal_wavelets`

Raw stdout: `v2/tmp_local_math_codex/signal_wavelets_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- MODWT D4 (detail level 4):
  - `signal_modwt_d4_level4_n100`: `[4.8533 µs 4.8742 µs 4.9056 µs]`
  - `signal_modwt_d4_level4_n1000`: `[47.784 µs 47.874 µs 47.972 µs]`
  - `signal_modwt_d4_level4_n10000`: `[478.59 µs 479.23 µs 479.96 µs]`
- Haar wavelet variance (scale 8):
  - `signal_haar_variance_n100_scale8`: `[1.0229 µs 1.0276 µs 1.0339 µs]`
  - `signal_haar_variance_n1000_scale8`: `[10.038 µs 10.064 µs 10.095 µs]`
  - `signal_haar_variance_n10000_scale8`: `[101.17 µs 101.32 µs 101.50 µs]`

---

## 2026-01-11T16:16:10Z — detrend polynomial precomputed QR (codex-cli)

Change scope:
- Added polynomial detrending API that reuses a precomputed QR factorization of the design matrix.

Command: `cargo bench -p math --bench signal_detrending`

Raw stdout: `v2/tmp_local_math_codex/signal_detrending_bench_stdout_v2_precomputed_qr.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- Polynomial detrend (degree 2, allocating API):
  - `signal_detrend_poly2_n100`: `[4.1729 µs 4.1955 µs 4.2369 µs]`
  - `signal_detrend_poly2_n1000`: `[43.074 µs 43.143 µs 43.221 µs]`
  - `signal_detrend_poly2_n10000`: `[375.77 µs 379.47 µs 384.48 µs]`
- Polynomial detrend (degree 2, workspace but QR recomputed each call):
  - `signal_detrend_poly2_ws_recompute_qr_n100`: `[3.8000 µs 3.8182 µs 3.8417 µs]`
  - `signal_detrend_poly2_ws_recompute_qr_n1000`: `[39.215 µs 39.402 µs 39.604 µs]`
  - `signal_detrend_poly2_ws_recompute_qr_n10000`: `[365.66 µs 367.11 µs 368.96 µs]`
- Polynomial detrend (degree 2, precomputed QR workspace):
  - `signal_detrend_poly2_ws_precomputed_qr_n100`: `[1.4793 µs 1.4816 µs 1.4841 µs]`
  - `signal_detrend_poly2_ws_precomputed_qr_n1000`: `[13.910 µs 13.949 µs 13.998 µs]`
  - `signal_detrend_poly2_ws_precomputed_qr_n10000`: `[139.66 µs 139.89 µs 140.14 µs]`

---

## 2026-01-11T16:43:01Z — v1.1 benches: spectral + multifractal core (codex-cli)

Environment:
- OS: `Linux tia 5.15.0-156-generic x86_64`
- CPU: `Intel(R) Xeon(R) W-2295 CPU @ 3.00GHz` (36 threads)
- Toolchain: `rustc 1.90.0`, `cargo 1.90.0`
- Profile: `bench` (optimized, Criterion)

### Command: `cargo bench -p math --bench signal_spectral`

Raw stdout: `v2/tmp_local_math_codex/signal_spectral_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- Periodogram (mean detrend):
  - `signal_periodogram_n100`: `[3.7630 µs 3.7821 µs 3.8061 µs]`
  - `signal_periodogram_n1000`: `[29.072 µs 29.123 µs 29.180 µs]`
  - `signal_periodogram_n10000`: `[282.75 µs 283.57 µs 284.63 µs]`
- FFT autocorrelation (lag 10):
  - `signal_autocorrelation_fft_n100_lag10`: `[10.433 µs 10.460 µs 10.489 µs]`
  - `signal_autocorrelation_fft_n1000_lag10`: `[94.297 µs 94.440 µs 94.587 µs]`
  - `signal_autocorrelation_fft_n10000_lag10`: `[2.0383 ms 2.0439 ms 2.0510 ms]`
- Coherence (mean detrend):
  - `signal_coherence_n100`: `[4.9760 µs 4.9823 µs 4.9888 µs]`
  - `signal_coherence_n1000`: `[38.498 µs 39.063 µs 40.167 µs]`
  - `signal_coherence_n10000`: `[395.46 µs 397.79 µs 401.00 µs]`
- STFT periodograms (w=64, max_windows=10, mean detrend):
  - `signal_stft_periodograms_n100_w64`: `[2.5521 µs 2.5590 µs 2.5676 µs]`
  - `signal_stft_periodograms_n1000_w64`: `[7.8833 µs 7.9019 µs 7.9241 µs]`
  - `signal_stft_periodograms_n10000_w64`: `[11.601 µs 11.614 µs 11.628 µs]`

### Command: `cargo bench -p math --bench signal_multifractal`

Raw stdout: `v2/tmp_local_math_codex/signal_multifractal_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- MF-DFA (degree=1, scales={32,64}, q={0,2}):
  - `signal_mfdfa_n100`: `[3.2473 µs 3.2524 µs 3.2583 µs]`
  - `signal_mfdfa_n1000`: `[11.169 µs 11.179 µs 11.191 µs]`
  - `signal_mfdfa_n10000`: `[110.74 µs 110.89 µs 111.03 µs]`
- WTMM partition (Mexican hat, scales={2,4,8}, q={0,2}):
  - `signal_wtmm_partition_n100`: `[30.540 µs 30.587 µs 30.639 µs]`
  - `signal_wtmm_partition_n1000`: `[138.34 µs 138.65 µs 139.02 µs]`
  - `signal_wtmm_partition_n10000`: `[1.3456 ms 1.3485 ms 1.3516 ms]`

---

## 2026-01-11T16:57:08Z — easy wins: workspace/into benches + fixed seed (codex-cli)

Change scope:
- Benches now measure allocation-disciplined `*_into` / workspace APIs and use deterministic fixed-seed input generation.

### Command: `cargo bench -p math --bench signal_spectral`

Raw stdout: `v2/tmp_local_math_codex/signal_spectral_bench_stdout_ws.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- Periodogram (workspace, mean detrend):
  - `signal_periodogram_ws_n100`: `[1.0266 µs 1.0281 µs 1.0294 µs]`
  - `signal_periodogram_ws_n1000`: `[9.8123 µs 9.8261 µs 9.8398 µs]`
  - `signal_periodogram_ws_n10000`: `[112.55 µs 112.80 µs 113.05 µs]`
- FFT autocorrelation (workspace, lag 10):
  - `signal_autocorrelation_fft_ws_n100_lag10`: `[1.5736 µs 1.5759 µs 1.5781 µs]`
  - `signal_autocorrelation_fft_ws_n1000_lag10`: `[17.609 µs 17.648 µs 17.688 µs]`
  - `signal_autocorrelation_fft_ws_n10000_lag10`: `[446.89 µs 448.19 µs 449.57 µs]`
- Coherence (workspace, mean detrend):
  - `signal_coherence_ws_n100`: `[1.9863 µs 1.9965 µs 2.0069 µs]`
  - `signal_coherence_ws_n1000`: `[18.380 µs 18.458 µs 18.521 µs]`
  - `signal_coherence_ws_n10000`: `[214.30 µs 214.66 µs 215.04 µs]`
- STFT periodograms (flat output, workspace, w=64, max_windows=10, mean detrend):
  - `signal_stft_periodograms_flat_ws_n100_w64`: `[1.1029 µs 1.1043 µs 1.1059 µs]`
  - `signal_stft_periodograms_flat_ws_n1000_w64`: `[5.5665 µs 5.5731 µs 5.5800 µs]`
  - `signal_stft_periodograms_flat_ws_n10000_w64`: `[9.1265 µs 9.1406 µs 9.1568 µs]`

### Command: `cargo bench -p math --bench signal_multifractal`

Raw stdout: `v2/tmp_local_math_codex/signal_multifractal_bench_stdout_ws.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- MF-DFA (workspace, degree=1, scales={32,64}, q={0,2}):
  - `signal_mfdfa_ws_n100`: `[2.9488 µs 2.9644 µs 2.9876 µs]`
  - `signal_mfdfa_ws_n1000`: `[10.963 µs 10.993 µs 11.026 µs]`
  - `signal_mfdfa_ws_n10000`: `[111.04 µs 111.61 µs 112.42 µs]`
- WTMM partition (workspace, Mexican hat, scales={2,4,8}, q={0,2}):
  - `signal_wtmm_partition_ws_n100`: `[22.366 µs 22.462 µs 22.594 µs]`
  - `signal_wtmm_partition_ws_n1000`: `[103.41 µs 103.68 µs 103.99 µs]`
  - `signal_wtmm_partition_ws_n10000`: `[1.0982 ms 1.1000 ms 1.1020 ms]`

---

## 2026-01-11T17:22:27Z — v1.2: Welch PSD (workspace) (codex-cli)

Command: `cargo bench -p math --bench signal_welch`

Raw stdout: `v2/tmp_local_math_codex/signal_welch_bench_stdout.txt`

Key results:
- `signal_welch_ws_n100_seg100`: `[3.2697 µs 3.2795 µs 3.2906 µs]`
- `signal_welch_ws_n1000_seg256`: `[22.336 µs 22.371 µs 22.409 µs]`
- `signal_welch_ws_n10000_seg256`: `[37.468 µs 37.526 µs 37.587 µs]`

---

## 2026-01-11T17:27:50Z — v1.2: Goertzel targeted bins (workspace) (codex-cli)

Command: `cargo bench -p math --bench signal_goertzel`

Raw stdout: `v2/tmp_local_math_codex/signal_goertzel_bench_stdout.txt`

Key results:
- `signal_goertzel_ws_n100_m8`: `[2.4033 µs 2.4383 µs 2.4756 µs]`
- `signal_goertzel_ws_n1000_m8`: `[28.126 µs 28.215 µs 28.290 µs]`
- `signal_goertzel_ws_n10000_m8`: `[259.20 µs 262.18 µs 265.55 µs]`

---

## 2026-01-11T18:10:55Z — v1.2: cross-spectrum + group delay (workspace) (codex-cli)

Command: `cargo bench -p math --bench signal_cross_spectrum`

Raw stdout: `v2/tmp_local_math_codex/signal_cross_spectrum_bench_stdout.txt`

Key results:
- `signal_cross_spectrum_ws_n100`: `[1.9720 µs 1.9771 µs 1.9832 µs]`
- `signal_cross_phase_gd_ws_n100`: `[7.2326 µs 7.2630 µs 7.3033 µs]`
- `signal_cross_spectrum_ws_n1000`: `[18.323 µs 18.393 µs 18.474 µs]`
- `signal_cross_phase_gd_ws_n1000`: `[67.297 µs 67.548 µs 67.836 µs]`
- `signal_cross_spectrum_ws_n10000`: `[209.09 µs 210.25 µs 211.51 µs]`
- `signal_cross_phase_gd_ws_n10000`: `[656.08 µs 657.35 µs 658.83 µs]`

---

## 2026-01-11T18:15:08Z — v1.2: cross-spectrum group-delay wrap (branch, no trig) (codex-cli)

Change scope:
- Replace `atan2(sin Δ, cos Δ)` wrap with branch wrap (valid for Δ ∈ [-2π, 2π]) to reduce post-FFT cost.

Command: `cargo bench -p math --bench signal_cross_spectrum`

Raw stdout: `v2/tmp_local_math_codex/signal_cross_spectrum_bench_stdout_wrap_opt.txt`

Key results:
- `signal_cross_spectrum_ws_n100`: `[1.9876 µs 1.9941 µs 2.0021 µs]`
- `signal_cross_phase_gd_ws_n100`: `[3.9627 µs 3.9681 µs 3.9733 µs]`
- `signal_cross_spectrum_ws_n1000`: `[18.574 µs 18.600 µs 18.625 µs]`
- `signal_cross_phase_gd_ws_n1000`: `[37.252 µs 37.360 µs 37.498 µs]`
- `signal_cross_spectrum_ws_n10000`: `[213.69 µs 213.98 µs 214.34 µs]`
- `signal_cross_phase_gd_ws_n10000`: `[395.02 µs 395.52 µs 396.04 µs]`

---

## 2026-01-11T18:33:34Z — v1.2: STFT periodograms (Hann window, workspace) (codex-cli)

Command: `cargo bench -p math --bench signal_stft_windowed`

Raw stdout: `v2/tmp_local_math_codex/signal_stft_windowed_bench_stdout.txt`

Key results:
- `signal_stft_hann_flat_ws_n100_w64_m10`: `[1.3850 µs 1.3868 µs 1.3887 µs]`
- `signal_stft_hann_flat_ws_n1000_w64_m10`: `[7.0279 µs 7.0514 µs 7.0840 µs]`
- `signal_stft_hann_flat_ws_n10000_w64_m10`: `[11.672 µs 11.686 µs 11.700 µs]`

---

## 2026-01-11T18:35:59Z — v1.2: multitaper PSD (DPSS cached, workspace) (codex-cli)

Notes:
- DPSS computation is cached in the workspace; the benchmark measures the repeated-call path (DPSS + FFT plan already warmed).
- DPSS generation is bounded to avoid O(n^2) memory blow-ups, so the largest bench size here is `n=4096`.

Command: `cargo bench -p math --bench signal_multitaper`

Raw stdout: `v2/tmp_local_math_codex/signal_multitaper_bench_stdout.txt`

Key results:
- `signal_multitaper_ws_n100_nw3_k5`: `[5.5235 µs 5.5313 µs 5.5394 µs]`
- `signal_multitaper_ws_n1000_nw3_k5`: `[52.852 µs 52.933 µs 53.012 µs]`
- `signal_multitaper_ws_n4096_nw3_k5`: `[224.10 µs 224.35 µs 224.59 µs]`

---

## 2026-01-11T18:47:30Z — v1.2: DPSS global cache + STFT windowing fast-path (codex-cli)

Change scope:
- DPSS: add bounded global LRU cache (shared across workspaces) to eliminate repeated eigensolves for `(n, nw, k)` triplets.
- STFT windowing: fuse copy+window multiply into one pass (avoid extend-from-slice + second pass).

### Command: `cargo bench -p math --bench signal_stft_windowed`

Raw stdout: `v2/tmp_local_math_codex/signal_stft_windowed_bench_stdout_opt.txt`

Key results:
- `signal_stft_hann_flat_ws_n100_w64_m10`: `[1.3692 µs 1.3725 µs 1.3761 µs]`
- `signal_stft_hann_flat_ws_n1000_w64_m10`: `[6.8789 µs 6.8889 µs 6.8987 µs]`
- `signal_stft_hann_flat_ws_n10000_w64_m10`: `[10.611 µs 10.653 µs 10.709 µs]`

### Command: `cargo bench -p math --bench signal_multitaper`

Raw stdout: `v2/tmp_local_math_codex/signal_multitaper_bench_stdout_opt.txt`

Key results:
- `signal_multitaper_ws_n100_nw3_k5`: `[5.4866 µs 5.4955 µs 5.5038 µs]`
- `signal_multitaper_ws_n1000_nw3_k5`: `[51.124 µs 51.266 µs 51.395 µs]`
- `signal_multitaper_ws_n4096_nw3_k5`: `[209.86 µs 210.68 µs 211.80 µs]`

---

## 2026-01-11T19:02:10Z — cold vs warm multitaper timing (ignored test) (codex-cli)

Command:
- `cargo test -p math --release signal -- signal::tests::test_spectral_multitaper_cold_warm_timing --ignored --nocapture`

Raw stdout: `v2/tmp_local_math_codex/signal_multitaper_cold_warm_test_stdout_release.txt`

Results (best-of-3 inside the test):
- `n=100, nw=3, k=5`: cold `3.991913ms`, warm `30.072µs` (≈`132.75x`)
- `n=1000, nw=3, k=5`: cold `941.054917ms`, warm `72.322µs` (≈`13012.01x`)

Notes:
- Cold includes dense DPSS eigensolve; warm hits the global DPSS cache and measures the repeated-call path.
- Cold timing at `n=10_000` is not measurable with the current DPSS algorithm (dense `n×n` eigensolve is not viable at that size).

---

## 2026-01-11T19:38:12Z — DPSS cold-start optimization (Lanczos on tridiagonal) (codex-cli)

Change scope:
- Replace dense `n×n` eigensolve DPSS with a deterministic Lanczos top‑K solver on the tridiagonal form (still cached globally).

### Command: cold/warm timing (ignored test)

Command:
- `cargo test -p math --release signal -- signal::tests::test_spectral_multitaper_cold_warm_timing --ignored --nocapture`

Raw stdout: `v2/tmp_local_math_codex/signal_multitaper_cold_warm_test_stdout_release_lanczos_v8.txt`

Results:
- `n=100, nw=3, k=5`: cold `5.846237ms`, warm `18.005µs`
- `n=1000, nw=3, k=5`: cold `32.598008ms`, warm `74.541µs`
- `n=10000, nw=3, k=5`: cold `14.004579254s`, warm `813.121µs`

### Command: warm-path bench

Command: `cargo bench -p math --bench signal_multitaper`

Raw stdout: `v2/tmp_local_math_codex/signal_multitaper_bench_stdout_after_dpss_lanczos.txt`

Key results:
- `signal_multitaper_ws_n100_nw3_k5`: `[5.3812 µs 5.3887 µs 5.3964 µs]`
- `signal_multitaper_ws_n1000_nw3_k5`: `[51.777 µs 51.845 µs 51.914 µs]`
- `signal_multitaper_ws_n4096_nw3_k5`: `[215.24 µs 215.57 µs 215.92 µs]`

---

## 2026-01-11T20:44:25Z — DPSS precomputed tapers for n=10k (codex-cli)

Change scope:
- Add a precomputed DPSS asset for the production triplet `(n=10_000, nw=3.0, k=5)` and load it before runtime DPSS solvers.

Command:
- `cargo test -p math --release signal -- signal::tests::test_spectral_multitaper_cold_warm_timing --ignored --nocapture`

Raw stdout: `v2/tmp_local_math_codex/signal_multitaper_cold_warm_test_stdout_release_precomputed.txt`

Results:
- `n=100, nw=3, k=5`: cold `6.442402ms`, warm `30.673µs`
- `n=1000, nw=3, k=5`: cold `33.320917ms`, warm `72.777µs`
- `n=10000, nw=3, k=5`: cold `1.013138ms`, warm `782.538µs`

---

## 2026-01-11T20:50:41Z — v1.2: Savitzky–Golay filter (workspace) (codex-cli)

Command: `cargo bench -p math --bench signal_savgol`

Raw stdout: `v2/tmp_local_math_codex/signal_savgol_bench_stdout.txt`

Key results:
- `signal_savgol_smooth_ws_n100_w11_p3`: `[1.6649 µs 1.6672 µs 1.6696 µs]`
- `signal_savgol_d1_ws_n100_w11_p3`: `[1.6622 µs 1.6650 µs 1.6682 µs]`
- `signal_savgol_smooth_ws_n1000_w11_p3`: `[16.412 µs 16.433 µs 16.453 µs]`
- `signal_savgol_d1_ws_n1000_w11_p3`: `[16.475 µs 16.519 µs 16.577 µs]`
- `signal_savgol_smooth_ws_n10000_w11_p3`: `[164.42 µs 164.63 µs 164.85 µs]`
- `signal_savgol_d1_ws_n10000_w11_p3`: `[164.11 µs 164.31 µs 164.51 µs]`

---

## 2026-01-11T21:12:32Z — v1.2: Savitzky–Golay optimization (precomputed coeffs + symmetry fast-path) (codex-cli)

Change scope:
- Remove `nalgebra` usage from SavGol coefficient generation (local small SPD Cholesky solve).
- Add precomputed coefficients for `(w=11,p=3,delta=1,edge=Nearest)` and use symmetry/antisymmetry fast convolution.

Command: `cargo bench -p math --bench signal_savgol`

Raw stdout: `v2/tmp_local_math_codex/signal_savgol_bench_stdout_opt2.txt`

Key results:
- `signal_savgol_smooth_ws_n100_w11_p3`: `[1.6693 µs 1.6714 µs 1.6740 µs]`
- `signal_savgol_d1_ws_n100_w11_p3`: `[1.4414 µs 1.4441 µs 1.4477 µs]`
- `signal_savgol_smooth_ws_n1000_w11_p3`: `[16.427 µs 16.449 µs 16.475 µs]`
- `signal_savgol_d1_ws_n1000_w11_p3`: `[14.166 µs 14.194 µs 14.231 µs]`
- `signal_savgol_smooth_ws_n10000_w11_p3`: `[163.86 µs 164.17 µs 164.58 µs]`
- `signal_savgol_d1_ws_n10000_w11_p3`: `[141.47 µs 142.26 µs 143.36 µs]`

---

## 2026-01-12T07:58:17Z — v1.2: Kalman filter primitives (workspace) (codex-cli)

Change scope:
- Validate performance for local-level and local-linear-trend primitives after removing redundant per-iteration finiteness checks (input finiteness is validated up-front).

Command: `cargo bench -p math --bench signal_kalman`

Raw stdout: `v2/tmp_local_math_codex/signal_kalman_bench_stdout_opt3.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- Local level:
  - `signal_kalman_local_level_ws_n100`: `[1.1040 µs 1.1048 µs 1.1055 µs]`
  - `signal_kalman_local_level_ws_n1000`: `[10.838 µs 10.849 µs 10.859 µs]`
  - `signal_kalman_local_level_ws_n10000`: `[108.91 µs 108.99 µs 109.07 µs]`
- Local linear trend:
  - `signal_kalman_local_trend_ws_n100`: `[2.1224 µs 2.1241 µs 2.1258 µs]`
  - `signal_kalman_local_trend_ws_n1000`: `[20.420 µs 20.438 µs 20.456 µs]`
  - `signal_kalman_local_trend_ws_n10000`: `[203.61 µs 203.79 µs 203.97 µs]`

---

## 2026-01-12T08:05:49Z — v1.2: Biquad (one-pass IIR) low/high-pass (workspace) (codex-cli)

Change scope:
- Add DF2T biquad filtering primitives with strict stability validation (Jury test), plus Butterworth coefficient helpers.
- Ensure hot loop has no redundant per-sample finiteness checks (input validated once up-front).

Command: `cargo bench -p math --bench signal_biquad`

Raw stdout: `v2/tmp_local_math_codex/signal_biquad_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- Low-pass (Butterworth, `f_cyc=0.05`):
  - `signal_biquad_lowpass_ws_n100`: `[749.69 ns 751.11 ns 752.66 ns]`
  - `signal_biquad_lowpass_ws_n1000`: `[7.2587 µs 7.2668 µs 7.2746 µs]`
  - `signal_biquad_lowpass_ws_n10000`: `[72.780 µs 72.906 µs 73.055 µs]`
- High-pass (Butterworth, `f_cyc=0.05`):
  - `signal_biquad_highpass_ws_n100`: `[750.76 ns 751.64 ns 752.55 ns]`
  - `signal_biquad_highpass_ws_n1000`: `[7.2820 µs 7.2962 µs 7.3142 µs]`
  - `signal_biquad_highpass_ws_n10000`: `[72.614 µs 72.688 µs 72.765 µs]`

---

## 2026-01-12T08:25:58Z — v1.2: Hilbert transform (analytic signal) (workspace) (codex-cli)

Change scope:
- Add FFT-based analytic signal construction to obtain `H{x}`, amplitude envelope, and instantaneous phase.

Command: `cargo bench -p math --bench signal_hilbert`

Raw stdout: `v2/tmp_local_math_codex/signal_hilbert_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- Analytic signal `z = x + i H{x}`:
  - `signal_hilbert_analytic_ws_n100`: `[1.0799 µs 1.0853 µs 1.0941 µs]`
  - `signal_hilbert_analytic_ws_n1000`: `[9.5771 µs 9.5900 µs 9.6030 µs]`
  - `signal_hilbert_analytic_ws_n10000`: `[130.90 µs 131.19 µs 131.44 µs]`
- Amplitude + phase (post analytic signal):
  - `signal_hilbert_amp_phase_ws_n100`: `[2.8686 µs 2.8726 µs 2.8765 µs]`
  - `signal_hilbert_amp_phase_ws_n1000`: `[31.161 µs 31.213 µs 31.266 µs]`
  - `signal_hilbert_amp_phase_ws_n10000`: `[361.71 µs 362.25 µs 362.82 µs]`

---

## 2026-01-12T08:38:42Z — v1.2: Wavelet denoising (MODWT D4 threshold) (workspace) (codex-cli)

Change scope:
- Add deterministic MODWT D4 decomposition/reconstruction with hard/soft thresholding of detail coefficients.

Command: `cargo bench -p math --bench signal_wavelet_denoise`

Raw stdout: `v2/tmp_local_math_codex/signal_wavelet_denoise_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- `signal_modwt_d4_denoise_ws_n100_lvl6`: `[13.870 µs 13.981 µs 14.221 µs]`
- `signal_modwt_d4_denoise_ws_n1000_lvl6`: `[133.62 µs 133.78 µs 133.95 µs]`
- `signal_modwt_d4_denoise_ws_n10000_lvl6`: `[1.3354 ms 1.3398 ms 1.3464 ms]`

---

## 2026-01-12T08:45:45Z — v1.2: Wavelet denoise optimization (remove inner-loop modulo) (codex-cli)

Change scope:
- Replace per-sample `% n` index wrapping in MODWT D4 decompose/reconstruct with per-level `d% n` precompute + branch-based wrap (`wrap_add`/`wrap_sub`).
- Keep the existing power-of-two `& (n-1)` fast path.

Command: `cargo bench -p math --bench signal_wavelet_denoise`

Raw stdout: `v2/tmp_local_math_codex/signal_wavelet_denoise_bench_stdout_wrap_fastpath.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- `signal_modwt_d4_denoise_ws_n100_lvl6`: `[7.0234 µs 7.0309 µs 7.0385 µs]`
- `signal_modwt_d4_denoise_ws_n1000_lvl6`: `[67.595 µs 67.682 µs 67.767 µs]`
- `signal_modwt_d4_denoise_ws_n10000_lvl6`: `[679.54 µs 680.68 µs 682.04 µs]`

---

## 2026-01-12T09:00:46Z — v1.2: Generalized MODWT families (D4/D6/D8) (codex-cli)

Change scope:
- Add MODWT support for additional Daubechies families (D6, D8) in addition to existing D4, with a shared fast wrapping implementation.

Command: `cargo bench -p math --bench signal_modwt_families`

Raw stdout: `v2/tmp_local_math_codex/signal_modwt_families_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`, `levels=6`):
- D4:
  - `signal_modwt_denoise_ModwtD4_ws_n100_lvl6`: `[7.4040 µs 7.4358 µs 7.4772 µs]`
  - `signal_modwt_denoise_ModwtD4_ws_n1000_lvl6`: `[71.582 µs 71.886 µs 72.296 µs]`
  - `signal_modwt_denoise_ModwtD4_ws_n10000_lvl6`: `[708.90 µs 710.45 µs 712.79 µs]`
- D6:
  - `signal_modwt_denoise_ModwtD6_ws_n100_lvl6`: `[10.971 µs 10.982 µs 10.994 µs]`
  - `signal_modwt_denoise_ModwtD6_ws_n1000_lvl6`: `[106.18 µs 106.32 µs 106.46 µs]`
  - `signal_modwt_denoise_ModwtD6_ws_n10000_lvl6`: `[1.0603 ms 1.0615 ms 1.0626 ms]`
- D8:
  - `signal_modwt_denoise_ModwtD8_ws_n100_lvl6`: `[14.442 µs 14.481 µs 14.542 µs]`
  - `signal_modwt_denoise_ModwtD8_ws_n1000_lvl6`: `[139.61 µs 139.83 µs 140.07 µs]`
  - `signal_modwt_denoise_ModwtD8_ws_n10000_lvl6`: `[1.3974 ms 1.3990 ms 1.4009 ms]`

---

## 2026-01-12T09:14:34Z — Power-of-two `n` bench variants (MODWT denoise) (codex-cli)

Purpose:
- Quantify the practical benefit of consumers using power-of-two `n` (enables `& (n-1)` wrap fast path).

Command:
- `cargo bench -p math --bench signal_wavelet_denoise`
- `cargo bench -p math --bench signal_modwt_families`

Raw stdout:
- `v2/tmp_local_math_codex/signal_wavelet_denoise_bench_stdout_pow2_sizes.txt`
- `v2/tmp_local_math_codex/signal_modwt_families_bench_stdout_pow2_sizes_full.txt`

Key results (additional sizes):
- D4 denoise (pow2):
  - `signal_modwt_d4_denoise_ws_pow2_n1024_lvl6`: `[59.531 µs 59.608 µs 59.691 µs]`
  - `signal_modwt_d4_denoise_ws_pow2_n8192_lvl6`: `[481.45 µs 485.38 µs 490.94 µs]`
- Denoise families (pow2, `levels=6`):
  - `signal_modwt_denoise_ModwtD4_ws_pow2_n1024_lvl6`: `[64.511 µs 64.640 µs 64.793 µs]`
  - `signal_modwt_denoise_ModwtD6_ws_pow2_n1024_lvl6`: `[88.383 µs 88.522 µs 88.667 µs]`
  - `signal_modwt_denoise_ModwtD8_ws_pow2_n1024_lvl6`: `[114.62 µs 115.04 µs 115.59 µs]`
  - `signal_modwt_denoise_ModwtD4_ws_pow2_n8192_lvl6`: `[514.27 µs 515.20 µs 516.26 µs]`
  - `signal_modwt_denoise_ModwtD6_ws_pow2_n8192_lvl6`: `[713.36 µs 715.46 µs 718.70 µs]`
  - `signal_modwt_denoise_ModwtD8_ws_pow2_n8192_lvl6`: `[914.44 µs 915.43 µs 916.49 µs]`

---

## 2026-01-12T09:27:02Z — v1.3: Entropy primitives (permutation entropy + sample entropy) (codex-cli)

Change scope:
- Add deterministic permutation entropy (ordinal patterns with stable tie-break) and sample entropy (Chebyshev distance) primitives under `math::signal`.

Command: `cargo bench -p math --bench signal_entropy`

Raw stdout: `v2/tmp_local_math_codex/signal_entropy_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- Permutation entropy (`m=5, tau=1`, workspace):
  - `signal_perm_entropy_m5_tau1_n100`: `[4.1890 µs 4.1977 µs 4.2080 µs]`
  - `signal_perm_entropy_m5_tau1_n1000`: `[48.510 µs 48.584 µs 48.661 µs]`
  - `signal_perm_entropy_m5_tau1_n10000`: `[513.79 µs 515.33 µs 517.38 µs]`
- Sample entropy (`m=2, tau=1, r=0.5`, parallel counting):
  - `signal_sample_entropy_m2_tau1_r0p5_n100`: `[46.589 µs 46.809 µs 47.026 µs]`
  - `signal_sample_entropy_m2_tau1_r0p5_n1000`: `[414.22 µs 415.03 µs 415.79 µs]`
  - `signal_sample_entropy_m2_tau1_r0p5_n10000`: `[33.001 ms 33.083 ms 33.168 ms]`

---

## 2026-01-12T09:36:53Z — v1.3: RQA primitives (bounded, deterministic subsample) (codex-cli)

Change scope:
- Add deterministic, time-bounded recurrence quantification analysis primitives under `math::signal::rqa`.

Command: `cargo bench -p math --bench signal_rqa`

Raw stdout: `v2/tmp_local_math_codex/signal_rqa_bench_stdout.txt`

Notes:
- For `n=10_000` the benchmark uses deterministic subsampling with `max_templates=2000` to keep runtime time-bounded.

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- `signal_rqa_m2_tau1_eps0p2_n100_t99`: `[41.230 µs 41.317 µs 41.402 µs]`
- `signal_rqa_m2_tau1_eps0p2_n1000_t999`: `[4.5613 ms 4.5688 ms 4.5779 ms]`
- `signal_rqa_m2_tau1_eps0p2_n10000_t2000`: `[18.570 ms 18.633 ms 18.717 ms]`

---

## 2026-01-12T10:12:29Z — SampEn exact counting optimization (sorted-window + parallel reduction) (codex-cli)

Change scope:
- Optimize `signal::entropy::sample_entropy_chebyshev` (exact SampEn) by switching from unconditional pair enumeration to:
  - sorted-window pruning on the first embedded coordinate,
  - deterministic parallel reduction for larger template counts,
  - exact `r=0` grouping fast path (hash by embedded vector bits),
  - exact all-pairs-match short-circuit when `range(data) <= r`.

Command: `cargo bench -p math --bench signal_entropy`

Raw stdout: `v2/tmp_local_math_codex/signal_entropy_bench_stdout_v4_sorted_window_par_smallseq.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- Permutation entropy (`m=5, tau=1`, workspace):
  - `signal_perm_entropy_m5_tau1_n100`: `[4.4607 µs 4.4655 µs 4.4702 µs]`
  - `signal_perm_entropy_m5_tau1_n1000`: `[50.908 µs 50.964 µs 51.024 µs]`
  - `signal_perm_entropy_m5_tau1_n10000`: `[530.99 µs 532.21 µs 533.70 µs]`
- Sample entropy (`m=2, tau=1, r=0.5`):
  - `signal_sample_entropy_m2_tau1_r0p5_n100`: `[15.716 µs 15.758 µs 15.799 µs]`
  - `signal_sample_entropy_m2_tau1_r0p5_n1000`: `[326.39 µs 327.81 µs 329.24 µs]`
  - `signal_sample_entropy_m2_tau1_r0p5_n10000`: `[18.850 ms 18.898 ms 18.976 ms]`

---

## 2026-01-12T10:18:50Z — v1.3: Hilbert phase unwrap + instantaneous frequency primitives (codex-cli)

Change scope:
- Add deterministic phase unwrapping and instantaneous frequency from phase primitives under `signal::spectral::hilbert`.

Command: `cargo bench -p math --bench signal_instantaneous_frequency`

Raw stdout: `v2/tmp_local_math_codex/signal_instantaneous_frequency_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- Phase unwrap:
  - `signal_unwrap_phase_n100`: `[197.44 ns 197.93 ns 198.52 ns]`
  - `signal_unwrap_phase_n1000`: `[1.8205 µs 1.8275 µs 1.8347 µs]`
  - `signal_unwrap_phase_n10000`: `[17.909 µs 17.978 µs 18.044 µs]`
- Instantaneous frequency from phase (unwrap + derivative + `/ (2*pi)`):
  - `signal_inst_freq_from_phase_n100`: `[407.65 ns 408.71 ns 409.91 ns]`
  - `signal_inst_freq_from_phase_n1000`: `[3.6089 µs 3.6661 µs 3.7273 µs]`
  - `signal_inst_freq_from_phase_n10000`: `[36.221 µs 36.278 µs 36.340 µs]`

---

## 2026-01-12T10:25:38Z — v1.3: DCCA primitives (rho_DCCA per scale) (codex-cli)

Change scope:
- Add deterministic DCCA primitives under `math::signal::dcca` (profile integration + linear detrend per window, forward+backward segmentation).

Command: `cargo bench -p math --bench signal_dcca`

Raw stdout: `v2/tmp_local_math_codex/signal_dcca_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- `signal_dcca_rho_scales3_n100`: `[4.3446 µs 4.3505 µs 4.3562 µs]` (scales: 8,16,32)
- `signal_dcca_rho_scales3_n1000`: `[37.118 µs 37.282 µs 37.489 µs]` (scales: 64,128,256)
- `signal_dcca_rho_scales3_n10000`: `[417.71 µs 418.67 µs 419.76 µs]` (scales: 64,128,256)

---

## 2026-01-12T10:31:10Z — DCCA perf optimization (prefix-sum window stats) (codex-cli)

Change scope:
- Optimize `math::signal::dcca` by computing per-window detrended variance/covariance from prefix sums (O(1) per window) instead of iterating over each window element.
- Added equivalence test comparing optimized implementation to a slow reference within `1e-10`.

Command: `cargo bench -p math --bench signal_dcca`

Raw stdout: `v2/tmp_local_math_codex/signal_dcca_bench_stdout_v2_prefix_sums.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- `signal_dcca_rho_scales3_n100`: `[2.3698 µs 2.3732 µs 2.3774 µs]` (scales: 8,16,32)
- `signal_dcca_rho_scales3_n1000`: `[13.850 µs 13.897 µs 13.960 µs]` (scales: 64,128,256)
- `signal_dcca_rho_scales3_n10000`: `[138.76 µs 138.92 µs 139.07 µs]` (scales: 64,128,256)

---

## 2026-01-12T12:29:08Z — v1.3: Multiscale entropy (MSE) wrapper over SampEn (codex-cli)

Change scope:
- Add bounded deterministic multiscale entropy primitives under `math::signal::mse`:
  - coarse-graining by mean over non-overlapping blocks,
  - per-scale SampEn evaluation for scales `1..=max_scale` (explicit cap).

Command: `cargo bench -p math --bench signal_mse`

Raw stdout: `v2/tmp_local_math_codex/signal_mse_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- `signal_mse_sampen_m2_tau1_r0p5_scales1to5_n100`: `[47.581 µs 47.649 µs 47.717 µs]`
- `signal_mse_sampen_m2_tau1_r0p5_scales1to5_n1000`: `[3.9844 ms 4.0026 ms 4.0205 ms]`
- `signal_mse_sampen_m2_tau1_r0p5_scales1to5_n10000`: `[32.228 ms 32.270 ms 32.317 ms]`

---

## 2026-01-12T12:41:16Z — SampEn auto-selection heuristic fix (avoid grid regressions) (codex-cli)

Change scope:
- Added an exact grid/box-hashing implementation for SampEn (Chebyshev metric), but restricted automatic selection with an explicit heuristic so the default path does not regress for moderate/large `r`.
- Added a correctness test comparing auto-selected SampEn to a baseline exact method within `1e-10`.

Commands:
- `cargo bench -p math --bench signal_entropy`
- `cargo bench -p math --bench signal_mse`

Raw stdout:
- `v2/tmp_local_math_codex/signal_entropy_bench_stdout_v6_grid_heuristic_fix.txt`
- `v2/tmp_local_math_codex/signal_mse_bench_stdout_v3_grid_heuristic_fix.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- SampEn (exact, auto-selected; `m=2,tau=1,r=0.5`):
  - `signal_sample_entropy_m2_tau1_r0p5_n100`: `[16.366 µs 16.410 µs 16.456 µs]`
  - `signal_sample_entropy_m2_tau1_r0p5_n1000`: `[330.89 µs 332.22 µs 333.53 µs]`
  - `signal_sample_entropy_m2_tau1_r0p5_n10000`: `[18.650 ms 18.695 ms 18.760 ms]`
- MSE (scales `1..=5`, uses SampEn; `m=2,tau=1,r=0.5`):
  - `signal_mse_sampen_m2_tau1_r0p5_scales1to5_n100`: `[51.308 µs 51.392 µs 51.476 µs]`
  - `signal_mse_sampen_m2_tau1_r0p5_scales1to5_n1000`: `[3.9112 ms 3.9251 ms 3.9390 ms]`
  - `signal_mse_sampen_m2_tau1_r0p5_scales1to5_n10000`: `[27.564 ms 27.670 ms 27.822 ms]`

---

## 2026-01-12T12:49:24Z — SampEn: bench all exact variants (auto vs sorted-window vs grid) (codex-cli)

Change scope:
- Expose exact SampEn variants for benchmarking:
  - `sample_entropy_chebyshev` (auto exact method selection),
  - `sample_entropy_chebyshev_exact_sorted_window` (baseline exact),
  - `sample_entropy_chebyshev_exact_grid` (grid/box hashing exact, `m<=2`).
- Extend `signal_entropy` bench to measure all three.

Command: `cargo bench -p math --bench signal_entropy`

Raw stdout: `v2/tmp_local_math_codex/signal_entropy_bench_stdout_v7_three_variants.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- SampEn (auto exact; `m=2,tau=1,r=0.5`):
  - `signal_sample_entropy_m2_tau1_r0p5_n100`: `[19.156 µs 19.182 µs 19.208 µs]`
  - `signal_sample_entropy_m2_tau1_r0p5_n1000`: `[325.84 µs 327.07 µs 328.31 µs]`
  - `signal_sample_entropy_m2_tau1_r0p5_n10000`: `[18.572 ms 18.629 ms 18.715 ms]`
- SampEn (sorted-window exact; `m=2,tau=1,r=0.5`):
  - `signal_sample_entropy_sorted_window_m2_tau1_r0p5_n100`: `[19.133 µs 19.179 µs 19.235 µs]`
  - `signal_sample_entropy_sorted_window_m2_tau1_r0p5_n1000`: `[342.48 µs 344.61 µs 346.94 µs]`
  - `signal_sample_entropy_sorted_window_m2_tau1_r0p5_n10000`: `[19.207 ms 19.245 ms 19.297 ms]`
- SampEn (grid exact; `m=2,tau=1,r=0.5`):
  - `signal_sample_entropy_grid_m2_tau1_r0p5_n100`: `[199.13 µs 200.09 µs 201.82 µs]`
  - `signal_sample_entropy_grid_m2_tau1_r0p5_n1000`: `[3.5787 ms 3.5830 ms 3.5874 ms]`
  - `signal_sample_entropy_grid_m2_tau1_r0p5_n10000`: `[178.64 ms 179.28 ms 180.33 ms]`

---

## 2026-01-12T12:56:29Z — v1.3: Zero-crossing rate + sign run-length stats (codex-cli)

Change scope:
- Add fast deterministic zero-crossing and sign-run-length measurements under `math::signal::zero_crossing`.

Command: `cargo bench -p math --bench signal_zero_crossing`

Raw stdout: `v2/tmp_local_math_codex/signal_zero_crossing_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- Zero-crossing rate (`ZeroHandling::AsZero`):
  - `signal_zero_crossing_rate_as_zero_n100`: `[153.43 ns 153.67 ns 153.91 ns]`
  - `signal_zero_crossing_rate_as_zero_n1000`: `[1.4376 µs 1.4400 µs 1.4425 µs]`
  - `signal_zero_crossing_rate_as_zero_n10000`: `[40.586 µs 40.669 µs 40.774 µs]`
- Sign run-length stats (`ZeroHandling::CarryForward`):
  - `signal_sign_run_stats_carry_forward_n100`: `[170.90 ns 171.24 ns 171.67 ns]`
  - `signal_sign_run_stats_carry_forward_n1000`: `[1.5494 µs 1.5531 µs 1.5572 µs]`
  - `signal_sign_run_stats_carry_forward_n10000`: `[44.605 µs 44.659 µs 44.713 µs]`

---

## 2026-01-12T13:18:49Z — v1.4: SSA rank-r reconstruction (codex-cli)

Change scope:
- Add `math::signal::ssa` rank-`r` SSA reconstruction (covariance eigendecomposition) + workspace API.

Command: `cargo bench -p math --bench signal_ssa`

Raw stdout: `v2/tmp_local_math_codex/signal_ssa_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- SSA workspace (`center=true`):
  - `signal_ssa_rank4_L30_n100_workspace`: `[105.46 µs 105.75 µs 106.16 µs]`
  - `signal_ssa_rank6_L80_n1000_workspace`: `[4.3589 ms 4.3647 ms 4.3706 ms]`
  - `signal_ssa_rank6_L120_n10000_workspace`: `[80.301 ms 80.382 ms 80.465 ms]`
- SSA alloc (`center=true`):
  - `signal_ssa_rank4_L30_n100_alloc`: `[104.47 µs 104.89 µs 105.41 µs]`
  - `signal_ssa_rank6_L80_n1000_alloc`: `[4.3690 ms 4.3827 ms 4.4014 ms]`
  - `signal_ssa_rank6_L120_n10000_alloc`: `[80.494 ms 80.566 ms 80.639 ms]`

---

## 2026-01-12T13:24:15Z — v1.4: SSA covariance rank-1 update optimization (codex-cli)

Change scope:
- SSA covariance construction rewritten to a single-pass rank-1 update (`cov += v v^T`) using a reusable workspace buffer.

Command: `cargo bench -p math --bench signal_ssa`

Raw stdout: `v2/tmp_local_math_codex/signal_ssa_bench_stdout_v2_cov_rank1update.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- SSA workspace (`center=true`):
  - `signal_ssa_rank4_L30_n100_workspace`: `[101.16 µs 101.45 µs 101.80 µs]`
  - `signal_ssa_rank6_L80_n1000_workspace`: `[2.8247 ms 2.8292 ms 2.8339 ms]`
  - `signal_ssa_rank6_L120_n10000_workspace`: `[40.338 ms 40.405 ms 40.480 ms]`
- SSA alloc (`center=true`):
  - `signal_ssa_rank4_L30_n100_alloc`: `[98.685 µs 99.010 µs 99.389 µs]`
  - `signal_ssa_rank6_L80_n1000_alloc`: `[2.8659 ms 2.8729 ms 2.8807 ms]`
  - `signal_ssa_rank6_L120_n10000_alloc`: `[40.518 ms 40.582 ms 40.660 ms]`

---

## 2026-01-12T13:32:35Z — v1.4: Lomb–Scargle periodogram (codex-cli)

Change scope:
- Add `math::signal::spectral::lomb_scargle` for irregular sampling (deterministic, time-shift formulation).

Command: `cargo bench -p math --bench signal_lomb_scargle`

Raw stdout: `v2/tmp_local_math_codex/signal_lomb_scargle_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`; frequency grid `m=64`):
- Lomb–Scargle workspace (`center=true`, normalization by variance):
  - `signal_lomb_scargle_n100_m64_workspace`: `[258.32 µs 259.11 µs 260.03 µs]`
  - `signal_lomb_scargle_n1000_m64_workspace`: `[2.4129 ms 2.4171 ms 2.4211 ms]`
  - `signal_lomb_scargle_n10000_m64_workspace`: `[23.851 ms 23.928 ms 24.019 ms]`
- Lomb–Scargle alloc (`center=true`, normalization by variance):
  - `signal_lomb_scargle_n100_m64_alloc`: `[257.71 µs 258.60 µs 259.96 µs]`
  - `signal_lomb_scargle_n1000_m64_alloc`: `[2.4130 ms 2.4156 ms 2.4185 ms]`
  - `signal_lomb_scargle_n10000_m64_alloc`: `[23.715 ms 23.761 ms 23.810 ms]`

---

## 2026-01-12T13:41:06Z — v1.4: Phase locking value (PLV) primitives (codex-cli)

Change scope:
- Add `math::signal::spectral::phase_coherence` (PLV from phases; PLV from signals via Hilbert-phase).

Command: `cargo bench -p math --bench signal_phase_coherence`

Raw stdout: `v2/tmp_local_math_codex/signal_phase_coherence_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- PLV from phases:
  - `signal_plv_from_phase_n100`: `[1.1545 µs 1.1603 µs 1.1657 µs]`
  - `signal_plv_from_phase_n1000`: `[12.794 µs 12.812 µs 12.831 µs]`
  - `signal_plv_from_phase_n10000`: `[166.26 µs 167.20 µs 168.40 µs]`
- PLV from signals (Hilbert, workspace reuse):
  - `signal_plv_from_signals_n100_workspace`: `[7.7355 µs 7.7752 µs 7.8235 µs]`
  - `signal_plv_from_signals_n1000_workspace`: `[70.240 µs 70.413 µs 70.611 µs]`
  - `signal_plv_from_signals_n10000_workspace`: `[778.05 µs 781.54 µs 785.66 µs]`

---

## 2026-01-12T13:47:08Z — v1.4: MODWT wavelet coherence (codex-cli)

Change scope:
- Add `math::signal::wavelet_coherence` (MODWT-detail-based real wavelet coherence with deterministic smoothing).

Command: `cargo bench -p math --bench signal_wavelet_coherence`

Raw stdout: `v2/tmp_local_math_codex/signal_wavelet_coherence_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`; family=D4, level=2, smooth_window=21):
- `signal_wavelet_coherence_modwt_level2_n100`: `[28.097 µs 28.239 µs 28.395 µs]`
- `signal_wavelet_coherence_modwt_level2_n1000`: `[2.8168 ms 2.8240 ms 2.8323 ms]`
- `signal_wavelet_coherence_modwt_level2_n10000`: `[281.56 ms 282.75 ms 284.18 ms]`

---

## 2026-01-12T13:51:20Z — v1.4: MODWT wavelet coherence O(n) smoothing fix (codex-cli)

Change scope:
- Fix wavelet coherence smoothing implementation from O(n^2) to O(n) by computing prefix sums once per moment series.

Command: `cargo bench -p math --bench signal_wavelet_coherence`

Raw stdout: `v2/tmp_local_math_codex/signal_wavelet_coherence_bench_stdout_v2_on_prefix.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`; family=D4, level=2, smooth_window=21):
- `signal_wavelet_coherence_modwt_level2_n100`: `[4.6728 µs 4.6784 µs 4.6841 µs]`
- `signal_wavelet_coherence_modwt_level2_n1000`: `[45.346 µs 45.444 µs 45.547 µs]`
- `signal_wavelet_coherence_modwt_level2_n10000`: `[457.82 µs 458.77 µs 460.20 µs]`

---

## 2026-01-12T14:01:07Z — v1.4: Signal shape measures (codex-cli)

Change scope:
- Add `math::signal::shape` (Hjorth, Teager–Kaiser energy, spectral flatness/crest/entropy reducers).

Command: `cargo bench -p math --bench signal_shape`

Raw stdout: `v2/tmp_local_math_codex/signal_shape_bench_stdout.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- Hjorth:
  - `signal_hjorth_n100`: `[241.15 ns 241.46 ns 241.81 ns]`
  - `signal_hjorth_n1000`: `[2.1607 µs 2.1645 µs 2.1688 µs]`
  - `signal_hjorth_n10000`: `[21.662 µs 21.740 µs 21.851 µs]`
- Teager–Kaiser mean:
  - `signal_tkeo_mean_n100`: `[131.16 ns 131.57 ns 132.12 ns]`
  - `signal_tkeo_mean_n1000`: `[1.2247 µs 1.2267 µs 1.2285 µs]`
  - `signal_tkeo_mean_n10000`: `[12.482 µs 12.497 µs 12.512 µs]`
- Spectral flatness (from periodogram-like `p`):
  - `signal_spectral_flatness_from_p_n100`: `[588.39 ns 589.66 ns 591.09 ns]`
  - `signal_spectral_flatness_from_p_n1000`: `[5.5753 µs 5.6495 µs 5.7826 µs]`
  - `signal_spectral_flatness_from_p_n10000`: `[56.522 µs 56.598 µs 56.674 µs]`
- Spectral crest factor (from `p`):
  - `signal_spectral_crest_from_p_n100`: `[217.28 ns 217.53 ns 217.77 ns]`
  - `signal_spectral_crest_from_p_n1000`: `[2.0718 µs 2.0754 µs 2.0798 µs]`
  - `signal_spectral_crest_from_p_n10000`: `[20.986 µs 21.106 µs 21.266 µs]`
- Spectral entropy (from `p`, normalized):
  - `signal_spectral_entropy_from_p_n100`: `[791.96 ns 795.55 ns 800.51 ns]`
  - `signal_spectral_entropy_from_p_n1000`: `[7.5141 µs 7.5237 µs 7.5335 µs]`
  - `signal_spectral_entropy_from_p_n10000`: `[75.413 µs 75.511 µs 75.615 µs]`

---

## 2026-01-13T17:29:07Z — spectral coherence frequency smoothing (codex-cli)

Change scope:
- Fix `signal::spectral::coherence` degeneracy by smoothing auto/cross spectra across frequency bins (single FFT + O(n) prefix sums).

Environment:
- OS: `Linux tia 5.15.0-156-generic x86_64`
- CPU: `Intel(R) Xeon(R) W-2295 CPU @ 3.00GHz`
- Toolchain: `rustc 1.90.0`, `cargo 1.90.0`
- Profile: `bench` (optimized, Criterion)

Command: `cargo bench --manifest-path v2/Cargo.toml -p math --bench signal_spectral`

Raw stdout: `v2/tmp_local_math_codex/signal_spectral_bench_stdout_coherence_smoothing_20260113T172907Z.txt`

Key results (standard sizes `n ∈ {100, 1000, 10000}`):
- `signal_periodogram_ws_n100`: `[1.1545 µs 1.1581 µs 1.1632 µs]`
- `signal_autocorrelation_fft_ws_n100_lag10`: `[1.7767 µs 1.7800 µs 1.7838 µs]`
- `signal_coherence_ws_n100`: `[3.2649 µs 3.2724 µs 3.2818 µs]`
- `signal_stft_periodograms_flat_ws_n100_w64`: `[1.2756 µs 1.2818 µs 1.2899 µs]`
- `signal_periodogram_ws_n1000`: `[11.129 µs 11.259 µs 11.419 µs]`
- `signal_autocorrelation_fft_ws_n1000_lag10`: `[19.014 µs 19.604 µs 20.349 µs]`
- `signal_coherence_ws_n1000`: `[31.671 µs 32.790 µs 34.219 µs]`
- `signal_stft_periodograms_flat_ws_n1000_w64`: `[6.6033 µs 6.8375 µs 7.1321 µs]`
- `signal_periodogram_ws_n10000`: `[120.22 µs 123.27 µs 127.32 µs]`
- `signal_autocorrelation_fft_ws_n10000_lag10`: `[497.58 µs 504.50 µs 513.58 µs]`
- `signal_coherence_ws_n10000`: `[344.32 µs 344.54 µs 344.76 µs]`
- `signal_stft_periodograms_flat_ws_n10000_w64`: `[11.535 µs 11.687 µs 11.894 µs]`
