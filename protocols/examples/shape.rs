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

//! Fast deterministic scalar “signal shape” measures.

use crate::signal::spectral::periodogram::{calculate_periodogram_into, PeriodogramWorkspace};
use crate::signal::types::DetrendMethod;
use crate::{MathError, MathResult};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HjorthParameters {
    pub activity: f64,
    pub mobility: f64,
    pub complexity: f64,
}

/// Hjorth parameters (activity, mobility, complexity).
///
/// Definitions (discrete):
/// - activity = var(x)
/// - mobility = sqrt(var(dx)/var(x))
/// - complexity = mobility(dx)/mobility(x)
pub fn hjorth_parameters(x: &[f64]) -> MathResult<HjorthParameters> {
    validate_finite_non_empty(x, "x")?;
    if x.len() < 3 {
        return Err(MathError::InsufficientDataAlgo {
            required: 3,
            actual: x.len(),
        });
    }

    let (var0, var1, var2) = hjorth_variances(x)?;
    let activity = var0;
    if activity == 0.0 {
        return Ok(HjorthParameters {
            activity: 0.0,
            mobility: 0.0,
            complexity: 0.0,
        });
    }

    let mobility = (var1 / var0).sqrt();
    let complexity = if var1 == 0.0 {
        0.0
    } else {
        let mobility_d = (var2 / var1).sqrt();
        mobility_d / mobility
    };
    if !activity.is_finite() || !mobility.is_finite() || !complexity.is_finite() {
        return Err(MathError::NumericalError {
            reason: "hjorth: non-finite output".to_string(),
            operation: Some("hjorth_parameters".to_string()),
        });
    }
    Ok(HjorthParameters {
        activity,
        mobility,
        complexity,
    })
}

fn hjorth_variances(x: &[f64]) -> MathResult<(f64, f64, f64)> {
    let n = x.len() as f64;
    let mut mean0 = 0.0;
    for &v in x {
        mean0 += v;
    }
    mean0 /= n;
    if !mean0.is_finite() {
        return Err(MathError::NumericalError {
            reason: "hjorth: non-finite mean".to_string(),
            operation: Some("hjorth_variances".to_string()),
        });
    }

    let mut sse0 = 0.0;
    let mut sse1 = 0.0;
    let mut sse2 = 0.0;

    let mut prev = x[0];
    let mut prev_d1 = x[1] - x[0];
    if !prev_d1.is_finite() {
        return Err(MathError::NumericalError {
            reason: "hjorth: non-finite diff".to_string(),
            operation: Some("hjorth_variances".to_string()),
        });
    }
    for i in 0..x.len() {
        let v = x[i];
        let d0 = v - mean0;
        sse0 += d0 * d0;
        if i > 0 {
            let d1 = v - prev;
            sse1 += d1 * d1;
            if i > 1 {
                let d2 = d1 - prev_d1;
                sse2 += d2 * d2;
            }
            prev_d1 = d1;
        }
        prev = v;
    }

    let var0 = sse0 / n;
    let var1 = sse1 / (n - 1.0);
    let var2 = sse2 / (n - 2.0);
    if !(var0.is_finite() && var1.is_finite() && var2.is_finite()) {
        return Err(MathError::NumericalError {
            reason: "hjorth: non-finite variance".to_string(),
            operation: Some("hjorth_variances".to_string()),
        });
    }
    Ok((var0, var1, var2))
}

/// Teager–Kaiser energy operator average:
/// `ψ[x_n] = x_n^2 - x_{n-1} x_{n+1}`.
pub fn teager_kaiser_energy_mean(x: &[f64]) -> MathResult<f64> {
    validate_finite_non_empty(x, "x")?;
    if x.len() < 3 {
        return Err(MathError::InsufficientDataAlgo {
            required: 3,
            actual: x.len(),
        });
    }
    let mut sum = 0.0;
    for i in 1..(x.len() - 1) {
        let v = x[i] * x[i] - x[i - 1] * x[i + 1];
        sum += v;
    }
    let mean = sum / ((x.len() - 2) as f64);
    if !mean.is_finite() {
        return Err(MathError::NumericalError {
            reason: "tkeo: non-finite mean".to_string(),
            operation: Some("teager_kaiser_energy_mean".to_string()),
        });
    }
    Ok(mean)
}

