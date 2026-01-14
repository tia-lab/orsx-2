use crate::signal::types::WaveletFamily;
use crate::signal::wavelet_coherence::{
    wavelet_coherence_modwt_level_mean_with_workspace,
    wavelet_coherence_modwt_level_series_into_with_workspace, WaveletCoherenceConfig,
    WaveletCoherenceWorkspace,
};
use crate::MathError;

fn sin_series(n: usize, cycles: f64, phase: f64, mean: f64) -> Vec<f64> {
    let mut x = vec![0.0; n];
    let two_pi = std::f64::consts::TAU;
    for i in 0..n {
        let t = i as f64 / (n.saturating_sub(1) as f64);
        x[i] = mean + (two_pi * cycles * t + phase).sin();
    }
    x
}

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
fn test_wavelet_coherence_identical_is_high() {
    let n = 1024;
    let x = sin_series(n, 7.0, 0.0, 0.0);
    let cfg = WaveletCoherenceConfig {
        family: WaveletFamily::ModwtD4,
        level: 2,
        smooth_window: 33,
    };
    let mut ws = WaveletCoherenceWorkspace::with_capacity(n).unwrap();
    let c = wavelet_coherence_modwt_level_mean_with_workspace(&x, &x, &cfg, &mut ws).unwrap();
    assert!(c >= 0.95, "c={c}");
}

#[test]
fn test_wavelet_coherence_symmetry_and_scaling_invariance() {
    let n = 2048;
    let x = sin_series(n, 9.0, 0.0, 0.0);
    let y = sin_series(n, 9.0, 0.5, 0.0);
    let cfg = WaveletCoherenceConfig {
        family: WaveletFamily::ModwtD6,
        level: 3,
        smooth_window: 41,
    };
    let mut ws = WaveletCoherenceWorkspace::with_capacity(n).unwrap();
    let c_xy = wavelet_coherence_modwt_level_mean_with_workspace(&x, &y, &cfg, &mut ws).unwrap();
    let c_yx = wavelet_coherence_modwt_level_mean_with_workspace(&y, &x, &cfg, &mut ws).unwrap();
    assert!((c_xy - c_yx).abs() <= 1e-12);

    let mut y2 = y.clone();
    for v in y2.iter_mut() {
        *v *= 3.0;
    }
    let c_scaled =
        wavelet_coherence_modwt_level_mean_with_workspace(&x, &y2, &cfg, &mut ws).unwrap();
    assert!((c_xy - c_scaled).abs() <= 1e-12);
}

#[test]
fn test_wavelet_coherence_series_in_range_and_finite() {
    let n = 1024;
    let x = gen_seeded(n, 1);
    let y = gen_seeded(n, 2);
    let cfg = WaveletCoherenceConfig {
        family: WaveletFamily::ModwtD8,
        level: 1,
        smooth_window: 21,
    };
    let mut ws = WaveletCoherenceWorkspace::with_capacity(n).unwrap();
    let mut out = vec![0.0; n];
    wavelet_coherence_modwt_level_series_into_with_workspace(&x, &y, &cfg, &mut out, &mut ws)
        .unwrap();
    for v in out {
        assert!(v.is_finite());
        assert!(v >= 0.0 && v <= 1.0 + 1e-12);
    }
}

#[test]
fn test_wavelet_coherence_rejects_invalid_inputs() {
    let x = vec![1.0, 2.0, 3.0, 4.0];
    let y = vec![1.0, 2.0, 3.0];
    let cfg = WaveletCoherenceConfig::default();
    let mut ws = WaveletCoherenceWorkspace::with_capacity(4).unwrap();
    assert!(matches!(
        wavelet_coherence_modwt_level_mean_with_workspace(&x, &y, &cfg, &mut ws),
        Err(MathError::InvalidData(_))
    ));

    let y = vec![1.0, 2.0, f64::NAN, 4.0];
    let y2 = vec![1.0, 2.0, 3.0, 4.0];
    assert!(matches!(
        wavelet_coherence_modwt_level_mean_with_workspace(&y, &y2, &cfg, &mut ws),
        Err(MathError::InvalidData(_))
    ));

    let cfg_bad = WaveletCoherenceConfig {
        family: WaveletFamily::ModwtD4,
        level: 0,
        smooth_window: 3,
    };
    assert!(matches!(
        wavelet_coherence_modwt_level_mean_with_workspace(&y2, &y2, &cfg_bad, &mut ws),
        Err(MathError::InvalidParameter { .. })
    ));

    let cfg_bad = WaveletCoherenceConfig {
        family: WaveletFamily::ModwtD4,
        level: 1,
        smooth_window: 0,
    };
    assert!(matches!(
        wavelet_coherence_modwt_level_mean_with_workspace(&y2, &y2, &cfg_bad, &mut ws),
        Err(MathError::InvalidParameter { .. })
    ));
}
