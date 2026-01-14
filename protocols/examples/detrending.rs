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

use crate::core::calculus::safe_mean;
use crate::linalg::qr::{
    default_rank_tol, qr_decompose_in_place, solve_least_squares_qr_from_precomputed_qr_into,
    solve_least_squares_qr_into_with_workspace,
};
use crate::linalg::validation::{V11_MAX_M, V11_MAX_N};
use crate::linalg::workspace::QrWorkspace;
use crate::signal::types::DetrendMethod;
use crate::{MathError, MathResult};

#[derive(Debug, Default, Clone)]
pub struct PolynomialDetrendWorkspace {
    pub(crate) qr: QrWorkspace,
    pub(crate) a: Vec<f64>,
    pub(crate) beta: Vec<f64>,
}

impl PolynomialDetrendWorkspace {
    pub fn with_capacity(n: usize, degree: usize) -> MathResult<Self> {
        let k = degree
            .checked_add(1)
            .ok_or_else(|| MathError::InvalidData("degree overflow".to_string()))?;
        let qr = QrWorkspace::with_capacity(n, k)?;
        Ok(Self {
            qr,
            a: Vec::with_capacity(n * k),
            beta: Vec::with_capacity(k),
        })
    }

    pub fn prepare(&mut self, n: usize, degree: usize) -> MathResult<usize> {
        let k = degree
            .checked_add(1)
            .ok_or_else(|| MathError::InvalidData("degree overflow".to_string()))?;
        self.qr.prepare(n, k)?;
        self.a.clear();
        self.a.resize(n * k, 0.0);
        self.beta.clear();
        self.beta.resize(k, 0.0);
        Ok(k)
    }
}

#[derive(Debug, Clone)]
pub struct PolynomialDetrendPrecomputedWorkspace {
    m: usize,
    k: usize,
    a_qr: Vec<f64>,
    tau: Vec<f64>,
    qt_b: Vec<f64>,
    beta: Vec<f64>,
    rank_tol: f64,
}

impl PolynomialDetrendPrecomputedWorkspace {
    pub fn with_capacity(n: usize, degree: usize) -> MathResult<Self> {
        let mut ws = Self {
            m: 0,
            k: 0,
            a_qr: Vec::new(),
            tau: Vec::new(),
            qt_b: Vec::new(),
            beta: Vec::new(),
            rank_tol: 0.0,
        };
        ws.prepare(n, degree)?;
        Ok(ws)
    }

    pub fn prepare(&mut self, n: usize, degree: usize) -> MathResult<()> {
        if degree < 2 {
            return Err(MathError::InvalidParameter {
                parameter: "degree".to_string(),
                value: degree as f64,
                constraint: "must be >= 2 for precomputed polynomial detrend".to_string(),
            });
        }
        if n < 2 {
            return Err(MathError::InvalidParameter {
                parameter: "n".to_string(),
                value: n as f64,
                constraint: "must be >= 2".to_string(),
            });
        }
        if degree >= n {
            return Err(MathError::InvalidParameter {
                parameter: "degree".to_string(),
                value: degree as f64,
                constraint: format!("must be < n={n}"),
            });
        }
        let k = degree
            .checked_add(1)
            .ok_or_else(|| MathError::InvalidData("degree overflow".to_string()))?;
        if k > n {
            return Err(MathError::InvalidParameter {
                parameter: "matrix_shape".to_string(),
                value: k as f64,
                constraint: format!("underdetermined system: n_features={k} > n_obs={n}"),
            });
        }
        if n > V11_MAX_M || k > V11_MAX_N {
            return Err(MathError::InvalidParameter {
                parameter: "matrix_shape".to_string(),
                value: (n.max(k)) as f64,
                constraint: format!("v1.1 bound: m <= {V11_MAX_M}, n <= {V11_MAX_N}"),
            });
        }

        self.m = n;
        self.k = k;
        self.a_qr.clear();
        self.a_qr.resize(n * k, 0.0);
        self.tau.clear();
        self.tau.resize(k, 0.0);
        self.qt_b.clear();
        self.qt_b.resize(n, 0.0);
        self.beta.clear();
        self.beta.resize(k, 0.0);

        // Build Vandermonde on normalized index t=i/(n-1) to reduce conditioning issues.
        let denom = (n - 1) as f64;
        if denom <= 0.0 {
            return Err(MathError::InvalidData(
                "detrend_polynomial: invalid normalization denominator".to_string(),
            ));
        }
        let mut sum_sq = 0.0f64;
        for i in 0..n {
            let t = (i as f64) / denom;
            let mut p = 1.0f64;
            for j in 0..k {
                let v = p;
                self.a_qr[i * k + j] = v;
                sum_sq += v * v;
                p *= t;
            }
        }
        if !sum_sq.is_finite() || sum_sq <= 0.0 {
            return Err(MathError::NumericalInstability(
                "detrend_polynomial: non-finite design norm".to_string(),
            ));
        }
        let norm_a = sum_sq.sqrt();
        self.rank_tol = default_rank_tol(n, k, norm_a);

        qr_decompose_in_place(&mut self.a_qr, n, k, &mut self.tau)?;
        Ok(())
    }
}

