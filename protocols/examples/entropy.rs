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

//! Deterministic entropy / complexity primitives for scalar time series.

use crate::{MathError, MathResult};
use rayon::prelude::*;
use std::collections::HashMap;

const MAX_EMBEDDING_DIM: usize = 8;

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

fn factorial_usize(n: usize) -> usize {
    let mut out = 1usize;
    for k in 2..=n {
        out = out.saturating_mul(k);
    }
    out
}

fn validate_permutation_entropy_params(n: usize, m: usize, tau: usize) -> MathResult<usize> {
    if m < 2 {
        return Err(MathError::InvalidParameter {
            parameter: "m".to_string(),
            value: m as f64,
            constraint: "must be >= 2".to_string(),
        });
    }
    if m > MAX_EMBEDDING_DIM {
        return Err(MathError::InvalidParameter {
            parameter: "m".to_string(),
            value: m as f64,
            constraint: format!("must be <= {MAX_EMBEDDING_DIM}"),
        });
    }
    if tau == 0 {
        return Err(MathError::InvalidParameter {
            parameter: "tau".to_string(),
            value: 0.0,
            constraint: "must be >= 1".to_string(),
        });
    }
    let span = (m - 1) * tau;
    if n <= span {
        return Err(MathError::InsufficientDataAlgo {
            required: span + 1,
            actual: n,
        });
    }
    Ok(n - span)
}

fn validate_sampen_params(n: usize, m: usize, tau: usize, r: f64) -> MathResult<(usize, usize)> {
    if m < 1 {
        return Err(MathError::InvalidParameter {
            parameter: "m".to_string(),
            value: m as f64,
            constraint: "must be >= 1".to_string(),
        });
    }
    if m >= MAX_EMBEDDING_DIM {
        return Err(MathError::InvalidParameter {
            parameter: "m".to_string(),
            value: m as f64,
            constraint: format!("must be <= {}", MAX_EMBEDDING_DIM - 1),
        });
    }
    if tau == 0 {
        return Err(MathError::InvalidParameter {
            parameter: "tau".to_string(),
            value: 0.0,
            constraint: "must be >= 1".to_string(),
        });
    }
    if !r.is_finite() {
        return Err(MathError::InvalidParameter {
            parameter: "r".to_string(),
            value: r,
            constraint: "must be finite".to_string(),
        });
    }
    if r < 0.0 {
        return Err(MathError::InvalidParameter {
            parameter: "r".to_string(),
            value: r,
            constraint: "must be >= 0".to_string(),
        });
    }
    let span_m = (m - 1) * tau;
    let span_m1 = m * tau;
    if n <= span_m1 {
        return Err(MathError::InsufficientDataAlgo {
            required: span_m1 + 1,
            actual: n,
        });
    }
    let nm = n - span_m;
    let nm1 = n - span_m1;
    Ok((nm, nm1))
}

#[derive(Debug, Default, Clone)]
pub struct PermutationEntropyWorkspace {
    counts: Vec<u64>,
    values: Vec<f64>,
    indices: Vec<usize>,
    used: [bool; MAX_EMBEDDING_DIM],
    code: [usize; MAX_EMBEDDING_DIM],
}

impl PermutationEntropyWorkspace {
    pub fn with_capacity(m: usize) -> Self {
        Self {
            counts: Vec::with_capacity(factorial_usize(m)),
            values: Vec::with_capacity(m),
            indices: Vec::with_capacity(m),
            used: [false; MAX_EMBEDDING_DIM],
            code: [0usize; MAX_EMBEDDING_DIM],
        }
    }

    fn prepare(&mut self, m: usize) -> MathResult<()> {
        if m > MAX_EMBEDDING_DIM {
            return Err(MathError::InvalidParameter {
                parameter: "m".to_string(),
                value: m as f64,
                constraint: format!("must be <= {MAX_EMBEDDING_DIM}"),
            });
        }
        let k = factorial_usize(m);
        self.counts.clear();
        self.counts.resize(k, 0);
        self.values.clear();
        self.values.resize(m, 0.0);
        self.indices.clear();
        self.indices.resize(m, 0);
        Ok(())
    }
}

