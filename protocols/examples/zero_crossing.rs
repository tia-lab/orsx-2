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

//! Zero-crossing and sign-run-length primitives for scalar time series.
//!
//! These are fast, deterministic structure/chop measurements:
//! - zero-crossing rate (ZCR) over a window,
//! - run-length statistics of the sign sequence (optionally treating zeros specially).

use crate::{MathError, MathResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZeroHandling {
    /// Treat zeros as their own label (0). This counts transitions into/out of zero as sign changes.
    AsZero,
    /// Replace zeros with the previous non-zero sign (if any); leading zeros become 0.
    CarryForward,
    /// Replace zeros with +1 (a deterministic tie-break).
    MapToPositive,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignRunStats {
    pub runs: usize,
    pub mean_run_length: f64,
    pub max_run_length: usize,
    pub pos_runs: usize,
    pub neg_runs: usize,
    pub zero_runs: usize,
}

fn validate_finite(data: &[f64], name: &'static str) -> MathResult<()> {
    if data.is_empty() {
        return Err(MathError::InsufficientDataAlgo {
            required: 1,
            actual: 0,
        });
    }
    if data.iter().any(|v| !v.is_finite()) {
        return Err(MathError::InvalidData(format!(
            "{name}: all values must be finite"
        )));
    }
    Ok(())
}

#[inline]
fn sign_i8(v: f64) -> i8 {
    if v > 0.0 {
        1
    } else if v < 0.0 {
        -1
    } else {
        0
    }
}

// (internal helper removed; we compute sign runs on-the-fly without allocating)

/// Zero-crossing rate (ZCR) computed on the sign sequence.
///
/// Definition:
/// - Let `s[i]` be a sign label in `{-1,0,+1}` after applying `zero_handling`.
/// - ZCR = (# of indices i in 1..n-1 with s[i] != s[i-1]) / (n-1)
pub fn zero_crossing_rate(data: &[f64], zero_handling: ZeroHandling) -> MathResult<f64> {
    validate_finite(data, "data")?;
    if data.len() < 2 {
        return Err(MathError::InsufficientDataAlgo {
            required: 2,
            actual: data.len(),
        });
    }
    let n = data.len();
    let mut prev = match zero_handling {
        ZeroHandling::AsZero => sign_i8(data[0]),
        ZeroHandling::MapToPositive => {
            let s = sign_i8(data[0]);
            if s == 0 {
                1
            } else {
                s
            }
        }
        ZeroHandling::CarryForward => sign_i8(data[0]),
    };
    let mut changes = 0usize;
    for i in 1..n {
        let mut s = sign_i8(data[i]);
        match zero_handling {
            ZeroHandling::AsZero => {}
            ZeroHandling::MapToPositive => {
                if s == 0 {
                    s = 1;
                }
            }
            ZeroHandling::CarryForward => {
                if s == 0 {
                    s = prev;
                }
            }
        }
        if s != prev {
            changes += 1;
        }
        prev = s;
    }
    let denom = (n - 1) as f64;
    Ok((changes as f64) / denom)
}

/// Run-length statistics of the sign sequence for `data`.
pub fn sign_run_stats(data: &[f64], zero_handling: ZeroHandling) -> MathResult<SignRunStats> {
    validate_finite(data, "data")?;
    let mut runs = 0usize;
    let mut pos_runs = 0usize;
    let mut neg_runs = 0usize;
    let mut zero_runs = 0usize;
    let mut max_len = 0usize;
    let mut len_sum = 0u64;

    let mut current = match zero_handling {
        ZeroHandling::AsZero => sign_i8(data[0]),
        ZeroHandling::MapToPositive => {
            let s = sign_i8(data[0]);
            if s == 0 {
                1
            } else {
                s
            }
        }
        ZeroHandling::CarryForward => sign_i8(data[0]),
    };
    let mut len = 1usize;

    for &v in data.iter().skip(1) {
        let mut s = sign_i8(v);
        match zero_handling {
            ZeroHandling::AsZero => {}
            ZeroHandling::MapToPositive => {
                if s == 0 {
                    s = 1;
                }
            }
            ZeroHandling::CarryForward => {
                if s == 0 {
                    s = current;
                }
            }
        }

        if s == current {
            len += 1;
        } else {
            runs += 1;
            len_sum += len as u64;
            max_len = max_len.max(len);
            match current {
                1 => pos_runs += 1,
                -1 => neg_runs += 1,
                _ => zero_runs += 1,
            }
            current = s;
            len = 1;
        }
    }

    runs += 1;
    len_sum += len as u64;
    max_len = max_len.max(len);
    match current {
        1 => pos_runs += 1,
        -1 => neg_runs += 1,
        _ => zero_runs += 1,
    }

    let mean = (len_sum as f64) / (runs as f64);
    if !mean.is_finite() || mean <= 0.0 {
        return Err(MathError::NumericalError {
            reason: "sign_run_stats: non-finite mean run length".to_string(),
            operation: Some("sign_run_stats".to_string()),
        });
    }
    Ok(SignRunStats {
        runs,
        mean_run_length: mean,
        max_run_length: max_len,
        pos_runs,
        neg_runs,
        zero_runs,
    })
}
