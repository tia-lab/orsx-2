use crate::signal::spectral::hilbert::{
    analytic_signal_amplitude_phase_into_with_workspace,
    instantaneous_frequency_hz_into_from_phase_with_workspace, unwrap_phase_into, HilbertWorkspace,
    InstantaneousFrequencyWorkspace,
};
use crate::{MathError, MathResult};

fn gen_cos(n: usize, k: usize) -> (Vec<f64>, Vec<f64>) {
    let mut theta = Vec::with_capacity(n);
    let mut cosx = Vec::with_capacity(n);
    for i in 0..n {
        let t = 2.0 * std::f64::consts::PI * (k as f64) * (i as f64) / (n as f64);
        theta.push(t);
        cosx.push(t.cos());
    }
    (theta, cosx)
}

#[test]
fn test_unwrap_phase_corrects_simple_wrap() -> MathResult<()> {
    let pi = std::f64::consts::PI;
    let phase = vec![0.9 * pi, -0.9 * pi];
    let mut out = vec![0.0f64; 2];
    unwrap_phase_into(&phase, &mut out)?;
    assert!((out[0] - 0.9 * pi).abs() <= 1e-15);
    // Should unwrap to 1.1*pi (add 2*pi).
    assert!((out[1] - 1.1 * pi).abs() <= 1e-12, "out[1]={}", out[1]);
    Ok(())
}

#[test]
fn test_instantaneous_frequency_matches_bin_aligned_cosine() -> MathResult<()> {
    // For a bin-aligned cosine x[i] = cos(2*pi*k*i/n), analytic phase is linear:
    // phi[i] = 2*pi*k*i/n (mod 2*pi). With dt=1, instantaneous frequency is k/n cycles/sample.
    let n = 4096usize;
    let k = 13usize;
    let (theta, x) = gen_cos(n, k);

    let mut amp = vec![0.0f64; n];
    let mut phase = vec![0.0f64; n];
    let mut hilbert_ws = HilbertWorkspace::with_capacity(n);
    analytic_signal_amplitude_phase_into_with_workspace(&x, &mut amp, &mut phase, &mut hilbert_ws)?;

    let mut freq = vec![0.0f64; n];
    let mut freq_ws = InstantaneousFrequencyWorkspace::with_capacity(n);
    instantaneous_frequency_hz_into_from_phase_with_workspace(
        &phase,
        1.0,
        &mut freq,
        &mut freq_ws,
    )?;

    let expected = (k as f64) / (n as f64);
    // Endpoints use 1st-order differences; focus on interior.
    let mut max_err = 0.0f64;
    for i in 1..(n - 1) {
        max_err = max_err.max((freq[i] - expected).abs());

        // Also sanity-check that unwrapped phase is consistent with theta modulo a constant.
        // We only check wrapped difference because theta grows beyond [-pi,pi].
        let mut d = phase[i] - theta[i];
        while d > std::f64::consts::PI {
            d -= 2.0 * std::f64::consts::PI;
        }
        while d < -std::f64::consts::PI {
            d += 2.0 * std::f64::consts::PI;
        }
        assert!(d.abs() <= 5e-10);
    }
    assert!(max_err <= 5e-10, "max_err={max_err:e}");
    Ok(())
}

#[test]
fn test_instantaneous_frequency_failure_contracts_and_no_panic() {
    let r = std::panic::catch_unwind(|| {
        let phase = vec![0.0, f64::NAN];
        let mut out = vec![0.0f64; 2];
        let mut ws = InstantaneousFrequencyWorkspace::with_capacity(2);
        let _ = instantaneous_frequency_hz_into_from_phase_with_workspace(
            &phase, 1.0, &mut out, &mut ws,
        );
    });
    assert!(r.is_ok());

    let phase = vec![0.0f64, 1.0];
    let mut ws = InstantaneousFrequencyWorkspace::with_capacity(2);
    let mut out_bad = vec![0.0f64; 1];
    let err = instantaneous_frequency_hz_into_from_phase_with_workspace(
        &phase,
        1.0,
        &mut out_bad,
        &mut ws,
    )
    .unwrap_err();
    assert!(matches!(err, MathError::InvalidParameter { .. }));

    let mut out = vec![0.0f64; 2];
    let err =
        instantaneous_frequency_hz_into_from_phase_with_workspace(&phase, 0.0, &mut out, &mut ws)
            .unwrap_err();
    assert!(matches!(err, MathError::InvalidParameter { .. }));
}
