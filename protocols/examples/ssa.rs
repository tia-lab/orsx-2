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

use crate::{MathError, MathResult};
use nalgebra::{DMatrix, SymmetricEigen};
use std::cmp::Ordering;

/// SSA (Singular Spectrum Analysis) configuration.
#[derive(Debug, Clone, Copy)]
pub struct SsaConfig {
    /// Embedding/window length `L` (trajectory matrix has shape `L x (n-L+1)`).
    pub window_len: usize,
    /// Rank `r` for the rank-`r` reconstruction (`1 <= r <= L`).
    pub rank: usize,
    /// If true, subtract the mean before SSA and add it back to the reconstruction.
    pub center: bool,
}

impl SsaConfig {
    pub fn new(window_len: usize, rank: usize) -> Self {
        Self {
            window_len,
            rank,
            center: true,
        }
    }
}

impl Default for SsaConfig {
    fn default() -> Self {
        Self {
            window_len: 32,
            rank: 2,
            center: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct SsaWorkspace {
    cov: Vec<f64>,
    eigvals: Vec<f64>,
    order: Vec<usize>,
    pc: Vec<f64>,
    window_buf: Vec<f64>,
}

impl SsaWorkspace {
    pub fn with_capacity(max_window_len: usize, max_n: usize) -> Self {
        let mut ws = Self::default();
        ws.ensure_capacity(max_window_len, max_n);
        ws
    }

    pub fn ensure_capacity(&mut self, max_window_len: usize, max_n: usize) {
        let l = max_window_len.max(2);
        let cov_len = l.saturating_mul(l);
        if self.cov.len() < cov_len {
            self.cov.resize(cov_len, 0.0);
        }
        if self.eigvals.len() < l {
            self.eigvals.resize(l, 0.0);
        }
        if self.order.len() < l {
            self.order.resize(l, 0usize);
        }

        // For SSA we need a principal component of length K = n-L+1 (>=1).
        // We size this pessimistically by max_n.
        if self.pc.len() < max_n.max(1) {
            self.pc.resize(max_n.max(1), 0.0);
        }

        if self.window_buf.len() < l {
            self.window_buf.resize(l, 0.0);
        }
    }

    fn resize_for(&mut self, window_len: usize, n: usize) -> MathResult<()> {
        validate_ssa_dims(n, window_len)?;
        self.ensure_capacity(window_len, n);
        let l = window_len;
        let cov_len = l * l;
        self.cov[..cov_len].fill(0.0);
        self.eigvals[..l].fill(0.0);
        for (i, slot) in self.order[..l].iter_mut().enumerate() {
            *slot = i;
        }
        Ok(())
    }
}

pub fn ssa_reconstruct_rank_r(values: &[f64], cfg: &SsaConfig) -> MathResult<Vec<f64>> {
    let mut ws = SsaWorkspace::default();
    let mut out = vec![0.0; values.len()];
    ssa_reconstruct_rank_r_into_with_workspace(values, cfg, &mut out, &mut ws)?;
    Ok(out)
}

pub fn ssa_reconstruct_rank_r_into_with_workspace(
    values: &[f64],
    cfg: &SsaConfig,
    out: &mut [f64],
    workspace: &mut SsaWorkspace,
) -> MathResult<()> {
    validate_inputs(values)?;
    if out.len() != values.len() {
        return Err(MathError::InvalidData(format!(
            "ssa: out length mismatch (out={}, values={})",
            out.len(),
            values.len()
        )));
    }
    validate_ssa_config(values.len(), cfg)?;

    let n = values.len();
    let l = cfg.window_len;
    let k = n - l + 1;
    workspace.resize_for(l, n)?;

    let mean = if cfg.center {
        mean_finite(values)?
    } else {
        0.0
    };

    // Build the lag-covariance matrix S = X X^T where X is the trajectory matrix (Hankel).
    // S[i,j] = sum_{t=0..k-1} x[t+i] * x[t+j].
    let cov_len = l * l;
    let cov = &mut workspace.cov[..cov_len];
    let window_buf = &mut workspace.window_buf[..l];
    for t in 0..k {
        for i in 0..l {
            window_buf[i] = values[t + i] - mean;
        }
        for i in 0..l {
            let xi = window_buf[i];
            for j in i..l {
                cov[i * l + j] += xi * window_buf[j];
            }
        }
    }
    for i in 0..l {
        for j in (i + 1)..l {
            cov[j * l + i] = cov[i * l + j];
        }
    }
    if cov.iter().any(|v| !v.is_finite()) {
        return Err(MathError::NumericalError {
            reason: "ssa: non-finite covariance entry".to_string(),
            operation: Some("ssa_covariance".to_string()),
        });
    }

    // Eigendecompose symmetric covariance.
    let cov_mat = DMatrix::from_row_slice(l, l, cov);
    let eigen = SymmetricEigen::new(cov_mat);

    let eigvals_out = &mut workspace.eigvals[..l];
    for (dst, src) in eigvals_out.iter_mut().zip(eigen.eigenvalues.iter()) {
        *dst = *src;
    }
    if eigvals_out.iter().any(|v| !v.is_finite()) {
        return Err(MathError::NumericalError {
            reason: "ssa: non-finite eigenvalues".to_string(),
            operation: Some("ssa_eigendecompose".to_string()),
        });
    }

    // Sort eigenpairs descending by eigenvalue with a deterministic tie-break on index.
    let order = &mut workspace.order[..l];
    order.sort_by(|&i, &j| {
        let a = eigvals_out[i];
        let b = eigvals_out[j];
        match b.partial_cmp(&a).unwrap_or(Ordering::Equal) {
            Ordering::Equal => i.cmp(&j),
            other => other,
        }
    });

    out.fill(0.0);

    // Rank-r reconstruction via diagonal averaging of rank-1 elementary matrices.
    // For each selected component p:
    // - principal component a_p[t] = sum_{i=0..l-1} u_p[i] * x[t+i] for t=0..k-1
    // - reconstructed series y_p[s] = (1/N_s) * sum_{i} u_p[i] * a_p[s-i]
    for &col in order.iter().take(cfg.rank) {
        let pc = &mut workspace.pc[..k];
        pc.fill(0.0);

        // Compute principal component (length k).
        for t in 0..k {
            let mut acc = 0.0;
            for i in 0..l {
                let u = eigen.eigenvectors[(i, col)];
                acc += u * (values[t + i] - mean);
            }
            if !acc.is_finite() {
                return Err(MathError::NumericalError {
                    reason: "ssa: non-finite principal component".to_string(),
                    operation: Some("ssa_principal_component".to_string()),
                });
            }
            pc[t] = acc;
        }

        // Hankelization (diagonal averaging).
        for s in 0..n {
            let i_start = s.saturating_sub(k - 1);
            let i_end = (l - 1).min(s);
            let count = i_end + 1 - i_start;
            debug_assert!(count >= 1);
            let inv = 1.0 / (count as f64);
            let mut sum = 0.0;
            for i in i_start..=i_end {
                let j = s - i;
                // j in 0..k by construction.
                let u = eigen.eigenvectors[(i, col)];
                sum += u * pc[j];
            }
            out[s] += sum * inv;
        }
    }

    if cfg.center {
        for v in out.iter_mut() {
            *v += mean;
        }
    }
    if out.iter().any(|v| !v.is_finite()) {
        return Err(MathError::NumericalError {
            reason: "ssa: non-finite reconstruction".to_string(),
            operation: Some("ssa_reconstruct_rank_r".to_string()),
        });
    }
    Ok(())
}

fn validate_inputs(values: &[f64]) -> MathResult<()> {
    if values.len() < 3 {
        return Err(MathError::InsufficientDataAlgo {
            required: 3,
            actual: values.len(),
        });
    }
    if values.iter().any(|v| !v.is_finite()) {
        return Err(MathError::InvalidData(
            "ssa: all input values must be finite".to_string(),
        ));
    }
    Ok(())
}

fn validate_ssa_config(n: usize, cfg: &SsaConfig) -> MathResult<()> {
    validate_ssa_dims(n, cfg.window_len)?;
    if cfg.rank == 0 || cfg.rank > cfg.window_len {
        return Err(MathError::InvalidParameter {
            parameter: "rank".to_string(),
            value: cfg.rank as f64,
            constraint: "must satisfy 1 <= rank <= window_len".to_string(),
        });
    }
    Ok(())
}

fn validate_ssa_dims(n: usize, window_len: usize) -> MathResult<()> {
    if window_len < 2 {
        return Err(MathError::InvalidParameter {
            parameter: "window_len".to_string(),
            value: window_len as f64,
            constraint: "must be >= 2".to_string(),
        });
    }
    if window_len >= n {
        return Err(MathError::InvalidParameter {
            parameter: "window_len".to_string(),
            value: window_len as f64,
            constraint: "must be < n".to_string(),
        });
    }
    Ok(())
}

fn mean_finite(values: &[f64]) -> MathResult<f64> {
    let mut sum = 0.0;
    for &v in values {
        sum += v;
    }
    let mean = sum / (values.len() as f64);
    if !mean.is_finite() {
        return Err(MathError::NumericalError {
            reason: "ssa: mean is non-finite".to_string(),
            operation: Some("mean".to_string()),
        });
    }
    Ok(mean)
}
