use crate::core::autocorrelation::AutocorrelationNormalization;
use crate::signal::spectral::autocorrelation_fft::calculate_autocorrelation_fft;
use crate::signal::spectral::coherence::magnitude_squared_coherence;
use crate::signal::spectral::periodogram::calculate_periodogram;
use crate::signal::types::DetrendMethod;
use crate::{MathError, MathResult};

#[test]
fn test_periodogram_translation_invariant_under_mean_detrend() -> MathResult<()> {
    let n = 2048usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        x.push((i as f64 * 0.01).sin() + 0.1 * ((i % 23) as f64));
    }
    let mut y = x.clone();
    for v in y.iter_mut() {
        // Translation invariance is limited by f64 precision. At ~1e12, the ulp is ~2.4e-4, which
        // can dominate small-amplitude signals after mean removal. Use a large offset that remains
        // representable at sub-micro scales.
        *v += 1.0e8;
    }

    let px = calculate_periodogram(&x, DetrendMethod::RemoveMean)?;
    let py = calculate_periodogram(&y, DetrendMethod::RemoveMean)?;
    for (a, b) in px.iter().zip(py.iter()) {
        assert!((a - b).abs() <= 1e-6);
    }
    Ok(())
}

#[test]
fn test_autocorrelation_fft_handles_large_offset_and_is_finite() -> MathResult<()> {
    let n = 4096usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        x.push(1.0e9 + (i as f64 * 0.1).sin());
    }
    let acf = calculate_autocorrelation_fft(&x, 20, AutocorrelationNormalization::Biased)?;
    assert_eq!(acf[0], 1.0);
    for v in acf.iter() {
        assert!(v.is_finite());
        assert!(*v >= -1.0 && *v <= 1.0);
    }
    Ok(())
}

#[test]
fn test_coherence_handles_large_offset_after_mean_detrend() -> MathResult<()> {
    let n = 2048usize;
    let mut x = Vec::with_capacity(n);
    let mut y = Vec::with_capacity(n);
    for i in 0..n {
        let base = 1.0e8;
        x.push(base + (i as f64 * 0.02).sin());
        y.push(base + (i as f64 * 0.02).sin());
    }
    let c = magnitude_squared_coherence(&x, &y, DetrendMethod::RemoveMean)?;
    for v in c.iter() {
        assert!(v.is_finite());
        assert!((*v - 1.0).abs() <= 1e-8);
    }
    Ok(())
}

#[test]
fn test_periodogram_rejects_too_short() {
    let x = vec![1.0, 2.0, 3.0];
    let e = calculate_periodogram(&x, DetrendMethod::None).unwrap_err();
    assert!(
        matches!(e, MathError::InsufficientDataAlgo { .. }),
        "e={e:?}"
    );
}
