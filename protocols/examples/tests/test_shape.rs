use crate::signal::shape::{
    hjorth_parameters, spectral_crest_from_periodogram, spectral_entropy_from_periodogram,
    spectral_flatness_from_periodogram, teager_kaiser_energy_mean,
};
use crate::MathError;

#[test]
fn test_hjorth_constant_is_zero() {
    let x = vec![2.0; 64];
    let h = hjorth_parameters(&x).unwrap();
    assert!(h.activity.abs() <= 1e-12);
    assert!(h.mobility.abs() <= 1e-12);
    assert!(h.complexity.abs() <= 1e-12);
}

#[test]
fn test_hjorth_linear_has_positive_activity_and_mobility() {
    let n = 100;
    let mut x = vec![0.0; n];
    for i in 0..n {
        x[i] = i as f64;
    }
    let h = hjorth_parameters(&x).unwrap();
    assert!(h.activity > 0.0);
    assert!(h.mobility > 0.0);
    assert!(h.complexity.is_finite());
}

#[test]
fn test_tkeo_constant_is_constant() {
    let x = vec![3.0; 50];
    // For constant c, ψ = c^2 - c*c = 0.
    let e = teager_kaiser_energy_mean(&x).unwrap();
    assert!(e.abs() <= 1e-12);
}

#[test]
fn test_spectral_flatness_and_crest_basic() {
    // Flat spectrum => flatness ~1, crest ~1.
    let p = vec![1.0; 32];
    let flat = spectral_flatness_from_periodogram(&p, 1e-12).unwrap();
    let crest = spectral_crest_from_periodogram(&p).unwrap();
    assert!((flat - 1.0).abs() <= 1e-12);
    assert!((crest - 1.0).abs() <= 1e-12);
}

#[test]
fn test_spectral_flatness_peaky_is_small_and_crest_large() {
    let mut p = vec![1e-6; 64];
    p[10] = 1.0;
    let flat = spectral_flatness_from_periodogram(&p, 1e-12).unwrap();
    let crest = spectral_crest_from_periodogram(&p).unwrap();
    assert!(flat < 0.2);
    assert!(crest > 10.0);
}

#[test]
fn test_spectral_entropy_bounds() {
    let p_flat = vec![1.0; 64];
    let h_flat = spectral_entropy_from_periodogram(&p_flat, 1e-18).unwrap();
    assert!(h_flat >= 0.99 && h_flat <= 1.0);

    let mut p_peaky = vec![0.0; 64];
    p_peaky[0] = 1.0;
    let h_peaky = spectral_entropy_from_periodogram(&p_peaky, 1e-18).unwrap();
    assert!(h_peaky <= 0.2);
}

#[test]
fn test_shape_rejects_nonfinite() {
    let x = vec![1.0, f64::NAN, 2.0];
    assert!(matches!(
        hjorth_parameters(&x),
        Err(MathError::InvalidData(_))
    ));
}
