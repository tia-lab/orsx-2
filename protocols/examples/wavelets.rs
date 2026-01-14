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

use crate::core::variance::variance_biased;
use crate::signal::types::WaveletFamily;
use crate::{MathError, MathResult};

const SQRT2_INV: f64 = 0.7071067811865476;

// Daubechies wavelet/scaling filters (orthonormal DWT). MODWT uses the same coefficients,
// scaled by 1/sqrt(2) at each level.
//
// Naming convention in this module:
// - D4: 4-tap Daubechies (2 vanishing moments)
// - D6: 6-tap Daubechies (3 vanishing moments)
// - D8: 8-tap Daubechies (4 vanishing moments)
const H0: f64 = 0.4829629131445341;
const H1: f64 = 0.8365163037378079;
const H2: f64 = 0.2241438680420134;
const H3: f64 = -0.12940952255126034;

const G0: f64 = -0.12940952255126034;
const G1: f64 = -0.2241438680420134;
const G2: f64 = 0.8365163037378079;
const G3: f64 = -0.4829629131445341;

// 6-tap Daubechies (D6) scaling coefficients.
const D6_H: [f64; 6] = [
    0.3326705529500826,
    0.8068915093110928,
    0.4598775021184915,
    -0.13501102001039084,
    -0.08544127388202666,
    0.03522629188570953,
];

// Corresponding wavelet coefficients: g[k] = (-1)^k * h[L-1-k].
const D6_G: [f64; 6] = [
    0.03522629188570953,
    0.08544127388202666,
    -0.13501102001039084,
    -0.4598775021184915,
    0.8068915093110928,
    -0.3326705529500826,
];

// 8-tap Daubechies (D8) scaling coefficients.
const D8_H: [f64; 8] = [
    0.2303778133088964,
    0.7148465705529154,
    0.6308807679298587,
    -0.027983769416859854,
    -0.18703481171888114,
    0.030841381835986965,
    0.0328830116668852,
    -0.010597401785069032,
];

const D8_G: [f64; 8] = [
    -0.010597401785069032,
    -0.0328830116668852,
    0.030841381835986965,
    0.18703481171888114,
    -0.027983769416859854,
    -0.6308807679298587,
    0.7148465705529154,
    -0.2303778133088964,
];

#[derive(Debug, Clone, Copy)]
struct ModwtFilters {
    h: &'static [f64],
    g: &'static [f64],
}

fn modwt_filters(family: WaveletFamily) -> Option<ModwtFilters> {
    match family {
        WaveletFamily::ModwtD4 => Some(ModwtFilters {
            h: &[H0, H1, H2, H3],
            g: &[G0, G1, G2, G3],
        }),
        WaveletFamily::ModwtD6 => Some(ModwtFilters { h: &D6_H, g: &D6_G }),
        WaveletFamily::ModwtD8 => Some(ModwtFilters { h: &D8_H, g: &D8_G }),
        WaveletFamily::Haar => None,
    }
}

#[derive(Debug, Default, Clone)]
pub struct ModwtD4Workspace {
    pub(crate) current: Vec<f64>,
    pub(crate) next: Vec<f64>,
}

impl ModwtD4Workspace {
    pub fn with_capacity(n: usize) -> MathResult<Self> {
        if n == 0 {
            return Err(MathError::InsufficientDataAlgo {
                required: 1,
                actual: 0,
            });
        }
        Ok(Self {
            current: Vec::with_capacity(n),
            next: Vec::with_capacity(n),
        })
    }

    pub fn prepare(&mut self, n: usize) -> MathResult<()> {
        if n == 0 {
            return Err(MathError::InsufficientDataAlgo {
                required: 1,
                actual: 0,
            });
        }
        self.current.clear();
        self.current.resize(n, 0.0);
        self.next.clear();
        self.next.resize(n, 0.0);
        Ok(())
    }
}

