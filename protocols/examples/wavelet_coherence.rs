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

//! Wavelet cross-spectrum and coherence primitives (MODWT-based; deterministic).
//!
//! This is not a full CWT/Morlet wavelet coherence pipeline. It provides a bounded,
//! deterministic multiscale dependence measure using MODWT detail coefficients:
//!
//! - Compute detail coefficients `w_x(t), w_y(t)` at a given level/scale.
//! - Define local smoothed moments over a centered window:
//!     S_xy(t) = mean(w_x * w_y), S_xx(t) = mean(w_x^2), S_yy(t) = mean(w_y^2)
//! - Define (real) wavelet coherence:
//!     C(t) = S_xy(t)^2 / (S_xx(t) * S_yy(t))    (if denom>0 else 0)
//!
//! This yields `C(t) ∈ [0,1]` and can be averaged over time to obtain a scalar per scale.

use crate::signal::types::WaveletFamily;
use crate::signal::wavelets::{modwt_detail_level_into_with_workspace, ModwtD4Workspace};
use crate::{MathError, MathResult};

#[derive(Debug, Clone, Copy)]
pub struct WaveletCoherenceConfig {
    pub family: WaveletFamily,
    /// MODWT detail level (1-indexed).
    pub level: usize,
    /// Centered smoothing window length in samples (must be >= 1).
    pub smooth_window: usize,
}

impl Default for WaveletCoherenceConfig {
    fn default() -> Self {
        Self {
            family: WaveletFamily::ModwtD4,
            level: 1,
            smooth_window: 9,
        }
    }
}

#[derive(Debug, Default)]
pub struct WaveletCoherenceWorkspace {
    modwt: ModwtD4Workspace,
    wx: Vec<f64>,
    wy: Vec<f64>,
    tmp_xy: Vec<f64>,
    tmp_xx: Vec<f64>,
    tmp_yy: Vec<f64>,
    prefix_xy: Vec<f64>,
    prefix_xx: Vec<f64>,
    prefix_yy: Vec<f64>,
}

impl WaveletCoherenceWorkspace {
    pub fn with_capacity(n: usize) -> MathResult<Self> {
        Ok(Self {
            modwt: ModwtD4Workspace::with_capacity(n)?,
            wx: Vec::with_capacity(n),
            wy: Vec::with_capacity(n),
            tmp_xy: Vec::with_capacity(n),
            tmp_xx: Vec::with_capacity(n),
            tmp_yy: Vec::with_capacity(n),
            prefix_xy: Vec::with_capacity(n + 1),
            prefix_xx: Vec::with_capacity(n + 1),
            prefix_yy: Vec::with_capacity(n + 1),
        })
    }

    fn prepare(&mut self, n: usize) -> MathResult<()> {
        self.modwt.prepare(n)?;
        self.wx.clear();
        self.wx.resize(n, 0.0);
        self.wy.clear();
        self.wy.resize(n, 0.0);
        self.tmp_xy.clear();
        self.tmp_xy.resize(n, 0.0);
        self.tmp_xx.clear();
        self.tmp_xx.resize(n, 0.0);
        self.tmp_yy.clear();
        self.tmp_yy.resize(n, 0.0);
        self.prefix_xy.clear();
        self.prefix_xy.resize(n + 1, 0.0);
        self.prefix_xx.clear();
        self.prefix_xx.resize(n + 1, 0.0);
        self.prefix_yy.clear();
        self.prefix_yy.resize(n + 1, 0.0);
        Ok(())
    }
}

pub fn wavelet_coherence_modwt_level_mean(
    x: &[f64],
    y: &[f64],
    cfg: &WaveletCoherenceConfig,
) -> MathResult<f64> {
    let mut ws = WaveletCoherenceWorkspace::with_capacity(x.len().max(y.len()))?;
    wavelet_coherence_modwt_level_mean_with_workspace(x, y, cfg, &mut ws)
}

pub fn wavelet_coherence_modwt_level_mean_with_workspace(
    x: &[f64],
    y: &[f64],
    cfg: &WaveletCoherenceConfig,
    workspace: &mut WaveletCoherenceWorkspace,
) -> MathResult<f64> {
    validate_inputs(x, y, cfg)?;
    let n = x.len();
    workspace.prepare(n)?;

    modwt_detail_level_into_with_workspace(
        x,
        cfg.family,
        cfg.level,
        &mut workspace.wx,
        &mut workspace.modwt,
    )?;
    modwt_detail_level_into_with_workspace(
        y,
        cfg.family,
        cfg.level,
        &mut workspace.wy,
        &mut workspace.modwt,
    )?;

    for i in 0..n {
        let wx = workspace.wx[i];
        let wy = workspace.wy[i];
        workspace.tmp_xy[i] = wx * wy;
        workspace.tmp_xx[i] = wx * wx;
        workspace.tmp_yy[i] = wy * wy;
    }

    build_prefix_sums(&workspace.tmp_xy, &mut workspace.prefix_xy);
    build_prefix_sums(&workspace.tmp_xx, &mut workspace.prefix_xx);
    build_prefix_sums(&workspace.tmp_yy, &mut workspace.prefix_yy);

    let mut sum_c = 0.0f64;
    let mut count_c = 0usize;
    for i in 0..n {
        let sxy = moving_mean_from_prefix(&workspace.prefix_xy, cfg.smooth_window, i);
        let sxx = moving_mean_from_prefix(&workspace.prefix_xx, cfg.smooth_window, i);
        let syy = moving_mean_from_prefix(&workspace.prefix_yy, cfg.smooth_window, i);
        let denom = sxx * syy;
        if !(denom.is_finite() && denom > 0.0) {
            continue;
        }
        let c = (sxy * sxy) / denom;
        if !c.is_finite() {
            return Err(MathError::NumericalError {
                reason: "wavelet_coherence: non-finite coherence".to_string(),
                operation: Some("wavelet_coherence_modwt_level_mean".to_string()),
            });
        }
        sum_c += c;
        count_c += 1;
    }

    if count_c == 0 {
        return Ok(0.0);
    }
    let mean = sum_c / (count_c as f64);
    Ok(mean.clamp(0.0, 1.0))
}

