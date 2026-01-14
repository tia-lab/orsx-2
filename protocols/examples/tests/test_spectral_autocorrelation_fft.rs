use crate::core::autocorrelation::{calculate_autocorrelation, AutocorrelationNormalization};
use crate::signal::spectral::autocorrelation_fft::calculate_autocorrelation_fft;
use crate::MathResult;

#[test]
fn test_autocorrelation_fft_matches_direct_small_case() -> MathResult<()> {
    let n = 128usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        x.push((i as f64 * 0.1).sin() + 0.01 * (i as f64));
    }
    let max_lag = 20usize;
    let a = calculate_autocorrelation(&x, max_lag, AutocorrelationNormalization::Biased)?;
    let b = calculate_autocorrelation_fft(&x, max_lag, AutocorrelationNormalization::Biased)?;
    for (lag, (u, v)) in a.iter().zip(b.iter()).enumerate() {
        assert!((u - v).abs() <= 1e-10, "lag={lag} u={u} v={v}");
    }
    Ok(())
}

#[test]
fn test_autocorrelation_fft_rejects_invalid() {
    let x = vec![1.0, 2.0, 3.0, 4.0];
    assert!(calculate_autocorrelation_fft(&x, 4, AutocorrelationNormalization::Biased).is_err());
    let x2 = vec![1.0, 2.0, f64::NAN, 4.0, 5.0];
    assert!(calculate_autocorrelation_fft(&x2, 2, AutocorrelationNormalization::Biased).is_err());
}

#[test]
fn test_autocorrelation_fft_does_not_panic_on_error_paths() {
    let r = std::panic::catch_unwind(|| {
        let _ = calculate_autocorrelation_fft(&[], 0, AutocorrelationNormalization::Biased);
        let _ = calculate_autocorrelation_fft(
            &[1.0, 2.0, 3.0],
            1,
            AutocorrelationNormalization::Biased,
        );
    });
    assert!(r.is_ok());
}
