use crate::signal::spectral::fft::Complex64;
use crate::signal::spectral::hilbert::{
    analytic_signal_amplitude_phase_into_with_workspace,
    calculate_analytic_signal_into_with_workspace, calculate_hilbert_transform_into_with_workspace,
    HilbertWorkspace,
};
use crate::{MathError, MathResult};

fn gen_cos_sin(n: usize, k: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut cosx = Vec::with_capacity(n);
    let mut sinx = Vec::with_capacity(n);
    let mut theta = Vec::with_capacity(n);
    for i in 0..n {
        let t = 2.0 * std::f64::consts::PI * (k as f64) * (i as f64) / (n as f64);
        theta.push(t);
        cosx.push(t.cos());
        sinx.push(t.sin());
    }
    (theta, cosx, sinx)
}

#[test]
fn test_hilbert_failure_contracts_and_no_panic() {
    let r = std::panic::catch_unwind(|| {
        let x = vec![1.0, f64::NAN];
        let mut out = vec![0.0f64; 2];
        let mut ws = HilbertWorkspace::with_capacity(2);
        let _ = calculate_hilbert_transform_into_with_workspace(&x, &mut out, &mut ws);
    });
    assert!(r.is_ok());

    let x_short = vec![1.0];
    let mut out_short = vec![0.0f64; 1];
    let mut ws = HilbertWorkspace::with_capacity(1);
    let err = calculate_hilbert_transform_into_with_workspace(&x_short, &mut out_short, &mut ws)
        .unwrap_err();
    assert!(matches!(err, MathError::InsufficientDataAlgo { .. }));
}

#[test]
fn test_analytic_signal_real_part_matches_input_constant() -> MathResult<()> {
    let x = vec![3.5f64; 257];
    let mut out = vec![Complex64::new(0.0, 0.0); x.len()];
    let mut ws = HilbertWorkspace::with_capacity(x.len());
    calculate_analytic_signal_into_with_workspace(&x, &mut out, &mut ws)?;
    for z in out.iter() {
        assert!((z.re - 3.5).abs() <= 1e-12);
        assert!(z.im.abs() <= 1e-12);
    }
    Ok(())
}

#[test]
fn test_hilbert_cos_is_sin_on_bin_even_length() -> MathResult<()> {
    let n = 1024usize;
    let k = 7usize;
    let (_theta, cosx, sinx) = gen_cos_sin(n, k);

    let mut h = vec![0.0f64; n];
    let mut ws = HilbertWorkspace::with_capacity(n);
    calculate_hilbert_transform_into_with_workspace(&cosx, &mut h, &mut ws)?;

    let mut max_err = 0.0f64;
    for (hi, si) in h.iter().zip(sinx.iter()) {
        max_err = max_err.max((*hi - *si).abs());
    }
    assert!(max_err <= 5e-11, "max_err={max_err:e}");
    Ok(())
}

#[test]
fn test_hilbert_sin_is_minus_cos_on_bin_odd_length() -> MathResult<()> {
    let n = 999usize;
    let k = 11usize;
    let (_theta, cosx, sinx) = gen_cos_sin(n, k);

    let mut h = vec![0.0f64; n];
    let mut ws = HilbertWorkspace::with_capacity(n);
    calculate_hilbert_transform_into_with_workspace(&sinx, &mut h, &mut ws)?;

    let mut max_err = 0.0f64;
    for (hi, ci) in h.iter().zip(cosx.iter()) {
        max_err = max_err.max((*hi + *ci).abs());
    }
    assert!(max_err <= 5e-11, "max_err={max_err:e}");
    Ok(())
}

#[test]
fn test_analytic_signal_amplitude_phase_for_pure_tone() -> MathResult<()> {
    let n = 2048usize;
    let k = 5usize;
    let (theta, cosx, _sinx) = gen_cos_sin(n, k);

    let mut amp = vec![0.0f64; n];
    let mut ph = vec![0.0f64; n];
    let mut ws = HilbertWorkspace::with_capacity(n);
    analytic_signal_amplitude_phase_into_with_workspace(&cosx, &mut amp, &mut ph, &mut ws)?;

    // For a bin-aligned cosine, analytic signal is exp(i*theta), so amp==1 and phase==theta (mod 2π).
    let mut max_amp_err = 0.0f64;
    let mut max_phase_wrap_err = 0.0f64;
    for i in 0..n {
        max_amp_err = max_amp_err.max((amp[i] - 1.0).abs());

        // Wrap phase difference to [-pi, pi] deterministically.
        let mut d = ph[i] - theta[i];
        while d > std::f64::consts::PI {
            d -= 2.0 * std::f64::consts::PI;
        }
        while d < -std::f64::consts::PI {
            d += 2.0 * std::f64::consts::PI;
        }
        max_phase_wrap_err = max_phase_wrap_err.max(d.abs());
    }
    assert!(max_amp_err <= 5e-11, "max_amp_err={max_amp_err:e}");
    assert!(
        max_phase_wrap_err <= 5e-11,
        "max_phase_wrap_err={max_phase_wrap_err:e}"
    );
    Ok(())
}

#[test]
fn test_hilbert_determinism() -> MathResult<()> {
    let n = 1500usize;
    let (_theta, cosx, _sinx) = gen_cos_sin(n, 17);
    let mut h1 = vec![0.0f64; n];
    let mut h2 = vec![0.0f64; n];
    let mut ws = HilbertWorkspace::with_capacity(n);
    calculate_hilbert_transform_into_with_workspace(&cosx, &mut h1, &mut ws)?;
    calculate_hilbert_transform_into_with_workspace(&cosx, &mut h2, &mut ws)?;
    assert_eq!(h1, h2);
    Ok(())
}

#[test]
fn test_hilbert_numerical_stability_large_offset() -> MathResult<()> {
    let n = 4096usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        x.push(1e12 + (i as f64) * 1e-3);
    }
    let mut h = vec![0.0f64; n];
    let mut ws = HilbertWorkspace::with_capacity(n);
    calculate_hilbert_transform_into_with_workspace(&x, &mut h, &mut ws)?;
    assert!(h.iter().all(|v| v.is_finite()));
    Ok(())
}