pub fn wavelet_coherence_modwt_level_series_into_with_workspace(
    x: &[f64],
    y: &[f64],
    cfg: &WaveletCoherenceConfig,
    out: &mut [f64],
    workspace: &mut WaveletCoherenceWorkspace,
) -> MathResult<()> {
    validate_inputs(x, y, cfg)?;
    let n = x.len();
    if out.len() != n {
        return Err(MathError::InvalidParameter {
            parameter: "out".to_string(),
            value: out.len() as f64,
            constraint: format!("must have length n={n}"),
        });
    }
    workspace.prepare(n)?;

    modwt_detail_level_into_with_workspace(
        x,
        cfg.family,
        cfg.level,
        &mut workspace.wx,
        &mut workspace.modwt,
    )?;
    modwt_detail_level_into_with_workspace(
        y,
        cfg.family,
        cfg.level,
        &mut workspace.wy,
        &mut workspace.modwt,
    )?;

    for i in 0..n {
        let wx = workspace.wx[i];
        let wy = workspace.wy[i];
        workspace.tmp_xy[i] = wx * wy;
        workspace.tmp_xx[i] = wx * wx;
        workspace.tmp_yy[i] = wy * wy;
    }

    build_prefix_sums(&workspace.tmp_xy, &mut workspace.prefix_xy);
    build_prefix_sums(&workspace.tmp_xx, &mut workspace.prefix_xx);
    build_prefix_sums(&workspace.tmp_yy, &mut workspace.prefix_yy);

    for i in 0..n {
        let sxy = moving_mean_from_prefix(&workspace.prefix_xy, cfg.smooth_window, i);
        let sxx = moving_mean_from_prefix(&workspace.prefix_xx, cfg.smooth_window, i);
        let syy = moving_mean_from_prefix(&workspace.prefix_yy, cfg.smooth_window, i);
        let denom = sxx * syy;
        if !(denom.is_finite() && denom > 0.0) {
            out[i] = 0.0;
            continue;
        }
        let c = (sxy * sxy) / denom;
        if !(c.is_finite() && c >= 0.0) {
            return Err(MathError::NumericalError {
                reason: "wavelet_coherence: non-finite coherence".to_string(),
                operation: Some("wavelet_coherence_modwt_level_series".to_string()),
            });
        }
        out[i] = c.min(1.0);
    }
    Ok(())
}

fn build_prefix_sums(values: &[f64], prefix: &mut [f64]) {
    prefix[0] = 0.0;
    for (i, &v) in values.iter().enumerate() {
        prefix[i + 1] = prefix[i] + v;
    }
}

fn moving_mean_from_prefix(prefix: &[f64], window: usize, idx: usize) -> f64 {
    let n = prefix.len() - 1;
    let half = window / 2;
    let start = idx.saturating_sub(half);
    let end = (idx + half).min(n - 1);
    let sum = prefix[end + 1] - prefix[start];
    sum / ((end + 1 - start) as f64)
}

fn validate_inputs(x: &[f64], y: &[f64], cfg: &WaveletCoherenceConfig) -> MathResult<()> {
    if x.len() != y.len() {
        return Err(MathError::InvalidData(format!(
            "wavelet_coherence: length mismatch (x={}, y={})",
            x.len(),
            y.len()
        )));
    }
    if x.is_empty() {
        return Err(MathError::InsufficientDataAlgo {
            required: 1,
            actual: 0,
        });
    }
    if x.iter().any(|v| !v.is_finite()) || y.iter().any(|v| !v.is_finite()) {
        return Err(MathError::InvalidData(
            "wavelet_coherence: all inputs must be finite".to_string(),
        ));
    }
    if cfg.level == 0 {
        return Err(MathError::InvalidParameter {
            parameter: "level".to_string(),
            value: 0.0,
            constraint: "must be >= 1".to_string(),
        });
    }
    if cfg.smooth_window == 0 {
        return Err(MathError::InvalidParameter {
            parameter: "smooth_window".to_string(),
            value: 0.0,
            constraint: "must be >= 1".to_string(),
        });
    }
    if cfg.smooth_window > x.len() {
        return Err(MathError::InvalidParameter {
            parameter: "smooth_window".to_string(),
            value: cfg.smooth_window as f64,
            constraint: format!("must be <= n={}", x.len()),
        });
    }
    Ok(())
}