fn validate_finite(values: &[f64], name: &'static str) -> MathResult<()> {
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

fn validate_scale(scale: usize, n: usize) -> MathResult<usize> {
    if scale < 2 {
        return Err(MathError::InvalidParameter {
            parameter: "scale".to_string(),
            value: scale as f64,
            constraint: "must be >= 2".to_string(),
        });
    }
    if scale > n {
        return Err(MathError::InvalidParameter {
            parameter: "scale".to_string(),
            value: scale as f64,
            constraint: format!("must be <= n={n}"),
        });
    }
    if !scale.is_power_of_two() {
        return Err(MathError::InvalidParameter {
            parameter: "scale".to_string(),
            value: scale as f64,
            constraint: "must be a power of two".to_string(),
        });
    }
    Ok(scale.trailing_zeros() as usize)
}

/// Compute MODWT D4 detail coefficients at `level` (1-indexed).
///
/// Boundary condition: circular (periodic).
pub fn modwt_d4_detail_level(values: &[f64], level: usize) -> MathResult<Vec<f64>> {
    validate_finite(values, "values")?;
    let mut ws = ModwtD4Workspace::with_capacity(values.len())?;
    let mut out = vec![0.0f64; values.len()];
    modwt_d4_detail_level_into_with_workspace(values, level, &mut out, &mut ws)?;
    Ok(out)
}

pub fn modwt_d4_detail_level_into_with_workspace(
    values: &[f64],
    level: usize,
    out: &mut [f64],
    workspace: &mut ModwtD4Workspace,
) -> MathResult<()> {
    modwt_detail_level_into_with_workspace(values, WaveletFamily::ModwtD4, level, out, workspace)
}

pub fn modwt_detail_level(
    values: &[f64],
    family: WaveletFamily,
    level: usize,
) -> MathResult<Vec<f64>> {
    let mut ws = ModwtD4Workspace::with_capacity(values.len())?;
    let mut out = vec![0.0f64; values.len()];
    modwt_detail_level_into_with_workspace(values, family, level, &mut out, &mut ws)?;
    Ok(out)
}

pub fn modwt_detail_level_into_with_workspace(
    values: &[f64],
    family: WaveletFamily,
    level: usize,
    out: &mut [f64],
    workspace: &mut ModwtD4Workspace,
) -> MathResult<()> {
    validate_finite(values, "values")?;
    if level == 0 {
        return Err(MathError::InvalidParameter {
            parameter: "level".to_string(),
            value: 0.0,
            constraint: "must be >= 1".to_string(),
        });
    }
    let n = values.len();
    if out.len() != n {
        return Err(MathError::InvalidParameter {
            parameter: "out".to_string(),
            value: out.len() as f64,
            constraint: format!("must have length n={n}"),
        });
    }

    let filters = modwt_filters(family).ok_or_else(|| MathError::InvalidParameter {
        parameter: "family".to_string(),
        value: 0.0,
        constraint: "family must be a MODWT Daubechies family".to_string(),
    })?;
    if filters.h.len() != filters.g.len() || filters.h.is_empty() {
        return Err(MathError::InvalidData(
            "modwt: invalid filter definition".to_string(),
        ));
    }

    workspace.prepare(n)?;
    workspace.current.copy_from_slice(values);

    for j in 1..=level {
        let dilation = 1usize << (j - 1);
        let l = filters.h.len();

        if n.is_power_of_two() {
            let mask = n - 1;
            let mut offs = [0usize; 8];
            if l > offs.len() {
                return Err(MathError::InvalidData(
                    "modwt: unsupported filter length".to_string(),
                ));
            }
            for k in 0..l {
                offs[k] = ((k * dilation) & mask) as usize;
            }
            for t in 0..n {
                let mut w = 0.0f64;
                for k in 0..l {
                    let idx = (t + n - offs[k]) & mask;
                    w += filters.g[k] * workspace.current[idx];
                }
                workspace.next[t] = w * SQRT2_INV;
            }
        } else {
            let mut offs = [0usize; 8];
            if l > offs.len() {
                return Err(MathError::InvalidData(
                    "modwt: unsupported filter length".to_string(),
                ));
            }
            for k in 0..l {
                offs[k] = (k * dilation) % n;
            }
            #[inline]
            fn wrap_sub(t: usize, d: usize, n: usize) -> usize {
                debug_assert!(d < n);
                if t >= d {
                    t - d
                } else {
                    t + (n - d)
                }
            }
            for t in 0..n {
                let mut w = 0.0f64;
                for k in 0..l {
                    w += filters.g[k] * workspace.current[wrap_sub(t, offs[k], n)];
                }
                workspace.next[t] = w * SQRT2_INV;
            }
        }

        if j == level {
            out.copy_from_slice(&workspace.next);
            break;
        }

        // Scaling update: V_j from V_{j-1}.
        if n.is_power_of_two() {
            let mask = n - 1;
            let l = filters.h.len();
            let mut offs = [0usize; 8];
            for k in 0..l {
                offs[k] = ((k * dilation) & mask) as usize;
            }
            for t in 0..n {
                let mut v = 0.0f64;
                for k in 0..l {
                    let idx = (t + n - offs[k]) & mask;
                    v += filters.h[k] * workspace.current[idx];
                }
                workspace.current[t] = v * SQRT2_INV;
            }
        } else {
            let l = filters.h.len();
            let mut offs = [0usize; 8];
            for k in 0..l {
                offs[k] = (k * dilation) % n;
            }
            #[inline]
            fn wrap_sub(t: usize, d: usize, n: usize) -> usize {
                debug_assert!(d < n);
                if t >= d {
                    t - d
                } else {
                    t + (n - d)
                }
            }
            for t in 0..n {
                let mut v = 0.0f64;
                for k in 0..l {
                    v += filters.h[k] * workspace.current[wrap_sub(t, offs[k], n)];
                }
                workspace.current[t] = v * SQRT2_INV;
            }
        }
    }

    if out.iter().any(|v| !v.is_finite()) {
        return Err(MathError::NumericalError {
            reason: "MODWT D4: non-finite output".to_string(),
            operation: Some("modwt_d4_detail_level".to_string()),
        });
    }
    Ok(())
}

/// Wavelet variance proxy: mean square of detail coefficients at dyadic `scale`.
///
/// - For `ModwtD4`: uses `level = log2(scale)` and MODWT detail coefficients at that level.
/// - For `Haar`: uses block means over `scale` and computes the variance of the Haar detail coefficients.
pub fn wavelet_variance(values: &[f64], family: WaveletFamily, scale: usize) -> MathResult<f64> {
    validate_finite(values, "values")?;
    match family {
        WaveletFamily::ModwtD4 | WaveletFamily::ModwtD6 | WaveletFamily::ModwtD8 => {
            wavelet_variance_modwt(values, family, scale)
        }
        WaveletFamily::Haar => wavelet_variance_haar(values, scale),
    }
}

pub fn wavelet_variance_modwt_d4(values: &[f64], scale: usize) -> MathResult<f64> {
    wavelet_variance_modwt(values, WaveletFamily::ModwtD4, scale)
}

pub fn wavelet_variance_modwt(
    values: &[f64],
    family: WaveletFamily,
    scale: usize,
) -> MathResult<f64> {
    validate_finite(values, "values")?;
    let n = values.len();
    let level = validate_scale(scale, n)?;
    let detail = modwt_detail_level(values, family, level)?;
    let mean_sq = detail.iter().map(|&x| x * x).sum::<f64>() / (detail.len() as f64);
    if !(mean_sq.is_finite() && mean_sq >= 0.0) {
        return Err(MathError::NumericalError {
            reason: "wavelet_variance_modwt: non-finite output".to_string(),
            operation: Some("wavelet_variance_modwt".to_string()),
        });
    }
    Ok(mean_sq)
}

pub fn wavelet_variance_haar(values: &[f64], scale: usize) -> MathResult<f64> {
    validate_finite(values, "values")?;
    if scale == 0 {
        return Err(MathError::InvalidParameter {
            parameter: "scale".to_string(),
            value: 0.0,
            constraint: "must be >= 1".to_string(),
        });
    }
    let n = values.len();
    if 2 * scale > n {
        return Err(MathError::InsufficientDataAlgo {
            required: 2 * scale,
            actual: n,
        });
    }

    // Prefix sums for stable block mean computation.
    let mut prefix = vec![0.0f64; n + 1];
    for i in 0..n {
        prefix[i + 1] = prefix[i] + values[i];
        if !prefix[i + 1].is_finite() {
            return Err(MathError::NumericalInstability(
                "haar_variance: prefix sum became non-finite".to_string(),
            ));
        }
    }

    let m = n - 2 * scale + 1;
    if m < 2 {
        return Err(MathError::InsufficientDataAlgo {
            required: 2,
            actual: m,
        });
    }

    let mut coeffs = vec![0.0f64; m];
    for i in 0..m {
        let mean1 = (prefix[i + scale] - prefix[i]) / (scale as f64);
        let mean2 = (prefix[i + 2 * scale] - prefix[i + scale]) / (scale as f64);
        let c = (mean1 - mean2) * SQRT2_INV;
        if !c.is_finite() {
            return Err(MathError::NumericalError {
                reason: "haar_variance: non-finite coefficient".to_string(),
                operation: Some("wavelet_variance_haar".to_string()),
            });
        }
        coeffs[i] = c;
    }

    // Use biased variance (mean square around mean) on coefficients; coefficients already represent detail signal.
    let var = variance_biased(&coeffs)?;
    if !(var.is_finite() && var >= 0.0) {
        return Err(MathError::NumericalError {
            reason: "haar_variance: non-finite output".to_string(),
            operation: Some("wavelet_variance_haar".to_string()),
        });
    }
    Ok(var)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdKind {
    Hard,
    Soft,
}

#[inline]
fn hard_threshold(x: f64, t: f64) -> f64 {
    if x.abs() >= t {
        x
    } else {
        0.0
    }
}

#[inline]
fn soft_threshold(x: f64, t: f64) -> f64 {
    let ax = x.abs();
    if ax <= t {
        0.0
    } else {
        let s = if x.is_sign_negative() { -1.0 } else { 1.0 };
        s * (ax - t)
    }
}

fn validate_threshold(threshold: f64) -> MathResult<()> {
    if !threshold.is_finite() {
        return Err(MathError::InvalidParameter {
            parameter: "threshold".to_string(),
            value: threshold,
            constraint: "must be finite".to_string(),
        });
    }
    if threshold < 0.0 {
        return Err(MathError::InvalidParameter {
            parameter: "threshold".to_string(),
            value: threshold,
            constraint: "must be >= 0".to_string(),
        });
    }
    Ok(())
}

/// Apply a hard/soft threshold to coefficients in-place.
///
/// Determinism:
/// - purely deterministic, no allocations.
pub fn threshold_coefficients_in_place(
    coeffs: &mut [f64],
    threshold: f64,
    kind: ThresholdKind,
) -> MathResult<()> {
    validate_finite(coeffs, "coeffs")?;
    validate_threshold(threshold)?;

    if threshold == 0.0 {
        return Ok(());
    }

    match kind {
        ThresholdKind::Hard => {
            for v in coeffs.iter_mut() {
                *v = hard_threshold(*v, threshold);
            }
        }
        ThresholdKind::Soft => {
            for v in coeffs.iter_mut() {
                *v = soft_threshold(*v, threshold);
            }
        }
    }
    Ok(())
}

/// Universal threshold: `t = sigma * sqrt(2 ln(n))`.
///
/// This does not estimate `sigma`; it requires `sigma` to be explicitly provided by the caller.
pub fn universal_threshold(sigma: f64, n: usize) -> MathResult<f64> {
    if n == 0 {
        return Err(MathError::InvalidParameter {
            parameter: "n".to_string(),
            value: 0.0,
            constraint: "must be >= 1".to_string(),
        });
    }
    if !sigma.is_finite() {
        return Err(MathError::InvalidParameter {
            parameter: "sigma".to_string(),
            value: sigma,
            constraint: "must be finite".to_string(),
        });
    }
    if sigma < 0.0 {
        return Err(MathError::InvalidParameter {
            parameter: "sigma".to_string(),
            value: sigma,
            constraint: "must be >= 0".to_string(),
        });
    }
    let ln_n = (n as f64).ln();
    let t = sigma * (2.0 * ln_n).sqrt();
    if !t.is_finite() || t < 0.0 {
        return Err(MathError::NumericalError {
            reason: "universal_threshold: non-finite result".to_string(),
            operation: Some("universal_threshold".to_string()),
        });
    }
    Ok(t)
}

fn validate_modwt_levels(n: usize, levels: usize) -> MathResult<()> {
    if n < 4 {
        return Err(MathError::InsufficientDataAlgo {
            required: 4,
            actual: n,
        });
    }
    if levels == 0 {
        return Err(MathError::InvalidParameter {
            parameter: "levels".to_string(),
            value: 0.0,
            constraint: "must be >= 1".to_string(),
        });
    }
    let max_levels = (usize::BITS as usize) - 1 - (n as u64).leading_zeros() as usize;
    if levels > max_levels {
        return Err(MathError::InvalidParameter {
            parameter: "levels".to_string(),
            value: levels as f64,
            constraint: format!("must be <= floor_log2(n)={max_levels}"),
        });
    }
    Ok(())
}

#[derive(Debug, Default, Clone)]
pub struct ModwtD4DenoiseWorkspace {
    modwt: ModwtD4Workspace,
    details_flat: Vec<f64>,
}

impl ModwtD4DenoiseWorkspace {
    pub fn with_capacity(n: usize, levels: usize) -> MathResult<Self> {
        validate_modwt_levels(n, levels)?;
        Ok(Self {
            modwt: ModwtD4Workspace::with_capacity(n)?,
            details_flat: Vec::with_capacity(n * levels),
        })
    }

    pub fn prepare(&mut self, n: usize, levels: usize) -> MathResult<()> {
        validate_modwt_levels(n, levels)?;
        self.modwt.prepare(n)?;
        self.details_flat.clear();
        self.details_flat.resize(n * levels, 0.0);
        Ok(())
    }
}

fn validate_filters(filters: ModwtFilters) -> MathResult<()> {
    if filters.h.len() != filters.g.len() || filters.h.is_empty() {
        return Err(MathError::InvalidData(
            "modwt: invalid filter definition".to_string(),
        ));
    }
    if filters.h.len() > 8 {
        return Err(MathError::InvalidData(
            "modwt: unsupported filter length".to_string(),
        ));
    }
    Ok(())
}

fn modwt_decompose_store_details_into(
    values: &[f64],
    levels: usize,
    filters: ModwtFilters,
    details_flat: &mut [f64],
    workspace: &mut ModwtD4Workspace,
) -> MathResult<()> {
    validate_finite(values, "values")?;
    let n = values.len();
    validate_modwt_levels(n, levels)?;
    validate_filters(filters)?;
    if details_flat.len() != n * levels {
        return Err(MathError::InvalidParameter {
            parameter: "details_flat".to_string(),
            value: details_flat.len() as f64,
            constraint: format!("must have length n*levels={}", n * levels),
        });
    }

    workspace.prepare(n)?;
    workspace.current.copy_from_slice(values);

    #[inline]
    fn wrap_sub(t: usize, d: usize, n: usize) -> usize {
        debug_assert!(d < n);
        if t >= d {
            t - d
        } else {
            t + (n - d)
        }
    }

    let l = filters.h.len();
    if n.is_power_of_two() {
        let mask = n - 1;
        for j in 1..=levels {
            let dilation = 1usize << (j - 1);
            let mut offs = [0usize; 8];
            for k in 0..l {
                offs[k] = (k * dilation) & mask;
            }

            for t in 0..n {
                let mut w = 0.0f64;
                for k in 0..l {
                    w += filters.g[k] * workspace.current[(t + n - offs[k]) & mask];
                }
                workspace.next[t] = w * SQRT2_INV;
            }
            let detail_slice = &mut details_flat[(j - 1) * n..j * n];
            detail_slice.copy_from_slice(&workspace.next);

            for t in 0..n {
                let mut v = 0.0f64;
                for k in 0..l {
                    v += filters.h[k] * workspace.current[(t + n - offs[k]) & mask];
                }
                workspace.next[t] = v * SQRT2_INV;
            }
            std::mem::swap(&mut workspace.current, &mut workspace.next);
        }
        return Ok(());
    }

    for j in 1..=levels {
        let dilation = 1usize << (j - 1);
        let mut offs = [0usize; 8];
        for k in 0..l {
            offs[k] = (k * dilation) % n;
        }

        for t in 0..n {
            let mut w = 0.0f64;
            for k in 0..l {
                w += filters.g[k] * workspace.current[wrap_sub(t, offs[k], n)];
            }
            workspace.next[t] = w * SQRT2_INV;
        }
        let detail_slice = &mut details_flat[(j - 1) * n..j * n];
        detail_slice.copy_from_slice(&workspace.next);

        for t in 0..n {
            let mut v = 0.0f64;
            for k in 0..l {
                v += filters.h[k] * workspace.current[wrap_sub(t, offs[k], n)];
            }
            workspace.next[t] = v * SQRT2_INV;
        }
        std::mem::swap(&mut workspace.current, &mut workspace.next);
    }

    Ok(())
}

fn imodwt_reconstruct_from_details_into(
    levels: usize,
    filters: ModwtFilters,
    details_flat: &[f64],
    out: &mut [f64],
    workspace: &mut ModwtD4Workspace,
) -> MathResult<()> {
    let n = out.len();
    validate_modwt_levels(n, levels)?;
    validate_filters(filters)?;
    if details_flat.len() != n * levels {
        return Err(MathError::InvalidParameter {
            parameter: "details_flat".to_string(),
            value: details_flat.len() as f64,
            constraint: format!("must have length n*levels={}", n * levels),
        });
    }

    #[inline]
    fn wrap_add(t: usize, d: usize, n: usize) -> usize {
        debug_assert!(d < n);
        if t < n - d {
            t + d
        } else {
            t - (n - d)
        }
    }

    let l = filters.h.len();
    if n.is_power_of_two() {
        let mask = n - 1;
        for j in (1..=levels).rev() {
            let dilation = 1usize << (j - 1);
            let mut offs = [0usize; 8];
            for k in 0..l {
                offs[k] = (k * dilation) & mask;
            }

            let wj = &details_flat[(j - 1) * n..j * n];
            for t in 0..n {
                let mut v_part = 0.0f64;
                let mut w_part = 0.0f64;
                for k in 0..l {
                    let idx = (t + offs[k]) & mask;
                    v_part += filters.h[k] * workspace.current[idx];
                    w_part += filters.g[k] * wj[idx];
                }
                workspace.next[t] = (v_part + w_part) * SQRT2_INV;
            }
            std::mem::swap(&mut workspace.current, &mut workspace.next);
        }
    } else {
        for j in (1..=levels).rev() {
            let dilation = 1usize << (j - 1);
            let mut offs = [0usize; 8];
            for k in 0..l {
                offs[k] = (k * dilation) % n;
            }

            let wj = &details_flat[(j - 1) * n..j * n];
            for t in 0..n {
                let mut v_part = 0.0f64;
                let mut w_part = 0.0f64;
                for k in 0..l {
                    let idx = wrap_add(t, offs[k], n);
                    v_part += filters.h[k] * workspace.current[idx];
                    w_part += filters.g[k] * wj[idx];
                }
                workspace.next[t] = (v_part + w_part) * SQRT2_INV;
            }
            std::mem::swap(&mut workspace.current, &mut workspace.next);
        }
    }

    out.copy_from_slice(&workspace.current);
    if out.iter().any(|v| !v.is_finite()) {
        return Err(MathError::NumericalError {
            reason: "modwt_denoise: non-finite output".to_string(),
            operation: Some("modwt_denoise".to_string()),
        });
    }
    Ok(())
}

/// MODWT D4 denoising by thresholding detail coefficients (circular boundary).
///
/// Algorithm:
/// 1) compute MODWT D4 details `W_1..W_J` and smooth `V_J`,
/// 2) apply thresholding to each `W_j`,
/// 3) reconstruct `x_denoised` via inverse MODWT.
pub fn modwt_d4_denoise_into_with_workspace(
    values: &[f64],
    levels: usize,
    threshold: f64,
    kind: ThresholdKind,
    out: &mut [f64],
    workspace: &mut ModwtD4DenoiseWorkspace,
) -> MathResult<()> {
    validate_finite(values, "values")?;
    validate_threshold(threshold)?;
    let n = values.len();
    validate_modwt_levels(n, levels)?;
    if out.len() != n {
        return Err(MathError::InvalidParameter {
            parameter: "out".to_string(),
            value: out.len() as f64,
            constraint: format!("must have length n={n}"),
        });
    }

    workspace.prepare(n, levels)?;
    let filters = modwt_filters(WaveletFamily::ModwtD4)
        .ok_or_else(|| MathError::InvalidData("modwt_d4: missing filters".to_string()))?;
    modwt_decompose_store_details_into(
        values,
        levels,
        filters,
        &mut workspace.details_flat,
        &mut workspace.modwt,
    )?;

    for j in 0..levels {
        let s = &mut workspace.details_flat[j * n..(j + 1) * n];
        threshold_coefficients_in_place(s, threshold, kind)?;
    }

    imodwt_reconstruct_from_details_into(
        levels,
        filters,
        &workspace.details_flat,
        out,
        &mut workspace.modwt,
    )?;
    Ok(())
}

pub fn modwt_d4_denoise(
    values: &[f64],
    levels: usize,
    threshold: f64,
    kind: ThresholdKind,
) -> MathResult<Vec<f64>> {
    let mut out = vec![0.0f64; values.len()];
    let mut ws = ModwtD4DenoiseWorkspace::with_capacity(values.len(), levels)?;
    modwt_d4_denoise_into_with_workspace(values, levels, threshold, kind, &mut out, &mut ws)?;
    Ok(out)
}

#[derive(Debug, Default, Clone)]
pub struct ModwtDenoiseWorkspace {
    modwt: ModwtD4Workspace,
    details_flat: Vec<f64>,
}

impl ModwtDenoiseWorkspace {
    pub fn with_capacity(n: usize, levels: usize) -> MathResult<Self> {
        validate_modwt_levels(n, levels)?;
        Ok(Self {
            modwt: ModwtD4Workspace::with_capacity(n)?,
            details_flat: Vec::with_capacity(n * levels),
        })
    }

    pub fn prepare(&mut self, n: usize, levels: usize) -> MathResult<()> {
        validate_modwt_levels(n, levels)?;
        self.modwt.prepare(n)?;
        self.details_flat.clear();
        self.details_flat.resize(n * levels, 0.0);
        Ok(())
    }
}

/// Generic MODWT denoising for Daubechies families (circular boundary).
///
/// This is identical to `modwt_d4_denoise_*` but allows choosing the family (`D4`, `D6`, `D8`).
pub fn modwt_denoise_into_with_workspace(
    values: &[f64],
    family: WaveletFamily,
    levels: usize,
    threshold: f64,
    kind: ThresholdKind,
    out: &mut [f64],
    workspace: &mut ModwtDenoiseWorkspace,
) -> MathResult<()> {
    validate_finite(values, "values")?;
    validate_threshold(threshold)?;
    let n = values.len();
    validate_modwt_levels(n, levels)?;
    if out.len() != n {
        return Err(MathError::InvalidParameter {
            parameter: "out".to_string(),
            value: out.len() as f64,
            constraint: format!("must have length n={n}"),
        });
    }

    let filters = modwt_filters(family).ok_or_else(|| MathError::InvalidParameter {
        parameter: "family".to_string(),
        value: 0.0,
        constraint: "family must be a MODWT Daubechies family".to_string(),
    })?;

    workspace.prepare(n, levels)?;
    modwt_decompose_store_details_into(
        values,
        levels,
        filters,
        &mut workspace.details_flat,
        &mut workspace.modwt,
    )?;

    for j in 0..levels {
        let s = &mut workspace.details_flat[j * n..(j + 1) * n];
        threshold_coefficients_in_place(s, threshold, kind)?;
    }

    imodwt_reconstruct_from_details_into(
        levels,
        filters,
        &workspace.details_flat,
        out,
        &mut workspace.modwt,
    )?;
    Ok(())
}

pub fn modwt_denoise(
    values: &[f64],
    family: WaveletFamily,
    levels: usize,
    threshold: f64,
    kind: ThresholdKind,
) -> MathResult<Vec<f64>> {
    let mut out = vec![0.0f64; values.len()];
    let mut ws = ModwtDenoiseWorkspace::with_capacity(values.len(), levels)?;
    modwt_denoise_into_with_workspace(values, family, levels, threshold, kind, &mut out, &mut ws)?;
    Ok(out)
}