/// Spectral flatness from a periodogram `P[k]`:
/// `flatness = exp(mean(log P)) / mean(P)` with an epsilon floor.
pub fn spectral_flatness_from_periodogram(p: &[f64], eps: f64) -> MathResult<f64> {
    validate_finite_non_empty(p, "p")?;
    if !(eps.is_finite() && eps > 0.0) {
        return Err(MathError::InvalidParameter {
            parameter: "eps".to_string(),
            value: eps,
            constraint: "must be finite and > 0".to_string(),
        });
    }
    let n = p.len() as f64;
    let mut sum = 0.0;
    let mut sum_log = 0.0;
    for &v in p {
        let vv = v.max(eps);
        sum += vv;
        sum_log += vv.ln();
    }
    let am = sum / n;
    let gm = (sum_log / n).exp();
    let flat = gm / am;
    if !(flat.is_finite() && flat >= 0.0) {
        return Err(MathError::NumericalError {
            reason: "spectral_flatness: non-finite".to_string(),
            operation: Some("spectral_flatness_from_periodogram".to_string()),
        });
    }
    Ok(flat)
}

/// Spectral crest factor from a periodogram `P[k]`: `max(P)/mean(P)`.
pub fn spectral_crest_from_periodogram(p: &[f64]) -> MathResult<f64> {
    validate_finite_non_empty(p, "p")?;
    let mut maxv = 0.0f64;
    let mut sum = 0.0f64;
    for &v in p {
        maxv = maxv.max(v);
        sum += v;
    }
    let mean = sum / (p.len() as f64);
    if mean == 0.0 {
        return Ok(0.0);
    }
    let crest = maxv / mean;
    if !(crest.is_finite() && crest >= 0.0) {
        return Err(MathError::NumericalError {
            reason: "spectral_crest: non-finite".to_string(),
            operation: Some("spectral_crest_from_periodogram".to_string()),
        });
    }
    Ok(crest)
}

/// Spectral entropy (normalized to `[0,1]`) from a non-negative spectrum.
pub fn spectral_entropy_from_periodogram(p: &[f64], eps: f64) -> MathResult<f64> {
    validate_finite_non_empty(p, "p")?;
    if !(eps.is_finite() && eps > 0.0) {
        return Err(MathError::InvalidParameter {
            parameter: "eps".to_string(),
            value: eps,
            constraint: "must be finite and > 0".to_string(),
        });
    }

    let mut sum = 0.0;
    for &v in p {
        let vv = v.max(0.0);
        sum += vv;
    }
    if sum == 0.0 {
        return Ok(0.0);
    }

    let inv_sum = 1.0 / sum;
    let mut h = 0.0;
    for &v in p {
        let prob = (v.max(0.0) * inv_sum).max(eps);
        h -= prob * prob.ln();
    }
    let h_max = (p.len() as f64).ln();
    let h_norm = h / h_max;
    if !(h_norm.is_finite() && h_norm >= 0.0) {
        return Err(MathError::NumericalError {
            reason: "spectral_entropy: non-finite".to_string(),
            operation: Some("spectral_entropy_from_periodogram".to_string()),
        });
    }
    Ok(h_norm.min(1.0))
}

/// Convenience: compute periodogram (mean detrend) then spectral flatness.
pub fn spectral_flatness(x: &[f64], eps: f64) -> MathResult<f64> {
    validate_finite_non_empty(x, "x")?;
    let n = x.len();
    let mut ws = PeriodogramWorkspace::with_capacity(n);
    let mut p = vec![0.0f64; n];
    calculate_periodogram_into(x, DetrendMethod::RemoveMean, &mut p, &mut ws)?;
    spectral_flatness_from_periodogram(&p, eps)
}

fn validate_finite_non_empty(values: &[f64], name: &'static str) -> MathResult<()> {
    if values.is_empty() {
        return Err(MathError::InsufficientDataAlgo {
            required: 1,
            actual: 0,
        });
    }
    if values.iter().any(|v| !v.is_finite()) {
        return Err(MathError::InvalidData(format!(
            "{name}: all values must be finite"
        )));
    }
    Ok(())
}
