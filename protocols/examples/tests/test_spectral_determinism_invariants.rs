use crate::core::autocorrelation::{calculate_autocorrelation, AutocorrelationNormalization};
use crate::signal::detrending::detrend_into;
use crate::signal::spectral::autocorrelation_fft::{
    calculate_autocorrelation_fft, calculate_autocorrelation_fft_into, AutocorrelationFftWorkspace,
};
use crate::signal::spectral::coherence::{
    magnitude_squared_coherence, magnitude_squared_coherence_into, CoherenceWorkspace,
};
use crate::signal::spectral::periodogram::{
    calculate_periodogram, calculate_periodogram_into, PeriodogramWorkspace,
};
use crate::signal::spectral::time_varying::{
    stft_periodograms_flat_into_with_workspace, StftPeriodogramWorkspace,
};
use crate::signal::types::DetrendMethod;
use crate::MathResult;

fn gen_seeded(n: usize, seed: u64) -> Vec<f64> {
    let mut x = Vec::with_capacity(n);
    let mut s = seed;
    for _ in 0..n {
        // simple deterministic LCG in tests (no external RNG dependency)
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let u = ((s >> 11) as f64) * (1.0 / ((1u64 << 53) as f64)); // [0,1)
        x.push(2.0 * u - 1.0);
    }
    x
}

#[test]
fn test_periodogram_workspace_matches_allocating_and_deterministic() -> MathResult<()> {
    let x = gen_seeded(1024, 1);
    let exp = calculate_periodogram(&x, DetrendMethod::RemoveMean)?;

    let mut ws = PeriodogramWorkspace::with_capacity(x.len());
    let mut out = vec![0.0f64; x.len()];
    for _ in 0..20 {
        calculate_periodogram_into(&x, DetrendMethod::RemoveMean, &mut out, &mut ws)?;
        for (a, b) in out.iter().zip(exp.iter()) {
            assert!((a - b).abs() <= 1e-10);
        }
    }

    Ok(())
}

#[test]
fn test_periodogram_parseval_identity_after_detrend() -> MathResult<()> {
    let x = gen_seeded(1024, 2);
    let mut detrended = vec![0.0f64; x.len()];
    detrend_into(&x, DetrendMethod::RemoveMean, &mut detrended)?;
    let time_energy: f64 = detrended.iter().map(|v| v * v).sum();

    let mut ws = PeriodogramWorkspace::with_capacity(x.len());
    let mut p = vec![0.0f64; x.len()];
    calculate_periodogram_into(&x, DetrendMethod::RemoveMean, &mut p, &mut ws)?;
    let freq_energy: f64 = p.iter().sum();

    let rel = (time_energy - freq_energy).abs() / time_energy.max(1e-12);
    assert!(
        rel <= 1e-8,
        "time={time_energy} freq={freq_energy} rel={rel}"
    );
    Ok(())
}

#[test]
fn test_autocorrelation_fft_matches_direct_and_is_deterministic() -> MathResult<()> {
    let x = gen_seeded(2048, 3);
    let max_lag = 20usize;

    let direct = calculate_autocorrelation(&x, max_lag, AutocorrelationNormalization::Biased)?;
    let alloc = calculate_autocorrelation_fft(&x, max_lag, AutocorrelationNormalization::Biased)?;

    for (i, (a, b)) in direct.iter().zip(alloc.iter()).enumerate() {
        assert!((a - b).abs() <= 1e-10, "lag={i} a={a} b={b}");
    }

    let mut ws = AutocorrelationFftWorkspace::with_capacity(x.len());
    let mut out = vec![0.0f64; max_lag + 1];
    for _ in 0..20 {
        calculate_autocorrelation_fft_into(
            &x,
            max_lag,
            AutocorrelationNormalization::Biased,
            &mut out,
            &mut ws,
        )?;
        assert!((out[0] - 1.0).abs() <= 1e-12);
        for v in out.iter() {
            assert!(v.is_finite());
            assert!(*v >= -1.0 && *v <= 1.0);
        }
    }

    Ok(())
}

#[test]
fn test_coherence_invariants_and_workspace_equivalence() -> MathResult<()> {
    let x = gen_seeded(1024, 4);
    let y = gen_seeded(1024, 5);
    let alloc = magnitude_squared_coherence(&x, &y, DetrendMethod::RemoveMean)?;

    let mut ws = CoherenceWorkspace::with_capacity(x.len());
    let mut out = vec![0.0f64; x.len()];
    magnitude_squared_coherence_into(&x, &y, DetrendMethod::RemoveMean, &mut out, &mut ws)?;

    for (a, b) in out.iter().zip(alloc.iter()) {
        assert!((a - b).abs() <= 1e-10);
        assert!(*a >= 0.0 && *a <= 1.0);
    }

    let alloc_sym = magnitude_squared_coherence(&y, &x, DetrendMethod::RemoveMean)?;
    for (a, b) in alloc.iter().zip(alloc_sym.iter()) {
        assert!((a - b).abs() <= 1e-10);
    }

    Ok(())
}

#[test]
fn test_stft_flat_workspace_determinism_and_bounds() -> MathResult<()> {
    let x = gen_seeded(2048, 6);
    let window_len = 128usize;
    let step = 64usize;
    let max_windows = 10usize;

    let mut ws = StftPeriodogramWorkspace::with_capacity(window_len);
    let mut out = vec![0.0f64; max_windows * window_len];

    let w1 = stft_periodograms_flat_into_with_workspace(
        &x,
        window_len,
        step,
        DetrendMethod::RemoveMean,
        max_windows,
        &mut out,
        &mut ws,
    )?;
    let snapshot = out.clone();
    let w2 = stft_periodograms_flat_into_with_workspace(
        &x,
        window_len,
        step,
        DetrendMethod::RemoveMean,
        max_windows,
        &mut out,
        &mut ws,
    )?;
    assert_eq!(w1, w2);
    assert_eq!(snapshot, out);

    for v in out.iter().take(w1 * window_len) {
        assert!(v.is_finite() && *v >= 0.0);
    }

    Ok(())
}
