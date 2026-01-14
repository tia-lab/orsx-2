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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RqaSampling {
    /// Use all valid template vectors (exact) and error if the series implies too many templates.
    All,
    /// Deterministically subsample template vectors to enforce a hard runtime bound.
    DeterministicSubsample { max_templates: usize },
}

#[derive(Debug, Clone)]
pub struct RqaConfig {
    pub embed_dim: usize,
    pub delay: usize,
    pub epsilon: f64,
    pub diag_min_len: usize,
    pub vert_min_len: usize,
    pub include_diagonal_in_recurrence_rate: bool,
    pub sampling: RqaSampling,
}

impl Default for RqaConfig {
    fn default() -> Self {
        Self {
            embed_dim: 2,
            delay: 1,
            epsilon: 0.5,
            diag_min_len: 2,
            vert_min_len: 2,
            include_diagonal_in_recurrence_rate: false,
            sampling: RqaSampling::DeterministicSubsample {
                max_templates: 2_048,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RqaMetrics {
    pub templates_used: usize,
    pub recurrence_points: u64,
    pub recurrence_rate: f64,
    pub determinism: f64,
    pub laminarity: f64,
    pub avg_diag_line_len: f64,
    pub max_diag_line_len: usize,
    pub trapping_time: f64,
    pub max_trapping_time: usize,
}

#[derive(Debug, Default)]
pub struct RqaWorkspace {
    template_indices: Vec<usize>,
    bits: Vec<u64>,
    words_per_row: usize,
    n_rows: usize,
}

impl RqaWorkspace {
    pub fn with_capacity(max_templates: usize) -> Self {
        let mut ws = Self::default();
        ws.ensure_capacity(max_templates);
        ws
    }

    pub fn ensure_capacity(&mut self, max_templates: usize) {
        let n_rows = max_templates.max(1);
        let words = words_per_row(n_rows);
        let required_words = n_rows.saturating_mul(words);
        if self.bits.len() < required_words {
            self.bits.resize(required_words, 0u64);
        }
        if self.template_indices.len() < n_rows {
            self.template_indices.resize(n_rows, 0usize);
        }
    }

    fn resize_for_templates(&mut self, n_templates: usize) {
        self.n_rows = n_templates;
        self.words_per_row = words_per_row(n_templates);
        let required_words = n_templates.saturating_mul(self.words_per_row);
        if self.bits.len() < required_words {
            self.bits.resize(required_words, 0u64);
        }
        self.bits[..required_words].fill(0u64);
    }
}

pub fn rqa_metrics(values: &[f64], cfg: &RqaConfig) -> MathResult<RqaMetrics> {
    let mut ws = RqaWorkspace::default();
    rqa_metrics_with_workspace(values, cfg, &mut ws)
}

pub fn rqa_metrics_with_workspace(
    values: &[f64],
    cfg: &RqaConfig,
    ws: &mut RqaWorkspace,
) -> MathResult<RqaMetrics> {
    validate_config(cfg)?;
    validate_all_finite(values, "values")?;

    let n = values.len();
    let required = (cfg.embed_dim - 1)
        .checked_mul(cfg.delay)
        .and_then(|v| v.checked_add(1))
        .ok_or_else(|| {
            MathError::InvalidData("rqa: embedding parameters overflowed usize".to_string())
        })?;
    if n < required {
        return Err(MathError::InsufficientDataAlgo {
            required,
            actual: n,
        });
    }

    let n_templates_total = n - (cfg.embed_dim - 1) * cfg.delay;
    let templates = match cfg.sampling {
        RqaSampling::All => n_templates_total,
        RqaSampling::DeterministicSubsample { max_templates } => {
            n_templates_total.min(max_templates)
        }
    };
    if templates < 2 {
        return Err(MathError::InsufficientDataAlgo {
            required: 2,
            actual: templates,
        });
    }
    if matches!(cfg.sampling, RqaSampling::All) && templates > 32_768 {
        return Err(MathError::InvalidParameter {
            parameter: "sampling".to_string(),
            value: templates as f64,
            constraint: "RqaSampling::All requires templates <= 32768 to remain time-bounded; use DeterministicSubsample".to_string(),
        });
    }

    ws.ensure_capacity(templates);
    ws.resize_for_templates(templates);
    select_template_indices(
        n_templates_total,
        templates,
        &mut ws.template_indices[..templates],
    );
    build_recurrence_bits(
        values,
        cfg,
        &ws.template_indices[..templates],
        ws.words_per_row,
        &mut ws.bits,
    )?;

    let include_diag_rr = cfg.include_diagonal_in_recurrence_rate;
    let recurrence_points_total =
        count_bits_total(ws.words_per_row, templates, &ws.bits, include_diag_rr);
    let denom_rr = if include_diag_rr {
        (templates as u64) * (templates as u64)
    } else {
        (templates as u64) * (templates as u64 - 1)
    };
    let recurrence_rate = if denom_rr == 0 {
        0.0
    } else {
        (recurrence_points_total as f64) / (denom_rr as f64)
    };

    let recurrence_points_ex_diag = count_bits_total(ws.words_per_row, templates, &ws.bits, false);

    let diag_stats = diagonal_line_stats(ws.words_per_row, templates, &ws.bits, cfg.diag_min_len);
    let horiz_stats =
        horizontal_line_stats(ws.words_per_row, templates, &ws.bits, cfg.vert_min_len);

    let (determinism, avg_diag_line_len) =
        if recurrence_points_ex_diag == 0 || diag_stats.lines == 0 {
            (0.0, 0.0)
        } else {
            (
                (diag_stats.points as f64) / (recurrence_points_ex_diag as f64),
                (diag_stats.len_sum as f64) / (diag_stats.lines as f64),
            )
        };

    let (laminarity, trapping_time) = if recurrence_points_ex_diag == 0 || horiz_stats.lines == 0 {
        (0.0, 0.0)
    } else {
        (
            (horiz_stats.points as f64) / (recurrence_points_ex_diag as f64),
            (horiz_stats.len_sum as f64) / (horiz_stats.lines as f64),
        )
    };

    Ok(RqaMetrics {
        templates_used: templates,
        recurrence_points: recurrence_points_total,
        recurrence_rate,
        determinism,
        laminarity,
        avg_diag_line_len,
        max_diag_line_len: diag_stats.max_len,
        trapping_time,
        max_trapping_time: horiz_stats.max_len,
    })
}

fn validate_config(cfg: &RqaConfig) -> MathResult<()> {
    if cfg.embed_dim < 1 {
        return Err(MathError::InvalidParameter {
            parameter: "embed_dim".to_string(),
            value: cfg.embed_dim as f64,
            constraint: "embed_dim >= 1".to_string(),
        });
    }
    if cfg.embed_dim > 8 {
        return Err(MathError::InvalidParameter {
            parameter: "embed_dim".to_string(),
            value: cfg.embed_dim as f64,
            constraint: "embed_dim <= 8 (time-bounded cap)".to_string(),
        });
    }
    if cfg.delay < 1 {
        return Err(MathError::InvalidParameter {
            parameter: "delay".to_string(),
            value: cfg.delay as f64,
            constraint: "delay >= 1".to_string(),
        });
    }
    if !(cfg.epsilon.is_finite() && cfg.epsilon > 0.0) {
        return Err(MathError::InvalidParameter {
            parameter: "epsilon".to_string(),
            value: cfg.epsilon,
            constraint: "epsilon must be finite and > 0".to_string(),
        });
    }
    if cfg.diag_min_len < 2 {
        return Err(MathError::InvalidParameter {
            parameter: "diag_min_len".to_string(),
            value: cfg.diag_min_len as f64,
            constraint: "diag_min_len >= 2".to_string(),
        });
    }
    if cfg.vert_min_len < 2 {
        return Err(MathError::InvalidParameter {
            parameter: "vert_min_len".to_string(),
            value: cfg.vert_min_len as f64,
            constraint: "vert_min_len >= 2".to_string(),
        });
    }
    if let RqaSampling::DeterministicSubsample { max_templates } = cfg.sampling {
        if max_templates < 2 {
            return Err(MathError::InvalidParameter {
                parameter: "sampling.max_templates".to_string(),
                value: max_templates as f64,
                constraint: "max_templates >= 2".to_string(),
            });
        }
    }
    Ok(())
}

fn validate_all_finite(values: &[f64], parameter: &str) -> MathResult<()> {
    if values.iter().any(|v| !v.is_finite()) {
        return Err(MathError::InvalidData(format!(
            "{parameter}: all values must be finite"
        )));
    }
    Ok(())
}

fn words_per_row(n: usize) -> usize {
    (n + 63) >> 6
}

fn select_template_indices(n_templates_total: usize, k: usize, out: &mut [usize]) {
    debug_assert!(k >= 2);
    debug_assert!(out.len() >= k);
    if k == n_templates_total {
        for (i, slot) in out[..k].iter_mut().enumerate() {
            *slot = i;
        }
        return;
    }
    let last = n_templates_total - 1;
    let denom = (k - 1) as u64;
    for i in 0..k {
        let num = (i as u64) * (last as u64);
        out[i] = (num / denom) as usize;
    }
}

#[inline]
fn set_bit(bits: &mut [u64], words: usize, i: usize, j: usize) {
    let idx = i * words + (j >> 6);
    bits[idx] |= 1u64 << (j & 63);
}

#[inline]
fn get_bit(bits: &[u64], words: usize, i: usize, j: usize) -> bool {
    let idx = i * words + (j >> 6);
    ((bits[idx] >> (j & 63)) & 1u64) != 0
}

fn build_recurrence_bits(
    values: &[f64],
    cfg: &RqaConfig,
    template_idx: &[usize],
    words: usize,
    bits: &mut [u64],
) -> MathResult<()> {
    let m = cfg.embed_dim;
    let tau = cfg.delay;
    let eps = cfg.epsilon;

    let n_templates = template_idx.len();
    for a in 0..n_templates {
        let ia = template_idx[a];
        if cfg.include_diagonal_in_recurrence_rate {
            set_bit(bits, words, a, a);
        }
        for b in (a + 1)..n_templates {
            let ib = template_idx[b];
            if embedded_chebyshev_leq(values, ia, ib, m, tau, eps) {
                set_bit(bits, words, a, b);
                set_bit(bits, words, b, a);
            }
        }
    }
    Ok(())
}

#[inline]
fn embedded_chebyshev_leq(
    values: &[f64],
    i: usize,
    j: usize,
    m: usize,
    tau: usize,
    eps: f64,
) -> bool {
    for k in 0..m {
        let a = values[i + k * tau];
        let b = values[j + k * tau];
        if (a - b).abs() > eps {
            return false;
        }
    }
    true
}

fn count_bits_total(words: usize, n: usize, bits: &[u64], include_diag: bool) -> u64 {
    if n == 0 {
        return 0;
    }
    let mut total = 0u64;
    for i in 0..n {
        let row = &bits[i * words..(i + 1) * words];
        let mut row_count = 0u64;
        for &w in row {
            row_count += w.count_ones() as u64;
        }
        if !include_diag && get_bit(bits, words, i, i) {
            row_count -= 1;
        }
        total += row_count;
    }
    total
}

#[derive(Debug, Clone, Copy)]
struct LineStats {
    points: u64,
    lines: u64,
    len_sum: u64,
    max_len: usize,
}

fn diagonal_line_stats(words: usize, n: usize, bits: &[u64], min_len: usize) -> LineStats {
    let mut points = 0u64;
    let mut lines = 0u64;
    let mut len_sum = 0u64;
    let mut max_len = 0usize;

    // Scan all off-diagonal diagonals (both above and below the main diagonal).
    for offset in 1..n {
        // above: (i, i+offset)
        let mut run = 0usize;
        let mut i = 0usize;
        let mut j = offset;
        while j < n {
            if get_bit(bits, words, i, j) {
                run += 1;
            } else {
                if run >= min_len {
                    points += run as u64;
                    lines += 1;
                    len_sum += run as u64;
                    max_len = max_len.max(run);
                }
                run = 0;
            }
            i += 1;
            j += 1;
        }
        if run >= min_len {
            points += run as u64;
            lines += 1;
            len_sum += run as u64;
            max_len = max_len.max(run);
        }

        // below: (i+offset, i)
        let mut run = 0usize;
        let mut i = offset;
        let mut j = 0usize;
        while i < n {
            if get_bit(bits, words, i, j) {
                run += 1;
            } else {
                if run >= min_len {
                    points += run as u64;
                    lines += 1;
                    len_sum += run as u64;
                    max_len = max_len.max(run);
                }
                run = 0;
            }
            i += 1;
            j += 1;
        }
        if run >= min_len {
            points += run as u64;
            lines += 1;
            len_sum += run as u64;
            max_len = max_len.max(run);
        }
    }

    LineStats {
        points,
        lines,
        len_sum,
        max_len,
    }
}

fn horizontal_line_stats(words: usize, n: usize, bits: &[u64], min_len: usize) -> LineStats {
    let mut points = 0u64;
    let mut lines = 0u64;
    let mut len_sum = 0u64;
    let mut max_len = 0usize;

    for i in 0..n {
        let mut run = 0usize;
        for j in 0..n {
            if i == j {
                if run >= min_len {
                    points += run as u64;
                    lines += 1;
                    len_sum += run as u64;
                    max_len = max_len.max(run);
                }
                run = 0;
                continue;
            }
            if get_bit(bits, words, i, j) {
                run += 1;
            } else {
                if run >= min_len {
                    points += run as u64;
                    lines += 1;
                    len_sum += run as u64;
                    max_len = max_len.max(run);
                }
                run = 0;
            }
        }
        if run >= min_len {
            points += run as u64;
            lines += 1;
            len_sum += run as u64;
            max_len = max_len.max(run);
        }
    }

    LineStats {
        points,
        lines,
        len_sum,
        max_len,
    }
}
