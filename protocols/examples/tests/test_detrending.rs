use crate::signal::detrending::{
    detrend, detrend_polynomial_into_with_workspace,
    detrend_polynomial_precomputed_into_with_workspace, PolynomialDetrendPrecomputedWorkspace,
    PolynomialDetrendWorkspace,
};
use crate::signal::types::DetrendMethod;
use crate::{MathError, MathResult};

#[test]
fn test_detrend_none_is_identity() -> MathResult<()> {
    let x = [1.0, 2.0, 3.0];
    let y = detrend(&x, DetrendMethod::None)?;
    assert_eq!(y, x);
    Ok(())
}

#[test]
fn test_detrend_remove_mean_has_zero_mean() -> MathResult<()> {
    let x = [1.0, 2.0, 3.0, 4.0];
    let y = detrend(&x, DetrendMethod::RemoveMean)?;
    let mean = y.iter().sum::<f64>() / (y.len() as f64);
    assert!(mean.abs() <= 1e-12);
    Ok(())
}

#[test]
fn test_detrend_remove_linear_removes_perfect_line() -> MathResult<()> {
    let x: Vec<f64> = (0..200).map(|i| 3.0 + 2.0 * (i as f64)).collect();
    let y = detrend(&x, DetrendMethod::RemoveLinear)?;
    let rms = (y.iter().map(|v| v * v).sum::<f64>() / (y.len() as f64)).sqrt();
    assert!(rms <= 1e-10, "rms={rms}");
    Ok(())
}

#[test]
fn test_detrend_polynomial_degree2_removes_quadratic() -> MathResult<()> {
    let n = 256usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f64 / (n as f64);
        x.push(1.0 + 2.0 * t + 3.0 * t * t);
    }
    let y = detrend(&x, DetrendMethod::RemovePolynomial { degree: 2 })?;
    let rms = (y.iter().map(|v| v * v).sum::<f64>() / (y.len() as f64)).sqrt();
    assert!(rms <= 1e-8, "rms={rms}");
    Ok(())
}

#[test]
fn test_detrend_polynomial_workspace_matches_allocating() -> MathResult<()> {
    let n = 200usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f64 / ((n - 1) as f64);
        x.push(1.0 + 2.0 * t + 0.1 * (t * 17.0).sin());
    }
    let a = detrend(&x, DetrendMethod::RemovePolynomial { degree: 2 })?;

    let mut ws = PolynomialDetrendWorkspace::with_capacity(n, 2)?;
    let mut out = vec![0.0f64; n];
    detrend_polynomial_into_with_workspace(&x, 2, &mut out, &mut ws)?;

    for (u, v) in a.iter().zip(out.iter()) {
        assert!((u - v).abs() <= 1e-12);
    }
    Ok(())
}

#[test]
fn test_detrend_polynomial_precomputed_workspace_matches_allocating() -> MathResult<()> {
    let n = 512usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f64 / ((n - 1) as f64);
        x.push(1.0 + 2.0 * t + 0.1 * (t * 17.0).sin());
    }
    let a = detrend(&x, DetrendMethod::RemovePolynomial { degree: 2 })?;

    let mut ws = PolynomialDetrendPrecomputedWorkspace::with_capacity(n, 2)?;
    let mut out = vec![0.0f64; n];
    detrend_polynomial_precomputed_into_with_workspace(&x, 2, &mut out, &mut ws)?;

    for (u, v) in a.iter().zip(out.iter()) {
        assert!((u - v).abs() <= 1e-12);
    }
    Ok(())
}

#[test]
fn test_detrend_polynomial_precomputed_rejects_workspace_mismatch() -> MathResult<()> {
    let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let mut out = vec![0.0f64; x.len()];
    let mut ws = PolynomialDetrendPrecomputedWorkspace::with_capacity(10, 2)?;
    assert!(detrend_polynomial_precomputed_into_with_workspace(&x, 2, &mut out, &mut ws).is_err());
    Ok(())
}

#[test]
fn test_detrend_rejects_non_finite_or_invalid_degree() {
    let x = [1.0, f64::NAN, 3.0];
    assert!(detrend(&x, DetrendMethod::RemoveMean).is_err());

    let x2 = [1.0, 2.0, 3.0];
    let e = detrend(&x2, DetrendMethod::RemovePolynomial { degree: 10 }).unwrap_err();
    assert!(matches!(e, MathError::InvalidParameter { .. }), "e={e:?}");
}

#[test]
fn test_detrend_does_not_panic_on_error_paths() {
    let r = std::panic::catch_unwind(|| {
        let _ = detrend(&[], DetrendMethod::RemoveMean);
        let _ = detrend(&[1.0], DetrendMethod::RemoveLinear);
    });
    assert!(r.is_ok());
}
