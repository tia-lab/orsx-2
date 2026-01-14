use crate::signal::spectral::multitaper::{
    calculate_multitaper_power_spectrum_into_with_workspace, MultitaperWorkspace,
};
use crate::signal::spectral::periodogram::calculate_periodogram;
use crate::signal::spectral::{compute_dpss_tapers_flat_into_with_workspace, DpssWorkspace};
use crate::signal::types::DetrendMethod;
use crate::{MathError, MathResult};

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

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / (v.len() as f64)
}

fn std(v: &[f64]) -> f64 {
    let m = mean(v);
    let mut acc = 0.0f64;
    for &x in v {
        let d = x - m;
        acc += d * d;
    }
    (acc / (v.len() as f64)).sqrt()
}

#[test]
fn test_dpss_orthonormality_and_sign_convention() -> MathResult<()> {
    let n = 256usize;
    let nw = 3.0;
    let k = 5usize;

    let mut tapers = vec![0.0f64; n * k];
    let mut eig = vec![0.0f64; k];
    let mut ws = DpssWorkspace::with_capacity(n, k);
    compute_dpss_tapers_flat_into_with_workspace(n, nw, k, &mut tapers, &mut eig, &mut ws)?;

    assert!(eig.iter().all(|v| v.is_finite() && *v > 0.0));

    // Norms ~ 1.
    for j in 0..k {
        let t = &tapers[j * n..(j + 1) * n];
        let norm2: f64 = t.iter().map(|x| x * x).sum();
        assert!((norm2 - 1.0).abs() <= 1e-8, "j={j} norm2={norm2}");
    }

    // Orthonormality.
    for i in 0..k {
        for j in 0..k {
            let ti = &tapers[i * n..(i + 1) * n];
            let tj = &tapers[j * n..(j + 1) * n];
            let dot: f64 = ti.iter().zip(tj.iter()).map(|(a, b)| a * b).sum();
            let expected = if i == j { 1.0 } else { 0.0 };
            assert!((dot - expected).abs() <= 1e-6, "i={i} j={j} dot={dot}");
        }
    }

    // Sign convention.
    for j in 0..k {
        let t = &tapers[j * n..(j + 1) * n];
        if j % 2 == 0 {
            let s: f64 = t.iter().sum();
            assert!(s >= 0.0);
        } else {
            assert!(t[0] >= 0.0);
        }
    }

    Ok(())
}

#[test]
fn test_multitaper_constant_signal_mean_detrend_is_near_zero() -> MathResult<()> {
    let n = 256usize;
    let x = vec![7.0f64; n];
    let nw = 3.0;
    let k = 5usize;

    let mut out = vec![0.0f64; n];
    let mut ws = MultitaperWorkspace::with_capacity(n, k);
    calculate_multitaper_power_spectrum_into_with_workspace(
        &x,
        DetrendMethod::RemoveMean,
        nw,
        k,
        &mut out,
        &mut ws,
    )?;

    for (i, v) in out.iter().enumerate() {
        assert!(v.is_finite() && *v >= 0.0);
        assert!(*v <= 1e-10, "i={i} v={v}");
    }
    Ok(())
}

#[test]
fn test_multitaper_determinism_and_reuse() -> MathResult<()> {
    let n = 256usize;
    let x = gen_seeded(n, 123);
    let nw = 2.0;
    let k = 3usize;

    let mut a = vec![0.0f64; n];
    let mut b = vec![0.0f64; n];
    let mut ws = MultitaperWorkspace::with_capacity(n, k);

    calculate_multitaper_power_spectrum_into_with_workspace(
        &x,
        DetrendMethod::RemoveMean,
        nw,
        k,
        &mut a,
        &mut ws,
    )?;
    calculate_multitaper_power_spectrum_into_with_workspace(
        &x,
        DetrendMethod::RemoveMean,
        nw,
        k,
        &mut b,
        &mut ws,
    )?;

    assert_eq!(a, b);
    Ok(())
}

#[test]
fn test_multitaper_reduces_frequency_variation_on_noise_vs_periodogram() -> MathResult<()> {
    let n = 256usize;
    let x = gen_seeded(n, 77);
    let nw = 2.0;
    let k = 3usize;

    let p = calculate_periodogram(&x, DetrendMethod::RemoveMean)?;
    let mut mt = vec![0.0f64; n];
    let mut ws = MultitaperWorkspace::with_capacity(n, k);
    calculate_multitaper_power_spectrum_into_with_workspace(
        &x,
        DetrendMethod::RemoveMean,
        nw,
        k,
        &mut mt,
        &mut ws,
    )?;

    // Compare coefficient of variation across positive bins excluding DC, on a single realization.
    let bins = &p[1..(n / 2)];
    let bins_mt = &mt[1..(n / 2)];
    let cv_p = std(bins) / mean(bins).max(1e-12);
    let cv_mt = std(bins_mt) / mean(bins_mt).max(1e-12);
    assert!(cv_mt <= cv_p, "cv_mt={cv_mt} cv_p={cv_p}");
    Ok(())
}

#[test]
fn test_multitaper_failure_contracts() {
    let x = vec![1.0, 2.0, 3.0, 4.0];
    let mut out_wrong = vec![0.0f64; 3];
    let mut ws = MultitaperWorkspace::with_capacity(4, 1);
    let err = calculate_multitaper_power_spectrum_into_with_workspace(
        &x,
        DetrendMethod::None,
        3.0,
        1,
        &mut out_wrong,
        &mut ws,
    )
    .unwrap_err();
    assert!(matches!(err, MathError::InvalidParameter { .. }));

    let x_bad = vec![1.0, f64::NAN, 3.0, 4.0];
    let mut out = vec![0.0f64; 4];
    let err = calculate_multitaper_power_spectrum_into_with_workspace(
        &x_bad,
        DetrendMethod::None,
        3.0,
        1,
        &mut out,
        &mut ws,
    )
    .unwrap_err();
    assert!(matches!(err, MathError::InvalidData(_)));

    // DPSS parameter failures are surfaced through multitaper.
    let err = calculate_multitaper_power_spectrum_into_with_workspace(
        &x,
        DetrendMethod::None,
        0.0,
        1,
        &mut out,
        &mut ws,
    )
    .unwrap_err();
    assert!(matches!(err, MathError::InvalidParameter { .. }));
}

#[test]
fn test_precomputed_dpss_asset_10000_is_valid_if_available() -> MathResult<()> {
    let n = 10_000usize;
    let nw = 3.0;
    let k = 5usize;

    let mut tapers = vec![0.0f64; n * k];
    let mut eig = vec![0.0f64; k];
    let mut ws = DpssWorkspace::with_capacity(n, k);
    compute_dpss_tapers_flat_into_with_workspace(n, nw, k, &mut tapers, &mut eig, &mut ws)?;

    // Basic finite checks.
    assert!(tapers.iter().all(|v| v.is_finite()));
    assert!(eig.iter().all(|v| v.is_finite() && *v > 0.0));

    // Orthonormality for large n: check a subset of dot products with loose tolerance.
    for i in 0..k {
        let ti = &tapers[i * n..(i + 1) * n];
        let norm2: f64 = ti.iter().map(|x| x * x).sum();
        assert!((norm2 - 1.0).abs() <= 1e-5, "i={i} norm2={norm2}");
        for j in (i + 1)..k {
            let tj = &tapers[j * n..(j + 1) * n];
            let dot: f64 = ti.iter().zip(tj.iter()).map(|(a, b)| a * b).sum();
            assert!(dot.abs() <= 1e-4, "i={i} j={j} dot={dot}");
        }
    }
    Ok(())
}
