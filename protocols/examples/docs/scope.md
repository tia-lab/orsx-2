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

# `math::signal` — scope (DRAFT)

## In scope

- Deterministic signal/time-series transforms and estimators (filters, detrending, wavelets, Kalman recursions).
- Algorithms whose meaning is signal-processing (not econometric hypothesis tests).
- Measurement-only primitives intended for reuse by 2+ consumers (no domain heuristics).

## Planned additions (v1.4)

These are in-scope for `math::signal` if they can be implemented with:
- deterministic defaults,
- explicit failure contracts (`MathResult`, no silent NaNs),
- workspace / allocation-disciplined APIs where relevant,
- benchmark coverage and append-only bench logs.

Candidates:
- SSA (Singular Spectrum Analysis) decomposition primitives (bounded rank, deterministic SVD strategy): — DONE
- Lomb–Scargle periodogram (for irregular sampling / missing observations): — DONE
- Phase-derived dependence measures based on Hilbert phase (e.g. PLV / phase coherence): — DONE
- Wavelet cross-spectrum / wavelet coherence (multiscale dependency primitives, no hypothesis tests): — DONE
- Small deterministic “signal shape” measures (Hjorth parameters, Teager–Kaiser energy, spectral flatness/crest factor/spectral entropy): — DONE

## Out of scope

- RNG-driven simulation or resampling.
- Finance-specific OHLC estimators and market conventions.
- Any non-deterministic parallel reductions in default APIs.
- Online change-point detection policies and Bayesian inference (e.g. CUSUM/Page–Hinkley/BOCPD) → separate `math::changepoint`/`math::detection`.
- Hypothesis testing / p-values / significance pipelines → `math::econometrics`.
