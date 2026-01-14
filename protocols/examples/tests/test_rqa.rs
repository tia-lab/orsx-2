use crate::signal::rqa::{rqa_metrics_with_workspace, RqaConfig, RqaSampling, RqaWorkspace};

fn gen_seeded(n: usize, seed: u64) -> Vec<f64> {
    let mut x = Vec::with_capacity(n);
    let mut s = seed;
    for _ in 0..n {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let u = ((s >> 11) as f64) * (1.0 / ((1u64 << 53) as f64));
        x.push(2.0 * u - 1.0);
    }
    x
}

fn chebyshev_embedded_leq(x: &[f64], i: usize, j: usize, m: usize, tau: usize, eps: f64) -> bool {
    for k in 0..m {
        if (x[i + k * tau] - x[j + k * tau]).abs() > eps {
            return false;
        }
    }
    true
}

fn bruteforce_recurrence_matrix(
    x: &[f64],
    m: usize,
    tau: usize,
    eps: f64,
    include_diag: bool,
) -> Vec<Vec<bool>> {
    let n_templates = x.len() - (m - 1) * tau;
    let mut r = vec![vec![false; n_templates]; n_templates];
    for i in 0..n_templates {
        for j in 0..n_templates {
            if i == j {
                r[i][j] = include_diag;
            } else {
                r[i][j] = chebyshev_embedded_leq(x, i, j, m, tau, eps);
            }
        }
    }
    r
}

