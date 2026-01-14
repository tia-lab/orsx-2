use crate::signal::dfa::{generate_window_sizes, integrate_series, segment_fluctuation_rms_linear};
use crate::MathResult;

#[test]
fn test_integrate_series_matches_centered_cumsum() -> MathResult<()> {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0];
    let y = integrate_series(&x)?;
    let exp = [-2.0, -3.0, -3.0, -2.0, 0.0];
    for (a, b) in y.iter().zip(exp.iter()) {
        assert!((a - b).abs() < 1e-12, "a={a} b={b}");
    }
    Ok(())
}

#[test]
fn test_segment_fluctuation_linear_is_near_zero() -> MathResult<()> {
    let seg: Vec<f64> = (0..50).map(|i| 3.0 + 2.0 * (i as f64)).collect();
    let rms = segment_fluctuation_rms_linear(&seg)?;
    assert!(rms.abs() < 1e-10, "rms={rms}");
    Ok(())
}

#[test]
fn test_segment_fluctuation_rejects_short_or_non_finite() {
    assert!(segment_fluctuation_rms_linear(&[1.0, 2.0]).is_err());
    assert!(segment_fluctuation_rms_linear(&[1.0, f64::NAN, 3.0]).is_err());
}

#[test]
fn test_generate_window_sizes_geometric_properties() -> MathResult<()> {
    let sizes = generate_window_sizes(1000, 10, 4.0)?;
    assert!(!sizes.is_empty());
    assert_eq!(sizes[0], 10);
    let max_size = (1000.0 / 4.0) as usize;
    assert!(*sizes.last().unwrap() <= max_size);
    for w in sizes.windows(2) {
        assert!(w[1] > w[0]);
    }
    Ok(())
}
