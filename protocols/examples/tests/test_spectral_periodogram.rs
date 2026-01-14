use crate::signal::spectral::periodogram::calculate_periodogram;
use crate::signal::types::DetrendMethod;
use crate::MathResult;

#[test]
fn test_periodogram_constant_is_near_zero_except_dc_without_detrend() -> MathResult<()> {
    let x = vec![3.0f64; 64];
    let p = calculate_periodogram(&x, DetrendMethod::None)?;
    // DC should dominate; others should be ~0 (numerical noise).
    for (k, &v) in p.iter().enumerate().skip(1) {
        assert!(v <= 1e-10, "k={k} v={v}");
    }
    Ok(())
}

#[test]
fn test_periodogram_constant_is_all_near_zero_with_mean_detrend() -> MathResult<()> {
    let x = vec![3.0f64; 64];
    let p = calculate_periodogram(&x, DetrendMethod::RemoveMean)?;
    for (k, &v) in p.iter().enumerate() {
        assert!(v <= 1e-10, "k={k} v={v}");
    }
    Ok(())
}

#[test]
fn test_periodogram_rejects_non_finite() {
    let x = vec![1.0, f64::NAN, 2.0, 3.0];
    assert!(calculate_periodogram(&x, DetrendMethod::None).is_err());
}