fn bruteforce_diag_stats(r: &[Vec<bool>], min_len: usize) -> (u64, u64, u64, usize) {
    let n = r.len();
    let mut points = 0u64;
    let mut lines = 0u64;
    let mut len_sum = 0u64;
    let mut max_len = 0usize;

    for offset in 1..n {
        // above main diagonal
        let mut run = 0usize;
        for i in 0..(n - offset) {
            let j = i + offset;
            if r[i][j] {
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

        // below main diagonal
        let mut run = 0usize;
        for j in 0..(n - offset) {
            let i = j + offset;
            if r[i][j] {
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

    (points, lines, len_sum, max_len)
}

fn bruteforce_horiz_stats(r: &[Vec<bool>], min_len: usize) -> (u64, u64, u64, usize) {
    let n = r.len();
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
            if r[i][j] {
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

    (points, lines, len_sum, max_len)
}

#[test]
fn test_rqa_matches_bruteforce_small_constant_series() {
    let x = vec![3.0f64; 10];
    let cfg = RqaConfig {
        embed_dim: 2,
        delay: 1,
        epsilon: 0.1,
        diag_min_len: 2,
        vert_min_len: 2,
        include_diagonal_in_recurrence_rate: false,
        sampling: RqaSampling::All,
    };

    let mut ws = RqaWorkspace::default();
    let got = rqa_metrics_with_workspace(&x, &cfg, &mut ws).unwrap();

    let r = bruteforce_recurrence_matrix(&x, cfg.embed_dim, cfg.delay, cfg.epsilon, false);
    let n = r.len();
    let denom_rr = (n as u64) * (n as u64 - 1);
    let rec_points_ex_diag = r
        .iter()
        .enumerate()
        .map(|(i, row)| {
            row.iter()
                .enumerate()
                .filter(|(j, &v)| i != *j && v)
                .count() as u64
        })
        .sum::<u64>();
    let rr = (rec_points_ex_diag as f64) / (denom_rr as f64);
    assert!((got.recurrence_rate - rr).abs() <= 1e-15);

    let (d_points, d_lines, d_len_sum, d_max) = bruteforce_diag_stats(&r, cfg.diag_min_len);
    let (h_points, h_lines, h_len_sum, h_max) = bruteforce_horiz_stats(&r, cfg.vert_min_len);

    let det = if rec_points_ex_diag == 0 {
        0.0
    } else {
        (d_points as f64) / (rec_points_ex_diag as f64)
    };
    let lam = if rec_points_ex_diag == 0 {
        0.0
    } else {
        (h_points as f64) / (rec_points_ex_diag as f64)
    };
    let avg_diag = if d_lines == 0 {
        0.0
    } else {
        (d_len_sum as f64) / (d_lines as f64)
    };
    let tt = if h_lines == 0 {
        0.0
    } else {
        (h_len_sum as f64) / (h_lines as f64)
    };

    assert!((got.determinism - det).abs() <= 1e-15);
    assert!((got.laminarity - lam).abs() <= 1e-15);
    assert!((got.avg_diag_line_len - avg_diag).abs() <= 1e-15);
    assert!((got.trapping_time - tt).abs() <= 1e-15);
    assert_eq!(got.max_diag_line_len, d_max);
    assert_eq!(got.max_trapping_time, h_max);
}

#[test]
fn test_rqa_no_recurrences_returns_zero_metrics() {
    let x: Vec<f64> = (0..50).map(|i| i as f64).collect();
    let cfg = RqaConfig {
        embed_dim: 1,
        delay: 1,
        epsilon: 1e-12,
        diag_min_len: 2,
        vert_min_len: 2,
        include_diagonal_in_recurrence_rate: false,
        sampling: RqaSampling::All,
    };

    let mut ws = RqaWorkspace::default();
    let got = rqa_metrics_with_workspace(&x, &cfg, &mut ws).unwrap();
    assert_eq!(got.recurrence_points, 0);
    assert_eq!(got.recurrence_rate, 0.0);
    assert_eq!(got.determinism, 0.0);
    assert_eq!(got.laminarity, 0.0);
    assert_eq!(got.avg_diag_line_len, 0.0);
    assert_eq!(got.max_diag_line_len, 0);
    assert_eq!(got.trapping_time, 0.0);
    assert_eq!(got.max_trapping_time, 0);
}

#[test]
fn test_rqa_subsample_is_deterministic() {
    let x = gen_seeded(10_000, 42);
    let cfg = RqaConfig {
        embed_dim: 2,
        delay: 1,
        epsilon: 0.2,
        diag_min_len: 2,
        vert_min_len: 2,
        include_diagonal_in_recurrence_rate: false,
        sampling: RqaSampling::DeterministicSubsample { max_templates: 512 },
    };

    let mut ws = RqaWorkspace::with_capacity(512);
    let a = rqa_metrics_with_workspace(&x, &cfg, &mut ws).unwrap();
    let b = rqa_metrics_with_workspace(&x, &cfg, &mut ws).unwrap();
    assert_eq!(a, b);
}

#[test]
fn test_rqa_failure_contract() {
    let x = vec![1.0f64, 2.0, 3.0, 4.0];
    let mut ws = RqaWorkspace::default();

    let bad_eps = RqaConfig {
        embed_dim: 1,
        delay: 1,
        epsilon: 0.0,
        diag_min_len: 2,
        vert_min_len: 2,
        include_diagonal_in_recurrence_rate: false,
        sampling: RqaSampling::All,
    };
    assert!(rqa_metrics_with_workspace(&x, &bad_eps, &mut ws).is_err());

    let bad_embed = RqaConfig {
        embed_dim: 9,
        ..bad_eps
    };
    assert!(rqa_metrics_with_workspace(&x, &bad_embed, &mut ws).is_err());

    let bad_delay = RqaConfig {
        embed_dim: 2,
        delay: 0,
        epsilon: 0.1,
        ..bad_eps
    };
    assert!(rqa_metrics_with_workspace(&x, &bad_delay, &mut ws).is_err());

    let non_finite = vec![1.0f64, f64::NAN, 3.0, 4.0];
    let ok = RqaConfig {
        embed_dim: 1,
        delay: 1,
        epsilon: 0.1,
        ..bad_eps
    };
    assert!(rqa_metrics_with_workspace(&non_finite, &ok, &mut ws).is_err());
}