fn validate_values(values: &[f64]) -> MathResult<()> {
    if values.is_empty() {
        return Err(MathError::InsufficientDataAlgo {
            required: 1,
            actual: 0,
        });
    }
    if values.iter().any(|v| !v.is_finite()) {
        return Err(MathError::InvalidData(
            "detrend: all values must be finite".to_string(),
        ));
    }
    Ok(())
}

/// Detrend a signal by subtracting a fitted trend.
///
/// Semantics:
/// - `None`: returns a copy of the input.
/// - `RemoveMean`: subtracts the sample mean.
/// - `RemoveLinear`: subtracts the OLS best-fit line `a + b*i` for `i=0..n-1`.
/// - `RemovePolynomial{degree}`: fits a polynomial in normalized index `t=i/(n-1)` via QR least squares.
pub fn detrend(values: &[f64], method: DetrendMethod) -> MathResult<Vec<f64>> {
    validate_values(values)?;
    let mut out = vec![0.0f64; values.len()];
    detrend_into(values, method, &mut out)?;
    Ok(out)
}

pub fn detrend_into(values: &[f64], method: DetrendMethod, out: &mut [f64]) -> MathResult<()> {
    validate_values(values)?;
    if out.len() != values.len() {
        return Err(MathError::InvalidParameter {
            parameter: "out".to_string(),
            value: out.len() as f64,
            constraint: format!("must have length n = {}", values.len()),
        });
    }

    match method {
        DetrendMethod::None => {
            out.copy_from_slice(values);
            Ok(())
        }
        DetrendMethod::RemoveMean => detrend_mean_into(values, out),
        DetrendMethod::RemoveLinear => detrend_linear_into(values, out),
        DetrendMethod::RemovePolynomial { degree } => {
            let mut ws = PolynomialDetrendWorkspace::with_capacity(values.len(), degree)?;
            detrend_polynomial_into_with_workspace(values, degree, out, &mut ws)
        }
    }
}

fn detrend_mean_into(values: &[f64], out: &mut [f64]) -> MathResult<()> {
    let mean = safe_mean(values)?;
    for (o, &v) in out.iter_mut().zip(values.iter()) {
        *o = v - mean;
        if !o.is_finite() {
            return Err(MathError::NumericalError {
                reason: "detrend_mean: non-finite output".to_string(),
                operation: Some("detrend_mean_into".to_string()),
            });
        }
    }
    Ok(())
}

fn detrend_linear_into(values: &[f64], out: &mut [f64]) -> MathResult<()> {
    let n = values.len();
    if n < 2 {
        return Err(MathError::InsufficientDataAlgo {
            required: 2,
            actual: n,
        });
    }

    // Stable centered formulation for x=i, i=0..n-1.
    let n_f = n as f64;
    let x_mean = (n_f - 1.0) / 2.0;
    let y_mean = safe_mean(values)?;

    let mut sxx = 0.0f64;
    let mut sxy = 0.0f64;
    for (i, &y) in values.iter().enumerate() {
        let x = (i as f64) - x_mean;
        let yc = y - y_mean;
        sxx += x * x;
        sxy += x * yc;
    }
    if !(sxx.is_finite() && sxy.is_finite()) {
        return Err(MathError::NumericalInstability(
            "detrend_linear: non-finite intermediate".to_string(),
        ));
    }
    if sxx <= 0.0 {
        return Err(MathError::NumericalInstability(
            "detrend_linear: zero variance in index".to_string(),
        ));
    }

    let slope = sxy / sxx;
    let intercept = y_mean - slope * x_mean;
    if !(slope.is_finite() && intercept.is_finite()) {
        return Err(MathError::NumericalError {
            reason: "detrend_linear: non-finite coefficients".to_string(),
            operation: Some("detrend_linear_into".to_string()),
        });
    }

    for (i, (o, &y)) in out.iter_mut().zip(values.iter()).enumerate() {
        let fitted = intercept + slope * (i as f64);
        *o = y - fitted;
        if !o.is_finite() {
            return Err(MathError::NumericalError {
                reason: "detrend_linear: non-finite output".to_string(),
                operation: Some("detrend_linear_into".to_string()),
            });
        }
    }
    Ok(())
}

