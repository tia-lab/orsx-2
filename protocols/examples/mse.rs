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

//! Multiscale entropy (MSE) primitives (bounded, deterministic).
//!
//! This module provides:
//! - coarse-graining by averaging non-overlapping blocks,
//! - per-scale sample entropy (SampEn) evaluation using `signal::entropy`.
//!
//! Notes:
//! - No hypothesis tests or significance assessment are provided here.
//! - All APIs are deterministic and bounded by explicit scale caps and workspace capacity.

use crate::signal::entropy::{sample_entropy_chebyshev, SampleEntropyWorkspace};
use crate::{MathError, MathResult};

#[derive(Debug, Default, Clone)]
pub struct MseWorkspace {
    coarse: Vec<f64>,
    sampen: SampleEntropyWorkspace,
}

impl MseWorkspace {
    pub fn with_capacity(n: usize) -> Self {
        Self {
            coarse: Vec::with_capacity(n),
            sampen: SampleEntropyWorkspace::with_capacity(n),
        }
    }

    fn ensure_coarse_len(&mut self, len: usize) {
        self.coarse.clear();
        self.coarse.resize(len, 0.0);
    }

    pub fn sample_entropy_workspace_mut(&mut self) -> &mut SampleEntropyWorkspace {
        &mut self.sampen
    }
}

fn validate_finite(data: &[f64]) -> MathResult<()> {
    if data.is_empty() {
        return Err(MathError::InsufficientDataAlgo {
            required: 1,
            actual: 0,
        });
    }
    if data.iter().any(|v| !v.is_finite()) {
        return Err(MathError::InvalidData(
            "mse: all values must be finite".to_string(),
        ));
    }
    Ok(())
}

/// Coarse-grain by averaging non-overlapping blocks of length `scale`.
///
/// Output length is `floor(n/scale)`.
pub fn coarse_grain_mean_into(data: &[f64], scale: usize, out: &mut [f64]) -> MathResult<()> {
    validate_finite(data)?;
    if scale < 1 {
        return Err(MathError::InvalidParameter {
            parameter: "scale".to_string(),
            value: scale as f64,
            constraint: "scale must be >= 1".to_string(),
        });
    }
    let n = data.len();
    let m = n / scale;
    if out.len() != m {
        return Err(MathError::InvalidParameter {
            parameter: "out".to_string(),
            value: out.len() as f64,
            constraint: format!("must have length floor(n/scale)={m} for n={n}, scale={scale}"),
        });
    }
    for j in 0..m {
        let start = j * scale;
        let end = start + scale;
        let mut sum = 0.0f64;
        for &v in &data[start..end] {
            sum += v;
        }
        let mean = sum / (scale as f64);
        if !mean.is_finite() {
            return Err(MathError::NumericalError {
                reason: "mse: non-finite coarse-grained mean".to_string(),
                operation: Some("coarse_grain_mean_into".to_string()),
            });
        }
        out[j] = mean;
    }
    Ok(())
}

pub fn coarse_grain_mean(data: &[f64], scale: usize) -> MathResult<Vec<f64>> {
    validate_finite(data)?;
    if scale < 1 {
        return Err(MathError::InvalidParameter {
            parameter: "scale".to_string(),
            value: scale as f64,
            constraint: "scale must be >= 1".to_string(),
        });
    }
    let m = data.len() / scale;
    let mut out = vec![0.0f64; m];
    coarse_grain_mean_into(data, scale, &mut out)?;
    Ok(out)
}

/// Compute multiscale sample entropy for scales `1..=max_scale`.
///
/// For each scale `s`:
/// 1) coarse-grain `data` into length `floor(n/s)` by mean of non-overlapping blocks,
/// 2) compute `SampEn` on the coarse series using embedding `m`, delay `tau`, tolerance `r`.
///
/// Bounds:
/// - `max_scale` must be <= `max_scale_cap` (explicit caller-provided cap).
/// - For each scale, if `floor(n/s)` is too small for SampEn, returns `Err`.
pub fn multiscale_sample_entropy_into_with_workspace(
    data: &[f64],
    m: usize,
    tau: usize,
    r: f64,
    max_scale: usize,
    max_scale_cap: usize,
    out: &mut [f64],
    workspace: &mut MseWorkspace,
) -> MathResult<()> {
    validate_finite(data)?;
    if max_scale < 1 {
        return Err(MathError::InvalidParameter {
            parameter: "max_scale".to_string(),
            value: max_scale as f64,
            constraint: "max_scale must be >= 1".to_string(),
        });
    }
    if max_scale > max_scale_cap {
        return Err(MathError::InvalidParameter {
            parameter: "max_scale".to_string(),
            value: max_scale as f64,
            constraint: format!("max_scale must be <= max_scale_cap={max_scale_cap}"),
        });
    }
    if out.len() != max_scale {
        return Err(MathError::InvalidParameter {
            parameter: "out".to_string(),
            value: out.len() as f64,
            constraint: format!("out must have length max_scale={max_scale}"),
        });
    }

    for s in 1..=max_scale {
        let coarse_len = data.len() / s;
        if coarse_len == 0 {
            return Err(MathError::InsufficientDataAlgo {
                required: s,
                actual: data.len(),
            });
        }
        workspace.ensure_coarse_len(coarse_len);
        coarse_grain_mean_into(data, s, &mut workspace.coarse)?;
        let se = sample_entropy_chebyshev(&workspace.coarse, m, tau, r, &mut workspace.sampen)?;
        if !se.is_finite() {
            return Err(MathError::NumericalError {
                reason: "mse: non-finite SampEn result".to_string(),
                operation: Some("multiscale_sample_entropy_into_with_workspace".to_string()),
            });
        }
        out[s - 1] = se;
    }
    Ok(())
}

pub fn multiscale_sample_entropy(
    data: &[f64],
    m: usize,
    tau: usize,
    r: f64,
    max_scale: usize,
    max_scale_cap: usize,
) -> MathResult<Vec<f64>> {
    let mut out = vec![0.0f64; max_scale];
    let mut ws = MseWorkspace::with_capacity(data.len());
    multiscale_sample_entropy_into_with_workspace(
        data,
        m,
        tau,
        r,
        max_scale,
        max_scale_cap,
        &mut out,
        &mut ws,
    )?;
    Ok(out)
}