fn permutation_to_lehmer_index(
    perm: &[usize],
    used: &mut [bool; MAX_EMBEDDING_DIM],
    code: &mut [usize; MAX_EMBEDDING_DIM],
) -> usize {
    let m = perm.len();
    used.fill(false);

    for i in 0..m {
        let p = perm[i];
        let mut smaller_unused = 0usize;
        for v in 0..p {
            if !used[v] {
                smaller_unused += 1;
            }
        }
        code[i] = smaller_unused;
        used[p] = true;
    }

    let mut idx = 0usize;
    for i in 0..m {
        let f = factorial_usize(m - 1 - i);
        idx = idx.saturating_add(code[i].saturating_mul(f));
    }
    idx
}

fn ordinal_pattern_with_ties(values: &[f64], indices: &mut [usize]) {
    for (i, idx) in indices.iter_mut().enumerate() {
        *idx = i;
    }
    indices.sort_unstable_by(|&a, &b| {
        let va = values[a];
        let vb = values[b];
        // Deterministic tie-breaker: keep increasing index order.
        match va.partial_cmp(&vb) {
            Some(std::cmp::Ordering::Less) => std::cmp::Ordering::Less,
            Some(std::cmp::Ordering::Greater) => std::cmp::Ordering::Greater,
            _ => a.cmp(&b),
        }
    });
}

/// Permutation entropy over ordinal patterns of length `m` with delay `tau`.
///
/// Returns:
/// - `entropy_nats`: `H = -Σ p_i ln p_i` in nats.
/// - `entropy_normalized`: `H / ln(m!)` in `[0,1]` (for `m>=2`).
pub fn permutation_entropy_into_with_workspace(
    data: &[f64],
    m: usize,
    tau: usize,
    workspace: &mut PermutationEntropyWorkspace,
) -> MathResult<(f64, f64)> {
    validate_finite(data, "data")?;
    let n = data.len();
    let windows = validate_permutation_entropy_params(n, m, tau)?;

    workspace.prepare(m)?;

    for start in 0..windows {
        for k in 0..m {
            workspace.values[k] = data[start + k * tau];
        }
        ordinal_pattern_with_ties(&workspace.values, &mut workspace.indices);
        // Convert sort order indices -> permutation ranks 0..m-1.
        let mut perm = [0usize; MAX_EMBEDDING_DIM];
        for (rank, &orig_idx) in workspace.indices.iter().enumerate() {
            perm[orig_idx] = rank;
        }
        let idx = permutation_to_lehmer_index(&perm[..m], &mut workspace.used, &mut workspace.code);
        if idx >= workspace.counts.len() {
            return Err(MathError::InvalidData(
                "permutation_entropy: internal index out of range".to_string(),
            ));
        }
        workspace.counts[idx] = workspace.counts[idx].saturating_add(1);
    }

    let total = windows as f64;
    let mut h = 0.0f64;
    for &c in workspace.counts.iter() {
        if c == 0 {
            continue;
        }
        let p = (c as f64) / total;
        h -= p * p.ln();
    }
    if !h.is_finite() || h < 0.0 {
        return Err(MathError::NumericalError {
            reason: "permutation_entropy: non-finite or negative entropy".to_string(),
            operation: Some("permutation_entropy_into_with_workspace".to_string()),
        });
    }
    let ln_fact = (factorial_usize(m) as f64).ln();
    if !(ln_fact.is_finite() && ln_fact > 0.0) {
        return Err(MathError::NumericalError {
            reason: "permutation_entropy: invalid ln(m!)".to_string(),
            operation: Some("permutation_entropy_into_with_workspace".to_string()),
        });
    }
    let hn = (h / ln_fact).clamp(0.0, 1.0);
    Ok((h, hn))
}

pub fn permutation_entropy(data: &[f64], m: usize, tau: usize) -> MathResult<(f64, f64)> {
    let mut ws = PermutationEntropyWorkspace::with_capacity(m);
    permutation_entropy_into_with_workspace(data, m, tau, &mut ws)
}

#[derive(Debug, Default, Clone)]
pub struct SampleEntropyWorkspace {
    idx_m: Vec<usize>,
    idx_m1: Vec<usize>,
    // Used only for the exact r=0 fast path (group identical embedded vectors).
    groups: HashMap<EmbeddedKey, u32>,
    // Used for the exact grid/box-hashing fast path (Chebyshev metric).
    cell_map: HashMap<CellKey, Vec<usize>>,
    cell_keys: Vec<CellKey>,
    neighbor_offsets: Vec<[i8; MAX_EMBEDDING_DIM]>,
}