pub fn detrend_polynomial_into_with_workspace(
    values: &[f64],
    degree: usize,
    out: &mut [f64],
    workspace: &mut PolynomialDetrendWorkspace,
) -> MathResult<()> {
    validate_values(values)?;
    if out.len() != values.len() {
        return Err(MathError::InvalidParameter {
            parameter: "out".to_string(),
            value: out.len() as f64,
            constraint: format!("must have length n = {}", values.len()),
        });
    }

    let n = values.len();
    if degree == 0 {
        return detrend_mean_into(values, out);
    }
    if degree == 1 {
        return detrend_linear_into(values, out);
    }
    if degree >= n {
        return Err(MathError::InvalidParameter {
            parameter: "degree".to_string(),
            value: degree as f64,
            constraint: format!("must be < n={n}"),
        });
    }

    let k = workspace.prepare(n, degree)?;
    if k > n {
        return Err(MathError::InvalidParameter {
            parameter: "matrix_shape".to_string(),
            value: k as f64,
            constraint: format!("underdetermined system: n_features={k} > n_obs={n}"),
        });
    }

    // Build Vandermonde on normalized index t=i/(n-1) to reduce conditioning issues.
    let denom = (n - 1) as f64;
    if denom <= 0.0 {
        return Err(MathError::InvalidData(
            "detrend_polynomial: invalid normalization denominator".to_string(),
        ));
    }

    for i in 0..n {
        let t = (i as f64) / denom;
        let mut p = 1.0f64;
        for j in 0..k {
            workspace.a[i * k + j] = p;
            p *= t;
        }
    }

    // Solve least squares (exact solve if full rank) for coefficients beta.
    solve_least_squares_qr_into_with_workspace(
        &workspace.a,
        n,
        k,
        values,
        &mut workspace.beta,
        &mut workspace.qr,
    )?;

    for i in 0..n {
        let t = (i as f64) / denom;
        let mut p = 1.0f64;
        let mut fitted = 0.0f64;
        for j in 0..k {
            fitted += workspace.beta[j] * p;
            p *= t;
        }
        let r = values[i] - fitted;
        if !r.is_finite() {
            return Err(MathError::NumericalError {
                reason: "detrend_polynomial: non-finite output".to_string(),
                operation: Some("detrend_polynomial_into_with_workspace".to_string()),
            });
        }
        out[i] = r;
    }
    Ok(())
}

pub fn detrend_polynomial_precomputed_into_with_workspace(
    values: &[f64],
    degree: usize,
    out: &mut [f64],
    workspace: &mut PolynomialDetrendPrecomputedWorkspace,
) -> MathResult<()> {
    validate_values(values)?;
    if out.len() != values.len() {
        return Err(MathError::InvalidParameter {
            parameter: "out".to_string(),
            value: out.len() as f64,
            constraint: format!("must have length n = {}", values.len()),
        });
    }
    let n = values.len();
    if degree < 2 {
        return Err(MathError::InvalidParameter {
            parameter: "degree".to_string(),
            value: degree as f64,
            constraint: "must be >= 2 for precomputed polynomial detrend".to_string(),
        });
    }
    if degree >= n {
        return Err(MathError::InvalidParameter {
            parameter: "degree".to_string(),
            value: degree as f64,
            constraint: format!("must be < n={n}"),
        });
    }

    let k = degree
        .checked_add(1)
        .ok_or_else(|| MathError::InvalidData("degree overflow".to_string()))?;
    if workspace.m != n || workspace.k != k {
        return Err(MathError::InvalidParameter {
            parameter: "workspace".to_string(),
            value: 0.0,
            constraint: format!(
                "workspace shape mismatch: expected (m={n}, k={k}), got (m={}, k={})",
                workspace.m, workspace.k
            ),
        });
    }

    solve_least_squares_qr_from_precomputed_qr_into(
        &workspace.a_qr,
        n,
        k,
        &workspace.tau,
        values,
        &mut workspace.beta,
        &mut workspace.qt_b,
        workspace.rank_tol,
    )?;

    // Evaluate fitted polynomial using Horner on normalized index t=i/(n-1).
    let denom = (n - 1) as f64;
    if denom <= 0.0 {
        return Err(MathError::InvalidData(
            "detrend_polynomial: invalid normalization denominator".to_string(),
        ));
    }
    for i in 0..n {
        let t = (i as f64) / denom;
        let mut fitted = 0.0f64;
        for j in (0..k).rev() {
            fitted = fitted * t + workspace.beta[j];
        }
        let r = values[i] - fitted;
        if !r.is_finite() {
            return Err(MathError::NumericalError {
                reason: "detrend_polynomial_precomputed: non-finite output".to_string(),
                operation: Some("detrend_polynomial_precomputed_into_with_workspace".to_string()),
            });
        }
        out[i] = r;
    }
    Ok(())
}
