use crate::signal::spectral::welch::{calculate_welch_power_spectrum_into, WelchWorkspace};
use crate::signal::spectral::windows::{apply_window_into, window_coefficients_into};
use crate::signal::types::{DetrendMethod, WindowFunction};
use crate::{MathError, MathResult};

#[test]
fn test_window_coefficients_are_finite_non_negative_and_symmetric() -> MathResult<()> {
    let n = 257usize;
    let mut w = vec![0.0f64; n];
    for &kind in &[
        WindowFunction::Rectangular,
        WindowFunction::Hann,
        WindowFunction::Hamming,
        WindowFunction::Blackman,
    ] {
        window_coefficients_into(n, kind, &mut w)?;
        for &v in w.iter() {
            assert!(v.is_finite());
            assert!(v >= 0.0);
        }
        for i in 0..n {
            let j = n - 1 - i;
            assert!((w[i] - w[j]).abs() <= 1e-12);
        }
    }
    Ok(())
}

#[test]
fn test_apply_window_into_matches_elementwise() -> MathResult<()> {
    let x = vec![1.0, 2.0, 3.0, 4.0];
    let w = vec![0.5, 0.25, 0.0, 2.0];
    let mut out = vec![0.0f64; x.len()];
    apply_window_into(&x, &w, &mut out)?;
    assert_eq!(out, vec![0.5, 0.5, 0.0, 8.0]);
    Ok(())
}

#[test]
fn test_welch_constant_signal_with_mean_detrend_is_near_zero() -> MathResult<()> {
    let data = vec![3.0f64; 2048];
    let segment_len = 256usize;
    let step = 128usize;
    let max_segments = 10usize;
    let mut out = vec![0.0f64; segment_len];
    let mut ws = WelchWorkspace::with_capacity(segment_len);
    let used = calculate_welch_power_spectrum_into(
        &data,
        segment_len,
        step,
        DetrendMethod::RemoveMean,
        WindowFunction::Hann,
        max_segments,
        &mut out,
        &mut ws,
    )?;
    assert!(used > 0);
    for (k, &v) in out.iter().enumerate() {
        assert!(v.is_finite());
        assert!(v <= 1e-10, "k={k} v={v}");
    }
    Ok(())
}

#[test]
fn test_welch_failure_contracts_and_no_panic() {
    let r = std::panic::catch_unwind(|| {
        let data = vec![1.0f64; 100];
        let mut out = vec![0.0f64; 10];
        let mut ws = WelchWorkspace::with_capacity(10);
        let e = calculate_welch_power_spectrum_into(
            &data,
            200,
            1,
            DetrendMethod::None,
            WindowFunction::Hann,
            10,
            &mut out,
            &mut ws,
        )
        .unwrap_err();
        assert!(matches!(e, MathError::InvalidParameter { .. }));

        let mut out2 = vec![0.0f64; 9];
        let e2 = calculate_welch_power_spectrum_into(
            &data,
            10,
            0,
            DetrendMethod::None,
            WindowFunction::Hann,
            10,
            &mut out2,
            &mut ws,
        )
        .unwrap_err();
        assert!(matches!(e2, MathError::InvalidParameter { .. }));
    });
    assert!(r.is_ok());
}