impl SampleEntropyWorkspace {
    pub fn with_capacity(n: usize) -> Self {
        Self {
            idx_m: Vec::with_capacity(n),
            idx_m1: Vec::with_capacity(n),
            groups: HashMap::new(),
            cell_map: HashMap::new(),
            cell_keys: Vec::with_capacity(n),
            neighbor_offsets: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct EmbeddedKey {
    dim: u8,
    // Uses only the first `dim` entries; the rest is zero.
    bits: [u64; MAX_EMBEDDING_DIM],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CellKey {
    dim: u8,
    coords: [i64; MAX_EMBEDDING_DIM],
}

fn cell_key_coords(data: &[f64], start: usize, dim: usize, tau: usize, r: f64) -> CellKey {
    debug_assert!(r > 0.0 && r.is_finite());
    let inv_r = 1.0 / r;
    let mut coords = [0i64; MAX_EMBEDDING_DIM];
    for k in 0..dim {
        let v = data[start + k * tau];
        coords[k] = (v * inv_r).floor() as i64;
    }
    CellKey {
        dim: dim as u8,
        coords,
    }
}

fn embedded_key_bits(data: &[f64], start: usize, dim: usize, tau: usize) -> EmbeddedKey {
    let mut bits = [0u64; MAX_EMBEDDING_DIM];
    for k in 0..dim {
        bits[k] = data[start + k * tau].to_bits();
    }
    EmbeddedKey {
        dim: dim as u8,
        bits,
    }
}

fn count_pairs_from_group_counts(map: &HashMap<EmbeddedKey, u32>) -> u64 {
    map.values()
        .map(|&c| {
            let c = c as u64;
            c.saturating_mul(c.saturating_sub(1)) / 2
        })
        .sum()
}

fn all_pairs_match(data: &[f64], r: f64) -> bool {
    debug_assert!(data.iter().all(|v| v.is_finite()));
    let mut min_v = f64::INFINITY;
    let mut max_v = f64::NEG_INFINITY;
    for &v in data {
        if v < min_v {
            min_v = v;
        }
        if v > max_v {
            max_v = v;
        }
    }
    (max_v - min_v) <= r
}

fn prepare_neighbor_offsets(dim: usize, out: &mut Vec<[i8; MAX_EMBEDDING_DIM]>) {
    out.clear();
    let count = 3usize.pow(dim as u32);
    out.reserve(count);
    for mut code in 0..count {
        let mut off = [0i8; MAX_EMBEDDING_DIM];
        for k in 0..dim {
            let trit = code % 3;
            code /= 3;
            off[k] = match trit {
                0 => -1,
                1 => 0,
                _ => 1,
            };
        }
        out.push(off);
    }
}

fn count_sampen_matches_sorted_window(
    data: &[f64],
    dim: usize,
    tau: usize,
    r: f64,
    n_templates: usize,
    idx: &mut Vec<usize>,
) -> MathResult<u64> {
    debug_assert!(dim >= 1);
    idx.clear();
    idx.extend(0..n_templates);
    idx.sort_unstable_by(|&a, &b| data[a].total_cmp(&data[b]));

    let idx_slice: &[usize] = &idx[..];

    let count = if n_templates < 256 {
        let mut total = 0u64;
        for pos_a in 0..n_templates {
            let i = idx_slice[pos_a];
            let xi0 = data[i];
            for pos_b in (pos_a + 1)..n_templates {
                let j = idx_slice[pos_b];
                let d0 = data[j] - xi0;
                if d0 > r {
                    break;
                }
                let mut ok = true;
                for k in 1..dim {
                    let di = data[i + k * tau];
                    let dj = data[j + k * tau];
                    if (di - dj).abs() > r {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    total += 1;
                }
            }
        }
        total
    } else {
        // Deterministic: we sum integer counts; the final result does not depend on reduction order.
        (0..n_templates)
            .into_par_iter()
            .map(|pos_a| {
                let i = idx_slice[pos_a];
                let xi0 = data[i];
                let mut local = 0u64;
                for pos_b in (pos_a + 1)..n_templates {
                    let j = idx_slice[pos_b];
                    // Exact pruning on the first component: since sorted by x0, abs(x0_j-x0_i)=x0_j-x0_i.
                    let d0 = data[j] - xi0;
                    if d0 > r {
                        break;
                    }
                    let mut ok = true;
                    for k in 1..dim {
                        let di = data[i + k * tau];
                        let dj = data[j + k * tau];
                        if (di - dj).abs() > r {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        local += 1;
                    }
                }
                local
            })
            .sum()
    };

    Ok(count)
}

fn count_sampen_matches_grid_exact(
    data: &[f64],
    dim: usize,
    tau: usize,
    r: f64,
    n_templates: usize,
    workspace: &mut SampleEntropyWorkspace,
) -> MathResult<u64> {
    debug_assert!(dim >= 1);
    debug_assert!(r > 0.0 && r.is_finite());

    workspace.cell_map.clear();
    workspace.cell_keys.clear();
    workspace.cell_keys.reserve(n_templates);
    prepare_neighbor_offsets(dim, &mut workspace.neighbor_offsets);

    // Build cell keys and map from cell -> template indices (in increasing index order).
    for i in 0..n_templates {
        let key = cell_key_coords(data, i, dim, tau, r);
        workspace.cell_keys.push(key);
        workspace.cell_map.entry(key).or_default().push(i);
    }

    let mut count = 0u64;
    for i in 0..n_templates {
        let key_i = workspace.cell_keys[i];

        for off in workspace.neighbor_offsets.iter() {
            let mut neighbor = key_i;
            for k in 0..dim {
                neighbor.coords[k] = neighbor.coords[k].saturating_add(off[k] as i64);
            }
            if let Some(list) = workspace.cell_map.get(&neighbor) {
                for &j in list.iter() {
                    if j <= i {
                        continue;
                    }
                    // Exact Chebyshev check.
                    let mut ok = true;
                    for k in 0..dim {
                        let di = data[i + k * tau];
                        let dj = data[j + k * tau];
                        if (di - dj).abs() > r {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        count += 1;
                    }
                }
            }
        }
    }

    Ok(count)
}

fn estimate_data_range(data: &[f64]) -> (f64, f64) {
    let mut min_v = f64::INFINITY;
    let mut max_v = f64::NEG_INFINITY;
    for &v in data {
        if v < min_v {
            min_v = v;
        }
        if v > max_v {
            max_v = v;
        }
    }
    (min_v, max_v)
}

fn should_use_grid_fast_path(dim: usize, n_templates: usize, data: &[f64], r: f64) -> bool {
    // Grid/box hashing has a fixed overhead (hashing + neighbor enumeration). It only wins when
    // the expected candidate set per point is small (i.e., r is small relative to the data range).
    if dim > 3 || n_templates < 2_000 {
        return false;
    }
    let (min_v, max_v) = estimate_data_range(data);
    let range = max_v - min_v;
    if !(range.is_finite() && range > 0.0) {
        return false;
    }
    if range <= r {
        return false;
    }

    // Baseline (sorted-window) checks pairs within |x0_j-x0_i|<=r on first coordinate.
    let p0 = (2.0 * r / range).clamp(0.0, 1.0);
    let baseline_est = p0 * ((n_templates as f64) * ((n_templates - 1) as f64) * 0.5);

    // Grid estimate: occupancy per cell ~ n / bins^dim, candidates per point ~ 3^dim * occupancy.
    // Include a conservative overhead multiplier for hashing/neighbors.
    let bins = (range / r).floor().max(1.0) + 1.0;
    let bins_pow = bins.powi(dim as i32);
    let grid_est = (3usize.pow(dim as u32) as f64) * (n_templates as f64) * (n_templates as f64)
        / bins_pow.max(1.0);
    let grid_overhead = 10.0;

    (grid_est * grid_overhead) < baseline_est
}

fn sample_entropy_chebyshev_exact_baseline(
    data: &[f64],
    m: usize,
    tau: usize,
    r: f64,
    workspace: &mut SampleEntropyWorkspace,
) -> MathResult<f64> {
    // This is the previous exact method (sorted-window pruning), kept for equivalence testing and fallback.
    let n = data.len();
    let (nm, nm1) = validate_sampen_params(n, m, tau, r)?;
    if nm < 2 || nm1 < 2 {
        return Err(MathError::InsufficientDataAlgo {
            required: 2,
            actual: nm.min(nm1),
        });
    }
    let b = count_sampen_matches_sorted_window(data, m, tau, r, nm, &mut workspace.idx_m)?;
    let a = count_sampen_matches_sorted_window(data, m + 1, tau, r, nm1, &mut workspace.idx_m1)?;
    if b == 0 {
        return Err(MathError::CalculationError(
            "sample_entropy: no matches for embedding length m".to_string(),
        ));
    }
    if a == 0 {
        return Err(MathError::CalculationError(
            "sample_entropy: no matches for embedding length m+1".to_string(),
        ));
    }
    let ratio = (a as f64) / (b as f64);
    if !(ratio.is_finite() && ratio > 0.0) {
        return Err(MathError::NumericalError {
            reason: "sample_entropy: invalid A/B ratio".to_string(),
            operation: Some("sample_entropy_chebyshev_exact_baseline".to_string()),
        });
    }
    let out = -ratio.ln();
    if !out.is_finite() || out < 0.0 {
        return Err(MathError::NumericalError {
            reason: "sample_entropy: non-finite or negative result".to_string(),
            operation: Some("sample_entropy_chebyshev_exact_baseline".to_string()),
        });
    }
    Ok(out)
}

/// Exact SampEn computed via sorted-window pruning (baseline exact method).
///
/// This is exposed to support deterministic benchmarking and cross-checking.
#[doc(hidden)]
pub fn sample_entropy_chebyshev_exact_sorted_window(
    data: &[f64],
    m: usize,
    tau: usize,
    r: f64,
    workspace: &mut SampleEntropyWorkspace,
) -> MathResult<f64> {
    validate_finite(data, "data")?;
    sample_entropy_chebyshev_exact_baseline(data, m, tau, r, workspace)
}

/// Exact SampEn computed via grid/box hashing in embedded space (Chebyshev metric).
///
/// This can be faster when `r` is small relative to the data range and matches are sparse.
/// It is exposed to support deterministic benchmarking and cross-checking.
#[doc(hidden)]
pub fn sample_entropy_chebyshev_exact_grid(
    data: &[f64],
    m: usize,
    tau: usize,
    r: f64,
    workspace: &mut SampleEntropyWorkspace,
) -> MathResult<f64> {
    validate_finite(data, "data")?;
    if !(r.is_finite() && r > 0.0) {
        return Err(MathError::InvalidParameter {
            parameter: "r".to_string(),
            value: r,
            constraint: "r must be finite and > 0".to_string(),
        });
    }
    if m < 1 || m >= MAX_EMBEDDING_DIM {
        return Err(MathError::InvalidParameter {
            parameter: "m".to_string(),
            value: m as f64,
            constraint: format!("must be in [1, {}]", MAX_EMBEDDING_DIM - 1),
        });
    }
    let n = data.len();
    let (nm, nm1) = validate_sampen_params(n, m, tau, r)?;
    if nm < 2 || nm1 < 2 {
        return Err(MathError::InsufficientDataAlgo {
            required: 2,
            actual: nm.min(nm1),
        });
    }
    if m + 1 > 3 {
        return Err(MathError::InvalidParameter {
            parameter: "m".to_string(),
            value: m as f64,
            constraint: "grid exact method is implemented only for m<=2 (dim<=3)".to_string(),
        });
    }

    let b = count_sampen_matches_grid_exact(data, m, tau, r, nm, workspace)?;
    let a = count_sampen_matches_grid_exact(data, m + 1, tau, r, nm1, workspace)?;
    if b == 0 {
        return Err(MathError::CalculationError(
            "sample_entropy: no matches for embedding length m".to_string(),
        ));
    }
    if a == 0 {
        return Err(MathError::CalculationError(
            "sample_entropy: no matches for embedding length m+1".to_string(),
        ));
    }

    let ratio = (a as f64) / (b as f64);
    if !(ratio.is_finite() && ratio > 0.0) {
        return Err(MathError::NumericalError {
            reason: "sample_entropy: invalid A/B ratio".to_string(),
            operation: Some("sample_entropy_chebyshev_exact_grid".to_string()),
        });
    }
    let out = -ratio.ln();
    if !out.is_finite() || out < 0.0 {
        return Err(MathError::NumericalError {
            reason: "sample_entropy: non-finite or negative result".to_string(),
            operation: Some("sample_entropy_chebyshev_exact_grid".to_string()),
        });
    }
    Ok(out)
}

/// Sample entropy (SampEn) with Chebyshev distance and embedding delay `tau`.
///
/// Definition:
/// - form template vectors of length `m` and `m+1` with delay `tau`
/// - count matching pairs (excluding self matches) within tolerance `r` using Chebyshev distance
/// - `SampEn = -ln(A/B)` where:
///   - `B` = number of matches for length `m`
///   - `A` = number of matches for length `m+1`
///
/// Notes:
/// - Deterministic: counts are integer; parallel reduction is order-independent.
pub fn sample_entropy_chebyshev(
    data: &[f64],
    m: usize,
    tau: usize,
    r: f64,
    workspace: &mut SampleEntropyWorkspace,
) -> MathResult<f64> {
    validate_finite(data, "data")?;
    let n = data.len();
    let (nm, nm1) = validate_sampen_params(n, m, tau, r)?;
    if nm < 2 || nm1 < 2 {
        return Err(MathError::InsufficientDataAlgo {
            required: 2,
            actual: nm.min(nm1),
        });
    }

    // Exact fast paths:
    // - r == 0: exact equality in embedded space (group by template vector bits)
    // - range(data) <= r: all embedded vectors match under Chebyshev (no need to enumerate pairs)
    let (b, a) = if r == 0.0 {
        workspace.groups.clear();
        for i in 0..nm {
            let key = embedded_key_bits(data, i, m, tau);
            *workspace.groups.entry(key).or_insert(0) += 1;
        }
        let b = count_pairs_from_group_counts(&workspace.groups);

        workspace.groups.clear();
        for i in 0..nm1 {
            let key = embedded_key_bits(data, i, m + 1, tau);
            *workspace.groups.entry(key).or_insert(0) += 1;
        }
        let a = count_pairs_from_group_counts(&workspace.groups);
        (b, a)
    } else if all_pairs_match(data, r) {
        let b = (nm as u64) * (nm as u64 - 1) / 2;
        let a = (nm1 as u64) * (nm1 as u64 - 1) / 2;
        (b, a)
    } else {
        // Automatic exact method selection:
        // - Grid/box hashing can drastically reduce comparisons when matches are sparse.
        // - Sorted-window pruning is a robust baseline (also exact) and performs better for small n.
        let (b, a) = if should_use_grid_fast_path(m, nm, data, r) {
            let b = count_sampen_matches_grid_exact(data, m, tau, r, nm, workspace)?;
            let a = count_sampen_matches_grid_exact(data, m + 1, tau, r, nm1, workspace)?;
            (b, a)
        } else {
            let b = count_sampen_matches_sorted_window(data, m, tau, r, nm, &mut workspace.idx_m)?;
            let a = count_sampen_matches_sorted_window(
                data,
                m + 1,
                tau,
                r,
                nm1,
                &mut workspace.idx_m1,
            )?;
            (b, a)
        };
        (b, a)
    };

    if b == 0 {
        return Err(MathError::CalculationError(
            "sample_entropy: no matches for embedding length m".to_string(),
        ));
    }
    if a == 0 {
        return Err(MathError::CalculationError(
            "sample_entropy: no matches for embedding length m+1".to_string(),
        ));
    }

    let ratio = (a as f64) / (b as f64);
    if !(ratio.is_finite() && ratio > 0.0) {
        return Err(MathError::NumericalError {
            reason: "sample_entropy: invalid A/B ratio".to_string(),
            operation: Some("sample_entropy_chebyshev".to_string()),
        });
    }
    let out = -ratio.ln();
    if !out.is_finite() || out < 0.0 {
        return Err(MathError::NumericalError {
            reason: "sample_entropy: non-finite or negative result".to_string(),
            operation: Some("sample_entropy_chebyshev".to_string()),
        });
    }
    Ok(out)
}
