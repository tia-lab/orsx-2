use crate::signal::types::WaveletFamily;
use crate::signal::wavelets::{modwt_d4_detail_level, wavelet_variance};
use crate::MathResult;

#[test]
fn test_modwt_d4_level1_constant_signal_is_zero() -> MathResult<()> {
    let x = vec![2.0f64; 256];
    let w = modwt_d4_detail_level(&x, 1)?;
    let mean_sq = w.iter().map(|v| v * v).sum::<f64>() / (w.len() as f64);
    assert!(mean_sq <= 1e-12, "mean_sq={mean_sq}");
    Ok(())
}

#[test]
fn test_wavelet_variance_rejects_invalid_scale() {
    let x = vec![1.0f64; 128];
    assert!(wavelet_variance(&x, WaveletFamily::ModwtD4, 1).is_err());
    assert!(wavelet_variance(&x, WaveletFamily::ModwtD4, 3).is_err());
    assert!(wavelet_variance(&x, WaveletFamily::ModwtD4, 256).is_err());
    assert!(wavelet_variance(&x, WaveletFamily::ModwtD6, 1).is_err());
    assert!(wavelet_variance(&x, WaveletFamily::ModwtD8, 3).is_err());
}

#[test]
fn test_wavelet_variance_modwt_is_positive_for_non_constant() -> MathResult<()> {
    let n = 512usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        x.push((i as f64 * 0.01).sin() + 0.1 * ((i % 17) as f64));
    }
    let v = wavelet_variance(&x, WaveletFamily::ModwtD4, 4)?;
    assert!(v.is_finite() && v >= 0.0, "v={v}");
    Ok(())
}

#[test]
fn test_wavelet_variance_haar_is_zero_for_constant() -> MathResult<()> {
    let x = vec![5.0f64; 256];
    let v = wavelet_variance(&x, WaveletFamily::Haar, 8)?;
    assert!(v <= 1e-12, "v={v}");
    Ok(())
}

#[test]
fn test_wavelets_reject_non_finite_and_no_panic() {
    let r = std::panic::catch_unwind(|| {
        let x = vec![1.0, f64::NAN, 2.0];
        let _ = modwt_d4_detail_level(&x, 1);
        let x2 = vec![1.0f64; 32];
        let _ = wavelet_variance(&x2, WaveletFamily::ModwtD4, 1);
    });
    assert!(r.is_ok());
}
