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

use crate::core::calculus::checked_sqrt;
use crate::{MathError, MathResult};

fn validate_all_finite(values: &[f64], parameter: &str) -> MathResult<()> {
    if values.iter().any(|v| !v.is_finite()) {
        return Err(MathError::InvalidData(format!(
            "{parameter}: all values must be finite"
        )));
    }
    Ok(())
}

/// Mean-centered cumulative sum used by DFA/MF-DFA.
///
/// This is equivalent to `math::core::integration::integrate_series`, but lives here as a
/// signal-processing building block for DFA pipelines.
pub fn integrate_series(values: &[f64]) -> MathResult<Vec<f64>> {
    crate::core::integration::integrate_series(values)
}

/// RMS fluctuation of a segment after removing a best-fit linear trend (OLS with intercept).
///
/// This corresponds to DFA detrending of order 1 on a segment.
pub fn segment_fluctuation_rms_linear(segment: &[f64]) -> MathResult<f64> {
    let n = segment.len();
    if n < 3 {
        return Err(MathError::InsufficientDataAlgo {
            required: 3,
            actual: n,
        });
    }
    validate_all_finite(segment, "segment")?;

    // OLS for x = 0..n-1 with intercept, using centered sums for stability:
    // slope = Σ (xi-x̄)(yi-ȳ) / Σ (xi-x̄)^2
    // intercept = ȳ - slope x̄
    let n_f = n as f64;
    let x_mean = (n_f - 1.0) / 2.0;
    let y_mean = segment.iter().sum::<f64>() / n_f;
    if !y_mean.is_finite() {
        return Err(MathError::NumericalError {
            reason: "segment_fluctuation_rms_linear: non-finite mean".to_string(),
            operation: Some("segment_fluctuation_rms_linear".to_string()),
        });
    }

    let mut sxx = 0.0f64;
    let mut sxy = 0.0f64;
    for (i, &y) in segment.iter().enumerate() {
        let x = (i as f64) - x_mean;
        let yc = y - y_mean;
        sxx += x * x;
        sxy += x * yc;
    }
    if !(sxx.is_finite() && sxy.is_finite()) {
        return Err(MathError::NumericalInstability(
            "segment_fluctuation_rms_linear: non-finite intermediate".to_string(),
        ));
    }
    if sxx <= 0.0 {
        return Err(MathError::InvalidData(
            "segment_fluctuation_rms_linear: zero variance in x".to_string(),
        ));
    }

    let slope = sxy / sxx;
    let intercept = y_mean - slope * x_mean;
    if !(slope.is_finite() && intercept.is_finite()) {
        return Err(MathError::NumericalError {
            reason: "segment_fluctuation_rms_linear: non-finite regression coefficients"
                .to_string(),
            operation: Some("segment_fluctuation_rms_linear".to_string()),
        });
    }

    let mut rss = 0.0f64;
    for (i, &y) in segment.iter().enumerate() {
        let fitted = intercept + slope * (i as f64);
        let r = y - fitted;
        rss += r * r;
        if !rss.is_finite() {
            return Err(MathError::NumericalInstability(
                "segment_fluctuation_rms_linear: residual sum of squares became non-finite"
                    .to_string(),
            ));
        }
    }

    checked_sqrt(rss / n_f)
}

/// Geometric window sizes for scaling analysis.
///
/// This replicates the legacy behavior:
/// - `max_size = n / max_size_factor`
/// - start at `min_size`, multiply by `growth_factor=1.1`, round to usize, keep strictly increasing.
pub fn generate_window_sizes(
    n: usize,
    min_size: usize,
    max_size_factor: f64,
) -> MathResult<Vec<usize>> {
    if n == 0 {
        return Err(MathError::InsufficientDataAlgo {
            required: 1,
            actual: 0,
        });
    }
    if min_size == 0 {
        return Err(MathError::InvalidParameter {
            parameter: "min_size".to_string(),
            value: 0.0,
            constraint: "must be >= 1".to_string(),
        });
    }
    if !(max_size_factor.is_finite() && max_size_factor > 0.0) {
        return Err(MathError::InvalidParameter {
            parameter: "max_size_factor".to_string(),
            value: max_size_factor,
            constraint: "must be finite and > 0".to_string(),
        });
    }

    let max_size_f64 = (n as f64) / max_size_factor;
    let min_size_f64 = min_size as f64;
    let growth_factor: f64 = 1.1;

    let sequence_length = if max_size_f64 <= min_size_f64 {
        1
    } else {
        let ratio: f64 = max_size_f64 / min_size_f64;
        ((ratio.ln() / growth_factor.ln()).floor() as usize) + 1
    };

    let mut sizes = Vec::with_capacity(sequence_length.min(1000));
    let mut current_size_f64 = min_size_f64;
    let mut iteration_count = 0usize;
    const MAX_ITERATIONS: usize = 1000;

    while current_size_f64 <= max_size_f64 && iteration_count < MAX_ITERATIONS {
        let size_usize = current_size_f64.round() as usize;
        if sizes.is_empty() || size_usize > *sizes.last().unwrap() {
            sizes.push(size_usize);
        }
        current_size_f64 *= growth_factor;
        iteration_count += 1;
    }

    if sizes.is_empty() {
        sizes.push(min_size);
    }

    Ok(sizes)
}
