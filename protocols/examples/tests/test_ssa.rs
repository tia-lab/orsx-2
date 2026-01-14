use crate::signal::ssa::{
    ssa_reconstruct_rank_r, ssa_reconstruct_rank_r_into_with_workspace, SsaConfig, SsaWorkspace,
};
use crate::MathError;

fn linspace_sin_with_mean(n: usize, mean: f64, cycles: f64) -> Vec<f64> {
    let mut out = vec![0.0; n];
    let two_pi = std::f64::consts::PI * 2.0;
    for i in 0..n {
        let t = i as f64 / (n.saturating_sub(1) as f64);
        out[i] = mean + (two_pi * cycles * t).sin();
    }
    out
}

#[test]
fn test_ssa_constant_series_rank1_reconstructs_exactly() {
    let n = 64;
    let x = vec![2.0; n];
    let cfg = SsaConfig {
        window_len: 20,
        rank: 1,
        center: true,
    };
    let y = ssa_reconstruct_rank_r(&x, &cfg).unwrap();
    for v in y {
        assert!((v - 2.0).abs() <= 1e-12);
    }
}

#[test]
fn test_ssa_full_rank_reconstructs_original_close() {
    let n = 60;
    let mut x = vec![0.0; n];
    for i in 0..n {
        x[i] = (i as f64) * 0.1 + (i as f64).sin() * 0.01;
    }
    let l = 25;
    let cfg = SsaConfig {
        window_len: l,
        rank: l,
        center: false,
    };
    let y = ssa_reconstruct_rank_r(&x, &cfg).unwrap();
    let mut max_abs: f64 = 0.0;
    for (a, b) in x.iter().zip(y.iter()) {
        max_abs = max_abs.max((a - b).abs());
    }
    assert!(max_abs <= 1e-8, "max_abs={max_abs}");
}

#[test]
fn test_ssa_sinusoid_centered_rank2_is_good_approximation() {
    let n = 256;
    let x = linspace_sin_with_mean(n, 1.0, 4.0);
    let cfg = SsaConfig {
        window_len: 80,
        rank: 2,
        center: true,
    };
    let y = ssa_reconstruct_rank_r(&x, &cfg).unwrap();

    let mut num = 0.0;
    let mut den = 0.0;
    for i in 0..n {
        let xt = x[i] - 1.0;
        let yt = y[i] - 1.0;
        num += (xt - yt) * (xt - yt);
        den += xt * xt;
    }
    let rmse = (num / (n as f64)).sqrt();
    let rmsx = (den / (n as f64)).sqrt();
    assert!(rmse.is_finite() && rmsx.is_finite());
    assert!(rmse <= 0.05 * rmsx, "rmse={rmse}, rmsx={rmsx}");
}

#[test]
fn test_ssa_deterministic_repeated_calls_same_output() {
    let n = 128;
    let x = linspace_sin_with_mean(n, 0.3, 3.0);
    let cfg = SsaConfig {
        window_len: 50,
        rank: 3,
        center: true,
    };
    let y1 = ssa_reconstruct_rank_r(&x, &cfg).unwrap();
    let y2 = ssa_reconstruct_rank_r(&x, &cfg).unwrap();
    for (a, b) in y1.iter().zip(y2.iter()) {
        assert!((a - b).abs() <= 1e-12);
    }
}

#[test]
fn test_ssa_rejects_invalid_params_and_nonfinite() {
    let x = vec![1.0, 2.0, 3.0, 4.0];

    // window_len too small
    let cfg = SsaConfig {
        window_len: 1,
        rank: 1,
        center: true,
    };
    assert!(matches!(
        ssa_reconstruct_rank_r(&x, &cfg),
        Err(MathError::InvalidParameter { .. })
    ));

    // window_len >= n
    let cfg = SsaConfig {
        window_len: 4,
        rank: 1,
        center: true,
    };
    assert!(matches!(
        ssa_reconstruct_rank_r(&x, &cfg),
        Err(MathError::InvalidParameter { .. })
    ));

    // rank invalid
    let cfg = SsaConfig {
        window_len: 2,
        rank: 3,
        center: true,
    };
    assert!(matches!(
        ssa_reconstruct_rank_r(&x, &cfg),
        Err(MathError::InvalidParameter { .. })
    ));

    // non-finite
    let x_nf = vec![1.0, f64::NAN, 2.0];
    let cfg = SsaConfig {
        window_len: 2,
        rank: 1,
        center: true,
    };
    assert!(matches!(
        ssa_reconstruct_rank_r(&x_nf, &cfg),
        Err(MathError::InvalidData(_))
    ));
}

#[test]
fn test_ssa_workspace_api_matches_allocating() {
    let n = 100;
    let x = linspace_sin_with_mean(n, 0.0, 2.0);
    let cfg = SsaConfig {
        window_len: 40,
        rank: 2,
        center: true,
    };

    let y_alloc = ssa_reconstruct_rank_r(&x, &cfg).unwrap();

    let mut ws = SsaWorkspace::with_capacity(cfg.window_len, n);
    let mut y_ws = vec![0.0; n];
    ssa_reconstruct_rank_r_into_with_workspace(&x, &cfg, &mut y_ws, &mut ws).unwrap();

    for (a, b) in y_alloc.iter().zip(y_ws.iter()) {
        assert!((a - b).abs() <= 1e-12);
    }
}
