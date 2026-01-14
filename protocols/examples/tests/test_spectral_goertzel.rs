use crate::signal::spectral::goertzel::{
    goertzel_power_bin, goertzel_powers_bins_into_with_workspace, GoertzelWorkspace,
};
use crate::signal::spectral::periodogram::calculate_periodogram;
use crate::signal::types::DetrendMethod;
use crate::MathResult;
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

#[test]
fn test_goertzel_matches_fft_periodogram_selected_bins() -> MathResult<()> {
    let n = 1024usize;
    let x = gen_seeded(n, 77);
    let p = calculate_periodogram(&x, DetrendMethod::None)?;

    for &k in &[0usize, 1, 7, 31, 128, 511, 513, 700, 900, 1023] {
        let g = goertzel_power_bin(&x, k)?;
        let f = p[k];
        let tol = 1e-8 * f.abs().max(1.0);
        assert!((g - f).abs() <= tol, "k={k} g={g} f={f} tol={tol}");
    }
    Ok(())
}

#[test]
fn test_goertzel_workspace_equivalence_and_determinism() -> MathResult<()> {
    let n = 2048usize;
    let x = gen_seeded(n, 78);
    let bins = [3usize, 17usize, 127usize, 1024usize];
    let mut out = vec![0.0f64; bins.len()];
    let mut ws = GoertzelWorkspace::with_capacity(bins.len());

    for _ in 0..20 {
        goertzel_powers_bins_into_with_workspace(&x, &bins, &mut out, &mut ws)?;
        for v in out.iter() {
            assert!(v.is_finite() && *v >= 0.0);
        }
    }

    let mut out2 = vec![0.0f64; bins.len()];
    goertzel_powers_bins_into_with_workspace(&x, &bins, &mut out2, &mut ws)?;
    assert_eq!(out, out2);
    Ok(())
}

#[test]
fn test_goertzel_closed_form_constant_signal() -> MathResult<()> {
    let n = 1024usize;
    let c = 3.0f64;
    let x = vec![c; n];

    // k=0: X[0] = n*c => |X|^2/n = n*c^2
    let p0 = goertzel_power_bin(&x, 0)?;
    let expected = (n as f64) * c * c;
    assert!(
        (p0 - expected).abs() <= 1e-8 * expected.max(1.0),
        "p0={p0} exp={expected}"
    );

    // k>0: should be ~0 (numerical noise only)
    for &k in &[1usize, 2, 7, 31, 127] {
        let pk = goertzel_power_bin(&x, k)?;
        assert!(pk <= 1e-8, "k={k} pk={pk}");
    }
    Ok(())
}

#[test]
fn test_goertzel_closed_form_single_tone_cosine_on_bin() -> MathResult<()> {
    let n = 2048usize;
    let k = 17usize;
    let mut x = Vec::with_capacity(n);
    for t in 0..n {
        x.push((2.0 * PI * (k as f64) * (t as f64) / (n as f64)).cos());
    }

    // For real cosine at exact bin: DFT magnitude at k and n-k is n/2 (others ~0).
    // Our scaling: |X[k]|^2 / n = (n/2)^2 / n = n/4
    let pk = goertzel_power_bin(&x, k)?;
    let expected = (n as f64) / 4.0;
    let rel = (pk - expected).abs() / expected.max(1.0);
    assert!(rel <= 1e-8, "pk={pk} exp={expected} rel={rel}");

    let pk2 = goertzel_power_bin(&x, n - k)?;
    let rel2 = (pk2 - expected).abs() / expected.max(1.0);
    assert!(rel2 <= 1e-8, "pk2={pk2} exp={expected} rel={rel2}");
    Ok(())
}

#[test]
fn test_goertzel_scaling_invariant_power_scales_quadratically() -> MathResult<()> {
    let n = 1024usize;
    let x = gen_seeded(n, 79);
    let k = 31usize;
    let p = goertzel_power_bin(&x, k)?;

    let a = 7.0f64;
    let x2: Vec<f64> = x.iter().map(|v| a * v).collect();
    let p2 = goertzel_power_bin(&x2, k)?;
    let expected = a * a * p;
    let rel = (p2 - expected).abs() / expected.max(1.0);
    assert!(rel <= 1e-10, "p2={p2} exp={expected} rel={rel}");
    Ok(())
}

#[test]
fn test_goertzel_multiple_bins_matches_individual_bins() -> MathResult<()> {
    let n = 2048usize;
    let x = gen_seeded(n, 80);
    let bins = [
        0usize,
        1usize,
        7usize,
        17usize,
        127usize,
        1024usize,
        1500usize,
        2047usize,
    ];
    let mut out = vec![0.0f64; bins.len()];
    let mut ws = GoertzelWorkspace::with_capacity(bins.len());
    goertzel_powers_bins_into_with_workspace(&x, &bins, &mut out, &mut ws)?;

    for (i, &k) in bins.iter().enumerate() {
        let pk = goertzel_power_bin(&x, k)?;
        assert!((out[i] - pk).abs() <= 1e-12, "k={k} out={} pk={pk}", out[i]);
    }
    Ok(())
}

#[test]
fn test_goertzel_numerical_stability_large_offset() -> MathResult<()> {
    let n = 4096usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        x.push(1.0e8 + (i as f64 * 0.1).sin());
    }
    let p = goertzel_power_bin(&x, 17)?;
    assert!(p.is_finite() && p >= 0.0);
    Ok(())
}

#[test]
fn test_goertzel_rejects_invalid_inputs_and_no_panic() {
    let r = std::panic::catch_unwind(|| {
        assert!(goertzel_power_bin(&[], 0).is_err());
        let x = vec![1.0, 2.0, 3.0, 4.0];
        assert!(goertzel_power_bin(&x, 4).is_err());
        let x2 = vec![1.0, f64::NAN, 2.0];
        assert!(goertzel_power_bin(&x2, 0).is_err());
    });
    assert!(r.is_ok());
}
