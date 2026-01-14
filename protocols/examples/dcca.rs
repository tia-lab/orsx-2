// ============================================================================
// MATHILDE PROPRIETARY AND CONFIDENTIAL
// Copyright (c) 2024 MATHILDE. All Rights Reserved.
//
// This source code contains trade secrets and confidential information owned
// exclusively by MATHILDE, protected under Swiss law:
//
// - URG Art. 2(3), 10(3): Computer program copyright protection
// - URG Art. 24: Reverse engineering/decompilation restricted
// - UWG Art. 5-6: Trade secret and confidential information protection
// - StGB Art. 143bis: Unauthorized data access (criminal)
// - StGB Art. 162: Trade secret violation (criminal)
//
// PROHIBITED: Reproduction, copying, modification, distribution, disclosure,
// reverse engineering, decompilation, or derivative works without prior
// written authorization from MATHILDE.
//
// ACCESS REQUIREMENT: Executed NDA with MATHILDE required. Unauthorized
// access or possession violates Swiss law and international treaties.
//
// ALGORITHMS: Mathematical methods and parameters in this file constitute
// trade secrets independent of copyright protection.
//
// Legal Contact: massimo.nicora@wnlegal.ch
// ============================================================================

//! Detrended cross-correlation analysis (DCCA) primitives (measurement-only).
//!
//! This module provides deterministic, time-bounded building blocks:
//! - profile integration (mean-centered cumulative sum),
//! - per-scale detrended covariance/variance over forward+backward windows,
//! - DCCA correlation coefficient `rho_DCCA(scale)` (no hypothesis testing).

use crate::{MathError, MathResult};

#[derive(Debug, Default, Clone)]
pub struct DccaWorkspace {
    x_profile: Vec<f64>,
    y_profile: Vec<f64>,
    // Prefix sums over profiles, length n+1 (p[0]=0).
    px: Vec<f64>,
    pkx: Vec<f64>,
    px2: Vec<f64>,
    py: Vec<f64>,
    pky: Vec<f64>,
    py2: Vec<f64>,
    pxy: Vec<f64>,
}

impl DccaWorkspace {
    pub fn with_capacity(n: usize) -> Self {
        Self {
            x_profile: Vec::with_capacity(n),
            y_profile: Vec::with_capacity(n),
            px: Vec::with_capacity(n + 1),
            pkx: Vec::with_capacity(n + 1),
            px2: Vec::with_capacity(n + 1),
            py: Vec::with_capacity(n + 1),
            pky: Vec::with_capacity(n + 1),
            py2: Vec::with_capacity(n + 1),
            pxy: Vec::with_capacity(n + 1),
        }
    }

    fn prepare(&mut self, n: usize) {
        self.x_profile.clear();
        self.y_profile.clear();
        self.x_profile.resize(n, 0.0);
        self.y_profile.resize(n, 0.0);

        let n1 = n + 1;
        self.px.clear();
        self.pkx.clear();
        self.px2.clear();
        self.py.clear();
        self.pky.clear();
        self.py2.clear();
        self.pxy.clear();
        self.px.resize(n1, 0.0);
        self.pkx.resize(n1, 0.0);
        self.px2.resize(n1, 0.0);
        self.py.resize(n1, 0.0);
        self.pky.resize(n1, 0.0);
        self.py2.resize(n1, 0.0);
        self.pxy.resize(n1, 0.0);
    }
}

fn validate_inputs(x: &[f64], y: &[f64]) -> MathResult<()> {
    if x.len() != y.len() {
        return Err(MathError::InvalidParameter {
            parameter: "y".to_string(),
            value: y.len() as f64,
            constraint: format!("must have length n={}", x.len()),
        });
    }
    if x.len() < 4 {
        return Err(MathError::InsufficientDataAlgo {
            required: 4,
            actual: x.len(),
        });
    }
    if x.iter().any(|v| !v.is_finite()) || y.iter().any(|v| !v.is_finite()) {
        return Err(MathError::InvalidData(
            "dcca: all values must be finite".to_string(),
        ));
    }
    Ok(())
}

