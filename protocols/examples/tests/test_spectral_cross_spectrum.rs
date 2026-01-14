use crate::signal::spectral::cross_spectrum::{
    calculate_cross_phase_and_group_delay_yx_into_with_workspace, calculate_cross_spectrum_yx_into,
    cross_phase_and_group_delay_from_spectrum_yx_into, CrossPhaseWorkspace, CrossSpectrumWorkspace,
};
use crate::signal::spectral::fft::Complex64;
use crate::signal::types::DetrendMethod;
use crate::MathError;
use std::f64::consts::PI;

fn gen_seeded(n: usize, seed: u64) -> Vec<f64> {
    let mut x = Vec::with_capacity(n);
    let mut s = seed;
    for _ in 0..n {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let u = ((s >> 11) as f64) * (1.0 / ((1u64 << 53) as f64));
        x.push(2.0 * u - 1.0);
    }
    x
}

fn circular_delay(x: &[f64], delay: usize) -> Vec<f64> {
    let n = x.len();
    let d = delay % n;
    let mut y = vec![0.0f64; n];
    for i in 0..n {
        y[i] = x[(i + n - d) % n];
    }
    y
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / (v.len() as f64)
}

#[test]
fn test_cross_phase_wrap_branch_matches_trig_on_domain() {
    // phase[k] comes from atan2 => [-π, π], so delta = phase[k+1]-phase[k-1] ∈ [-2π, 2π].
    // Our production wrap is branch-based; this test ensures it matches the canonical trig wrap.
    let n = 10_000usize;
    let mut s = 123u64;
    for _ in 0..n {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let u = ((s >> 11) as f64) * (1.0 / ((1u64 << 53) as f64));
        let delta = (4.0 * PI) * (u - 0.5); // [-2π, 2π]

        let trig = delta.sin().atan2(delta.cos());
        let mut branch = delta;
        if branch > PI {
            branch -= 2.0 * PI;
        } else if branch < -PI {
            branch += 2.0 * PI;
        }

        assert!(
            (trig - branch).abs() <= 1e-12,
            "delta={delta} trig={trig} branch={branch}"
        );
    }
}

#[test]
fn test_cross_spectrum_identity_phase_and_group_delay_are_zero() {
    let n = 256usize;
    let x = gen_seeded(n, 1);
    let y = x.clone();

    let mut spectrum = vec![Complex64::new(0.0, 0.0); n];
    let mut ws = CrossSpectrumWorkspace::with_capacity(n);
    calculate_cross_spectrum_yx_into(&x, &y, DetrendMethod::None, &mut spectrum, &mut ws).unwrap();

    let mut phase = vec![0.0f64; n];
    let mut tau = vec![0.0f64; n];
    cross_phase_and_group_delay_from_spectrum_yx_into(&spectrum, &mut phase, &mut tau).unwrap();

    for k in 0..n {
        let s = spectrum[k];
        let mag2 = s.re * s.re + s.im * s.im;
        if mag2 > 1e-20 {
            assert!(phase[k].abs() <= 1e-12, "k={k} phase={}", phase[k]);
        }
        assert!(tau[k].abs() <= 1e-9, "k={k} tau={}", tau[k]);
    }
}

#[test]
fn test_cross_phase_group_delay_delay_oracle() {
    let n = 512usize;
    let delay = 7usize;
    let x = gen_seeded(n, 2);
    let y = circular_delay(&x, delay);

    let mut phase = vec![0.0f64; n];
    let mut tau = vec![0.0f64; n];
    let mut ws = CrossPhaseWorkspace::with_capacity(n);
    calculate_cross_phase_and_group_delay_yx_into_with_workspace(
        &x,
        &y,
        DetrendMethod::None,
        &mut phase,
        &mut tau,
        &mut ws,
    )
    .unwrap();

    let mut spectrum = vec![Complex64::new(0.0, 0.0); n];
    let mut cs_ws = CrossSpectrumWorkspace::with_capacity(n);
    calculate_cross_spectrum_yx_into(&x, &y, DetrendMethod::None, &mut spectrum, &mut cs_ws)
        .unwrap();

    let mut tau_used = Vec::new();
    for k in 2..(n / 2 - 2) {
        let s = spectrum[k];
        let mag2 = s.re * s.re + s.im * s.im;
        if mag2 > 1e-14 {
            tau_used.push(tau[k]);
        }
    }
    assert!(tau_used.len() > 32, "not enough usable bins");
    let tau_mean = mean(&tau_used);
    assert!(
        (tau_mean - delay as f64).abs() <= 0.15,
        "tau_mean={tau_mean} expected={delay}"
    );
}

#[test]
fn test_cross_spectrum_determinism() {
    let n = 256usize;
    let x = gen_seeded(n, 3);
    let y = gen_seeded(n, 4);
    let mut out1 = vec![Complex64::new(0.0, 0.0); n];
    let mut out2 = vec![Complex64::new(0.0, 0.0); n];
    let mut ws = CrossSpectrumWorkspace::with_capacity(n);

    for _ in 0..10 {
        calculate_cross_spectrum_yx_into(&x, &y, DetrendMethod::RemoveMean, &mut out1, &mut ws)
            .unwrap();
        calculate_cross_spectrum_yx_into(&x, &y, DetrendMethod::RemoveMean, &mut out2, &mut ws)
            .unwrap();
        for k in 0..n {
            assert_eq!(out1[k], out2[k]);
        }
    }
}

#[test]
fn test_cross_spectrum_failure_contract() {
    let x = vec![1.0, 2.0, 3.0, 4.0];
    let y_short = vec![1.0, 2.0, 3.0];
    let mut out = vec![Complex64::new(0.0, 0.0); x.len()];
    let mut ws = CrossSpectrumWorkspace::with_capacity(x.len());

    let err =
        calculate_cross_spectrum_yx_into(&x, &y_short, DetrendMethod::None, &mut out, &mut ws)
            .unwrap_err();
    assert!(matches!(err, MathError::InvalidParameter { .. }));

    let y = vec![f64::NAN, 2.0, 3.0, 4.0];
    let err = calculate_cross_spectrum_yx_into(&x, &y, DetrendMethod::None, &mut out, &mut ws)
        .unwrap_err();
    assert!(matches!(err, MathError::InvalidData(_)));

    let mut out_short = vec![Complex64::new(0.0, 0.0); 3];
    let err = calculate_cross_spectrum_yx_into(
        &x,
        &vec![1.0, 2.0, 3.0, 4.0],
        DetrendMethod::None,
        &mut out_short,
        &mut ws,
    )
    .unwrap_err();
    assert!(matches!(err, MathError::InvalidParameter { .. }));

    let spectrum_short = vec![Complex64::new(0.0, 0.0); 3];
    let mut phase = vec![0.0f64; 3];
    let mut tau = vec![0.0f64; 3];
    let err =
        cross_phase_and_group_delay_from_spectrum_yx_into(&spectrum_short, &mut phase, &mut tau)
            .unwrap_err();
    assert!(matches!(err, MathError::InsufficientDataAlgo { .. }));
}

#[test]
fn test_cross_spectrum_panic_safety_on_error_paths() {
    let x = vec![1.0, 2.0, 3.0, 4.0];
    let y = vec![1.0, 2.0, 3.0];
    let mut out = vec![Complex64::new(0.0, 0.0); x.len()];
    let mut ws = CrossSpectrumWorkspace::with_capacity(x.len());

    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = calculate_cross_spectrum_yx_into(&x, &y, DetrendMethod::None, &mut out, &mut ws);
    }));
    assert!(r.is_ok());
}
