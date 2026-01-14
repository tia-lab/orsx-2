use crate::signal::spectral::{
    lomb_scargle_power, lomb_scargle_power_into_with_workspace, LombScargleConfig,
    LombScargleNormalization, LombScargleWorkspace,
};
use crate::MathError;

fn irregular_times(n: usize) -> Vec<f64> {
    let mut t = vec![0.0; n];
    let mut acc = 0.0;
    for i in 0..n {
        let jitter = 0.01 * (i as f64).sin();
        acc += 1.0 + jitter.abs();
        t[i] = acc;
    }
    t
}

fn sin_at_freq(t: &[f64], f_hz: f64, mean: f64) -> Vec<f64> {
    let mut x = vec![0.0; t.len()];
    let two_pi = std::f64::consts::TAU;
    for (i, &ti) in t.iter().enumerate() {
        x[i] = mean + (two_pi * f_hz * ti).sin();
    }
    x
}

#[test]
fn test_lomb_scargle_constant_series_is_zero() {
    let t = irregular_times(128);
    let x = vec![2.0; 128];
    let freqs = [0.01, 0.02, 0.05, 0.08];
    let cfg = LombScargleConfig {
        normalization: LombScargleNormalization::ByVariance,
        center: true,
    };
    let p = lomb_scargle_power(&t, &x, &freqs, &cfg).unwrap();
    for v in p {
        assert!(v.abs() <= 1e-12);
    }
}

#[test]
fn test_lomb_scargle_peak_at_true_frequency_irregular_sampling() {
    let t = irregular_times(512);
    let f0 = 0.07;
    let x = sin_at_freq(&t, f0, 0.3);
    let freqs = [0.05, 0.06, 0.07, 0.08, 0.09];
    let cfg = LombScargleConfig::default();
    let p = lomb_scargle_power(&t, &x, &freqs, &cfg).unwrap();

    let (imax, &pmax) = p
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap();
    assert_eq!(imax, 2);
    assert!(pmax.is_finite());
    assert!(pmax > p[1] + 0.2);
    assert!(pmax > p[3] + 0.2);
}

#[test]
fn test_lomb_scargle_time_shift_invariant() {
    let t = irregular_times(256);
    let mut t2 = t.clone();
    for v in t2.iter_mut() {
        *v += 123.456;
    }
    let x = sin_at_freq(&t, 0.04, 0.0);
    let freqs = [0.02, 0.03, 0.04, 0.05];
    let cfg = LombScargleConfig::default();

    let p1 = lomb_scargle_power(&t, &x, &freqs, &cfg).unwrap();
    let p2 = lomb_scargle_power(&t2, &x, &freqs, &cfg).unwrap();
    for (a, b) in p1.iter().zip(p2.iter()) {
        assert!((*a - *b).abs() <= 1e-12);
    }
}

#[test]
fn test_lomb_scargle_normalization_scaling_invariance() {
    let t = irregular_times(200);
    let freqs = [0.01, 0.02, 0.03];
    let x = sin_at_freq(&t, 0.02, 0.0);
    let mut x2 = x.clone();
    for v in x2.iter_mut() {
        *v *= 3.0;
    }
    let cfg = LombScargleConfig {
        normalization: LombScargleNormalization::ByVariance,
        center: true,
    };
    let p1 = lomb_scargle_power(&t, &x, &freqs, &cfg).unwrap();
    let p2 = lomb_scargle_power(&t, &x2, &freqs, &cfg).unwrap();
    for (a, b) in p1.iter().zip(p2.iter()) {
        assert!((*a - *b).abs() <= 1e-12);
    }
}

#[test]
fn test_lomb_scargle_workspace_matches_allocating() {
    let t = irregular_times(300);
    let x = sin_at_freq(&t, 0.03, 0.0);
    let freqs = [0.01, 0.02, 0.03, 0.04, 0.05];
    let cfg = LombScargleConfig::default();

    let p_alloc = lomb_scargle_power(&t, &x, &freqs, &cfg).unwrap();
    let mut p_ws = vec![0.0; freqs.len()];
    let mut ws = LombScargleWorkspace::with_capacity(t.len());
    lomb_scargle_power_into_with_workspace(&t, &x, &freqs, &cfg, &mut p_ws, &mut ws).unwrap();
    for (a, b) in p_alloc.iter().zip(p_ws.iter()) {
        assert!((*a - *b).abs() <= 1e-12);
    }
}

#[test]
fn test_lomb_scargle_rejects_invalid_inputs_and_params() {
    let t = vec![0.0, 1.0, 2.0, 3.0];
    let x = vec![1.0, 2.0, 3.0, 4.0];
    let cfg = LombScargleConfig::default();

    // mismatch lengths
    assert!(matches!(
        lomb_scargle_power(&t, &x[..3], &[0.1], &cfg),
        Err(MathError::InvalidData(_))
    ));

    // non-increasing times
    let t_bad = vec![0.0, 1.0, 1.0, 2.0];
    assert!(matches!(
        lomb_scargle_power(&t_bad, &x, &[0.1], &cfg),
        Err(MathError::InvalidData(_))
    ));

    // invalid frequency (<=0)
    assert!(matches!(
        lomb_scargle_power(&t, &x, &[0.0], &cfg),
        Err(MathError::InvalidParameter { .. })
    ));
}