fn validate_scales(scales: &[usize]) -> MathResult<()> {
    if scales.is_empty() {
        return Err(MathError::InvalidParameter {
            parameter: "scales".to_string(),
            value: 0.0,
            constraint: "must be non-empty".to_string(),
        });
    }
    for &s in scales {
        if s < 4 {
            return Err(MathError::InvalidParameter {
                parameter: "scale".to_string(),
                value: s as f64,
                constraint:
                    "scale must be >= 4 (needs >=3 points per window; we keep a stricter bound)"
                        .to_string(),
            });
        }
    }
    Ok(())
}

fn integrate_profile_into(values: &[f64], out: &mut [f64]) -> MathResult<()> {
    let n = values.len();
    let mean = values.iter().sum::<f64>() / (n as f64);
    if !mean.is_finite() {
        return Err(MathError::NumericalError {
            reason: "dcca: non-finite mean".to_string(),
            operation: Some("integrate_profile_into".to_string()),
        });
    }
    let mut acc = 0.0f64;
    for i in 0..n {
        acc += values[i] - mean;
        if !acc.is_finite() {
            return Err(MathError::NumericalInstability(
                "dcca: profile integration became non-finite".to_string(),
            ));
        }
        out[i] = acc;
    }
    Ok(())
}

fn fill_prefix_sums(
    x: &[f64],
    y: &[f64],
    px: &mut [f64],
    pkx: &mut [f64],
    px2: &mut [f64],
    py: &mut [f64],
    pky: &mut [f64],
    py2: &mut [f64],
    pxy: &mut [f64],
) -> MathResult<()> {
    let n = x.len();
    debug_assert!(y.len() == n);
    debug_assert!(px.len() == n + 1);
    px[0] = 0.0;
    pkx[0] = 0.0;
    px2[0] = 0.0;
    py[0] = 0.0;
    pky[0] = 0.0;
    py2[0] = 0.0;
    pxy[0] = 0.0;

    let mut sx = 0.0f64;
    let mut skx = 0.0f64;
    let mut sx2 = 0.0f64;
    let mut sy = 0.0f64;
    let mut sky = 0.0f64;
    let mut sy2 = 0.0f64;
    let mut sxy = 0.0f64;
    for i in 0..n {
        let k = i as f64;
        let xi = x[i];
        let yi = y[i];
        sx += xi;
        skx += k * xi;
        sx2 += xi * xi;
        sy += yi;
        sky += k * yi;
        sy2 += yi * yi;
        sxy += xi * yi;
        if !(sx.is_finite()
            && skx.is_finite()
            && sx2.is_finite()
            && sy.is_finite()
            && sky.is_finite()
            && sy2.is_finite()
            && sxy.is_finite())
        {
            return Err(MathError::NumericalInstability(
                "dcca: non-finite prefix accumulation".to_string(),
            ));
        }
        px[i + 1] = sx;
        pkx[i + 1] = skx;
        px2[i + 1] = sx2;
        py[i + 1] = sy;
        pky[i + 1] = sky;
        py2[i + 1] = sy2;
        pxy[i + 1] = sxy;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct WindowSums {
    sum_x: f64,
    sum_ix: f64,
    sum_x2: f64,
    sum_y: f64,
    sum_iy: f64,
    sum_y2: f64,
    sum_xy: f64,
}

#[inline]
fn window_sums(start: usize, end: usize, ws: &DccaWorkspace) -> WindowSums {
    let sum_x = ws.px[end] - ws.px[start];
    let sum_kx = ws.pkx[end] - ws.pkx[start];
    let sum_x2 = ws.px2[end] - ws.px2[start];

    let sum_y = ws.py[end] - ws.py[start];
    let sum_ky = ws.pky[end] - ws.pky[start];
    let sum_y2 = ws.py2[end] - ws.py2[start];

    let sum_xy = ws.pxy[end] - ws.pxy[start];

    let start_f = start as f64;
    WindowSums {
        sum_x,
        sum_ix: sum_kx - start_f * sum_x,
        sum_x2,
        sum_y,
        sum_iy: sum_ky - start_f * sum_y,
        sum_y2,
        sum_xy,
    }
}

#[inline]
fn detrended_stats_linear_from_sums(s: usize, sums: WindowSums) -> MathResult<(f64, f64, f64)> {
    // Fit separate OLS lines (intercept + slope) to X and Y over i=0..s-1, then compute mean(rx^2), mean(ry^2), mean(rx*ry).
    //
    // All required sums are in `sums` for local indices i.
    let s_f = s as f64;
    let x_idx_mean = (s_f - 1.0) / 2.0;

    let x_mean = sums.sum_x / s_f;
    let y_mean = sums.sum_y / s_f;
    if !(x_mean.is_finite() && y_mean.is_finite()) {
        return Err(MathError::NumericalError {
            reason: "dcca: non-finite window mean".to_string(),
            operation: Some("detrended_stats_linear_from_sums".to_string()),
        });
    }

    // Σ (i-x̄)^2 = s*(s^2-1)/12.
    let sxx = s_f * (s_f * s_f - 1.0) / 12.0;
    if !(sxx.is_finite() && sxx > 0.0) {
        return Err(MathError::NumericalInstability(
            "dcca: non-finite or degenerate sxx".to_string(),
        ));
    }

    let sxy_x = sums.sum_ix - x_idx_mean * sums.sum_x;
    let sxy_y = sums.sum_iy - x_idx_mean * sums.sum_y;
    if !(sxy_x.is_finite() && sxy_y.is_finite()) {
        return Err(MathError::NumericalInstability(
            "dcca: non-finite sxy".to_string(),
        ));
    }

    let slope_x = sxy_x / sxx;
    let intercept_x = x_mean - slope_x * x_idx_mean;
    let slope_y = sxy_y / sxx;
    let intercept_y = y_mean - slope_y * x_idx_mean;
    if !(slope_x.is_finite()
        && intercept_x.is_finite()
        && slope_y.is_finite()
        && intercept_y.is_finite())
    {
        return Err(MathError::NumericalError {
            reason: "dcca: non-finite regression coefficients".to_string(),
            operation: Some("detrended_stats_linear_from_sums".to_string()),
        });
    }

    let sum_i = s_f * (s_f - 1.0) * 0.5;
    let sum_i2 = (s_f - 1.0) * s_f * (2.0 * s_f - 1.0) / 6.0;

    // Σ rx^2 = Σ x^2 - 2a Σ x - 2b Σ i x + a^2 s + 2ab Σ i + b^2 Σ i^2.
    let srx2 = sums.sum_x2 - 2.0 * intercept_x * sums.sum_x - 2.0 * slope_x * sums.sum_ix
        + intercept_x * intercept_x * s_f
        + 2.0 * intercept_x * slope_x * sum_i
        + slope_x * slope_x * sum_i2;

    let sry2 = sums.sum_y2 - 2.0 * intercept_y * sums.sum_y - 2.0 * slope_y * sums.sum_iy
        + intercept_y * intercept_y * s_f
        + 2.0 * intercept_y * slope_y * sum_i
        + slope_y * slope_y * sum_i2;

    // Σ rx*ry = Σ x*y - a_x Σ y - b_x Σ i y - a_y Σ x - b_y Σ i x
    //          + a_x a_y s + (a_x b_y + a_y b_x) Σ i + b_x b_y Σ i^2.
    let srxry = sums.sum_xy
        - intercept_x * sums.sum_y
        - slope_x * sums.sum_iy
        - intercept_y * sums.sum_x
        - slope_y * sums.sum_ix
        + intercept_x * intercept_y * s_f
        + (intercept_x * slope_y + intercept_y * slope_x) * sum_i
        + slope_x * slope_y * sum_i2;

    if !(srx2.is_finite() && sry2.is_finite() && srxry.is_finite()) {
        return Err(MathError::NumericalInstability(
            "dcca: non-finite residual sums".to_string(),
        ));
    }

    let inv_s = 1.0 / s_f;
    Ok((srx2 * inv_s, sry2 * inv_s, srxry * inv_s))
}

/// Compute DCCA correlation coefficient `rho_DCCA(scale)` for each `scale` (window length).
///
/// Steps:
/// 1) integrate profiles `X` and `Y` (mean-centered cumulative sums),
/// 2) for each scale `s`, split into `Ns = floor(n/s)` windows, use both forward and backward windows (2*Ns),
/// 3) detrend each window with separate linear fits and compute detrended covariance/variances,
/// 4) `rho = F_xy(s) / sqrt(F_x(s) * F_y(s))`.
///
/// Returns `Err` if any `scale` implies `Ns < 2` (too little averaging) or if variances collapse to zero.
pub fn dcca_rho_into_with_workspace(
    x: &[f64],
    y: &[f64],
    scales: &[usize],
    out_rho: &mut [f64],
    workspace: &mut DccaWorkspace,
) -> MathResult<()> {
    validate_inputs(x, y)?;
    validate_scales(scales)?;

    if out_rho.len() != scales.len() {
        return Err(MathError::InvalidParameter {
            parameter: "out_rho".to_string(),
            value: out_rho.len() as f64,
            constraint: format!("must have length scales.len()={}", scales.len()),
        });
    }

    let n = x.len();
    workspace.prepare(n);
    integrate_profile_into(x, &mut workspace.x_profile)?;
    integrate_profile_into(y, &mut workspace.y_profile)?;
    fill_prefix_sums(
        &workspace.x_profile,
        &workspace.y_profile,
        &mut workspace.px,
        &mut workspace.pkx,
        &mut workspace.px2,
        &mut workspace.py,
        &mut workspace.pky,
        &mut workspace.py2,
        &mut workspace.pxy,
    )?;

    for (idx, &s) in scales.iter().enumerate() {
        let ns = n / s;
        if ns < 2 {
            return Err(MathError::InvalidParameter {
                parameter: "scale".to_string(),
                value: s as f64,
                constraint: format!("scale too large for n={n}: requires floor(n/scale) >= 2"),
            });
        }

        let mut fx = 0.0f64;
        let mut fy = 0.0f64;
        let mut fxy = 0.0f64;
        let mut windows = 0u64;

        // forward windows
        for w in 0..ns {
            let start = w * s;
            let end = start + s;
            let sums = window_sums(start, end, workspace);
            let (vx, vy, vxy) = detrended_stats_linear_from_sums(s, sums)?;
            fx += vx;
            fy += vy;
            fxy += vxy;
            windows += 1;
        }
        // backward windows
        for w in 0..ns {
            let end = n - w * s;
            let start = end - s;
            let sums = window_sums(start, end, workspace);
            let (vx, vy, vxy) = detrended_stats_linear_from_sums(s, sums)?;
            fx += vx;
            fy += vy;
            fxy += vxy;
            windows += 1;
        }

        let inv_w = 1.0 / (windows as f64);
        fx *= inv_w;
        fy *= inv_w;
        fxy *= inv_w;
        if !(fx.is_finite() && fy.is_finite() && fxy.is_finite()) {
            return Err(MathError::NumericalInstability(
                "dcca: non-finite fluctuation values".to_string(),
            ));
        }
        if fx <= 0.0 || fy <= 0.0 {
            return Err(MathError::CalculationError(
                "dcca: zero or negative detrended variance".to_string(),
            ));
        }
        let denom = (fx * fy).sqrt();
        if !(denom.is_finite() && denom > 0.0) {
            return Err(MathError::NumericalError {
                reason: "dcca: invalid normalization denominator".to_string(),
                operation: Some("dcca_rho_into_with_workspace".to_string()),
            });
        }
        let rho = fxy / denom;
        if !rho.is_finite() {
            return Err(MathError::NumericalError {
                reason: "dcca: non-finite rho".to_string(),
                operation: Some("dcca_rho_into_with_workspace".to_string()),
            });
        }
        out_rho[idx] = rho;
    }
    Ok(())
}

pub fn dcca_rho(x: &[f64], y: &[f64], scales: &[usize]) -> MathResult<Vec<f64>> {
    let mut out = vec![0.0f64; scales.len()];
    let mut ws = DccaWorkspace::with_capacity(x.len());
    dcca_rho_into_with_workspace(x, y, scales, &mut out, &mut ws)?;
    Ok(out)
}
