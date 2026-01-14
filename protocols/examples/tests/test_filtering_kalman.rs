use crate::signal::filtering::kalman::{
    kalman_local_level_filter_into, kalman_local_linear_trend_filter_into,
};
use crate::{MathError, MathResult};

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

#[test]
fn test_kalman_local_level_failure_contracts() {
    let y = vec![1.0, 2.0, 3.0];
    let mut m = vec![0.0; 3];
    let mut v = vec![0.0; 3];

    assert!(kalman_local_level_filter_into(&y, 0.0, 0.0, 0.0, 1.0, &mut m, &mut v).is_err());
    assert!(kalman_local_level_filter_into(&y, 1.0, -1.0, 0.0, 1.0, &mut m, &mut v).is_err());
    assert!(kalman_local_level_filter_into(&y, 1.0, 0.0, f64::NAN, 1.0, &mut m, &mut v).is_err());
    assert!(kalman_local_level_filter_into(&y, 1.0, 0.0, 0.0, -1.0, &mut m, &mut v).is_err());

    let y_bad = vec![1.0, f64::NAN, 3.0];
    assert!(kalman_local_level_filter_into(&y_bad, 1.0, 0.0, 0.0, 1.0, &mut m, &mut v).is_err());

    let mut short = vec![0.0; 2];
    assert!(kalman_local_level_filter_into(&y, 1.0, 0.0, 0.0, 1.0, &mut short, &mut v).is_err());
}

#[test]
fn test_kalman_local_level_constant_signal_converges() -> MathResult<()> {
    let n = 500usize;
    let y = vec![7.0f64; n];
    let r = 1.0;
    let q = 0.0;
    let mut mean = vec![0.0f64; n];
    let mut var = vec![0.0f64; n];
    kalman_local_level_filter_into(&y, r, q, 0.0, 100.0, &mut mean, &mut var)?;

    // Converge near 7.
    let last = mean[n - 1];
    assert!((last - 7.0).abs() <= 2e-4, "last={last}");

    // Variance should decrease and stay non-negative.
    for i in 1..n {
        assert!(var[i].is_finite() && var[i] >= 0.0);
        assert!(var[i] <= var[i - 1] + 1e-12);
    }
    Ok(())
}

#[test]
fn test_kalman_local_level_determinism() -> MathResult<()> {
    let n = 1000usize;
    let y = gen_seeded(n, 11);
    let r = 0.5;
    let q = 0.1;

    let mut m1 = vec![0.0f64; n];
    let mut v1 = vec![0.0f64; n];
    let mut m2 = vec![0.0f64; n];
    let mut v2 = vec![0.0f64; n];

    kalman_local_level_filter_into(&y, r, q, 0.0, 1.0, &mut m1, &mut v1)?;
    kalman_local_level_filter_into(&y, r, q, 0.0, 1.0, &mut m2, &mut v2)?;
    assert_eq!(m1, m2);
    assert_eq!(v1, v2);
    Ok(())
}

#[test]
fn test_kalman_local_level_numerical_stability_large_offset() -> MathResult<()> {
    let n = 1000usize;
    let mut y = Vec::with_capacity(n);
    for i in 0..n {
        y.push(1e12 + (i as f64) * 1e-3);
    }
    let mut mean = vec![0.0f64; n];
    let mut var = vec![0.0f64; n];
    kalman_local_level_filter_into(&y, 1.0, 0.01, 1e12, 1.0, &mut mean, &mut var)?;
    assert!(mean.iter().all(|v| v.is_finite()));
    assert!(var.iter().all(|v| v.is_finite() && *v >= 0.0));
    Ok(())
}

#[test]
fn test_kalman_local_linear_trend_tracks_perfect_line_low_noise() -> MathResult<()> {
    let n = 300usize;
    let intercept = 10.0;
    let slope = 0.25;
    let mut y = Vec::with_capacity(n);
    for i in 0..n {
        y.push(intercept + slope * (i as f64));
    }

    let r = 1e-9;
    let q_level = 0.0;
    let q_trend = 0.0;

    let mut level = vec![0.0f64; n];
    let mut trend = vec![0.0f64; n];
    let mut var_level = vec![0.0f64; n];
    let mut var_trend = vec![0.0f64; n];

    kalman_local_linear_trend_filter_into(
        &y,
        r,
        q_level,
        q_trend,
        0.0,
        0.0,
        1e6,
        1e6,
        &mut level,
        &mut trend,
        &mut var_level,
        &mut var_trend,
    )?;

    assert!((level[n - 1] - y[n - 1]).abs() <= 1e-6);
    assert!((trend[n - 1] - slope).abs() <= 1e-6);
    assert!(var_level[n - 1] >= 0.0 && var_trend[n - 1] >= 0.0);
    Ok(())
}

#[test]
fn test_kalman_local_linear_trend_failure_contracts() {
    let y = vec![1.0, 2.0, 3.0];
    let mut level = vec![0.0; 3];
    let mut trend = vec![0.0; 3];
    let mut vl = vec![0.0; 3];
    let mut vt = vec![0.0; 3];

    let err = kalman_local_linear_trend_filter_into(
        &y, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, &mut level, &mut trend, &mut vl, &mut vt,
    )
    .unwrap_err();
    assert!(matches!(err, MathError::InvalidParameter { .. }));

    let y_bad = vec![1.0, f64::NAN, 3.0];
    let err = kalman_local_linear_trend_filter_into(
        &y_bad, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, &mut level, &mut trend, &mut vl, &mut vt,
    )
    .unwrap_err();
    assert!(matches!(err, MathError::InvalidData(_)));
}

#[test]
fn test_kalman_no_panic_on_error_paths() {
    let r = std::panic::catch_unwind(|| {
        let y = vec![1.0, 2.0, 3.0];
        let mut m = vec![0.0; 3];
        let mut v = vec![0.0; 3];
        let _ = kalman_local_level_filter_into(&y, 0.0, 0.0, 0.0, 1.0, &mut m, &mut v);
    });
    assert!(r.is_ok());
}
