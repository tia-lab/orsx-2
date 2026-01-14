use crate::core::variance::variance_biased;
use crate::signal::wavelets::{
    modwt_d4_denoise_into_with_workspace, threshold_coefficients_in_place, ModwtD4DenoiseWorkspace,
    ThresholdKind,
};
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

#[test]
fn test_threshold_coefficients_hard_soft() -> MathResult<()> {
    let mut c = vec![-2.0, -0.5, 0.0, 0.5, 2.0];
    threshold_coefficients_in_place(&mut c, 1.0, ThresholdKind::Hard)?;
    assert_eq!(c, vec![-2.0, 0.0, 0.0, 0.0, 2.0]);

    let mut c2 = vec![-2.0, -0.5, 0.0, 0.5, 2.0];
    threshold_coefficients_in_place(&mut c2, 1.0, ThresholdKind::Soft)?;
    assert_eq!(c2, vec![-1.0, 0.0, 0.0, 0.0, 1.0]);
    Ok(())
}

#[test]
fn test_threshold_coefficients_failure_contracts() {
    let mut c = vec![1.0, 2.0, 3.0];
    assert!(threshold_coefficients_in_place(&mut c, -1.0, ThresholdKind::Hard).is_err());
    assert!(threshold_coefficients_in_place(&mut c, f64::NAN, ThresholdKind::Soft).is_err());
    let mut c_bad = vec![1.0, f64::NAN, 3.0];
    assert!(threshold_coefficients_in_place(&mut c_bad, 1.0, ThresholdKind::Soft).is_err());
}

#[test]
fn test_modwt_d4_denoise_threshold_zero_is_identity_hard_soft() -> MathResult<()> {
    let n = 1024usize;
    let levels = 5usize;
    let x = gen_seeded(n, 123);

    for &kind in &[ThresholdKind::Hard, ThresholdKind::Soft] {
        let mut out = vec![0.0f64; n];
        let mut ws = ModwtD4DenoiseWorkspace::with_capacity(n, levels)?;
        modwt_d4_denoise_into_with_workspace(&x, levels, 0.0, kind, &mut out, &mut ws)?;
        let mut max_err = 0.0f64;
        for i in 0..n {
            max_err = max_err.max((out[i] - x[i]).abs());
        }
        assert!(max_err <= 5e-11, "kind={kind:?} max_err={max_err:e}");
    }
    Ok(())
}

#[test]
fn test_modwt_d4_denoise_reduces_variance_for_noise() -> MathResult<()> {
    let n = 2048usize;
    let levels = 6usize;
    let x = gen_seeded(n, 7);
    let v_in = variance_biased(&x)?;

    let mut out = vec![0.0f64; n];
    let mut ws = ModwtD4DenoiseWorkspace::with_capacity(n, levels)?;
    modwt_d4_denoise_into_with_workspace(&x, levels, 0.25, ThresholdKind::Soft, &mut out, &mut ws)?;

    let v_out = variance_biased(&out)?;
    assert!(v_out.is_finite() && v_out >= 0.0);
    assert!(v_out <= v_in + 1e-12, "v_in={v_in} v_out={v_out}");
    Ok(())
}

#[test]
fn test_modwt_d4_denoise_determinism_with_workspace_reuse() -> MathResult<()> {
    let n = 1000usize;
    let levels = 4usize;
    let x = gen_seeded(n, 99);
    let mut out1 = vec![0.0f64; n];
    let mut out2 = vec![0.0f64; n];
    let mut ws = ModwtD4DenoiseWorkspace::with_capacity(n, levels)?;
    modwt_d4_denoise_into_with_workspace(&x, levels, 0.1, ThresholdKind::Hard, &mut out1, &mut ws)?;
    modwt_d4_denoise_into_with_workspace(&x, levels, 0.1, ThresholdKind::Hard, &mut out2, &mut ws)?;
    assert_eq!(out1, out2);
    Ok(())
}

#[test]
fn test_modwt_d4_denoise_failure_contracts_and_no_panic() {
    let r = std::panic::catch_unwind(|| {
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let mut out = vec![0.0f64; 4];
        let mut ws = ModwtD4DenoiseWorkspace::with_capacity(4, 1).unwrap();
        let _ = modwt_d4_denoise_into_with_workspace(
            &x,
            0,
            0.0,
            ThresholdKind::Hard,
            &mut out,
            &mut ws,
        );
    });
    assert!(r.is_ok());

    let x_short = vec![1.0, 2.0, 3.0];
    let mut out = vec![0.0f64; 3];
    let mut ws = ModwtD4DenoiseWorkspace::with_capacity(4, 1).unwrap();
    let err = modwt_d4_denoise_into_with_workspace(
        &x_short,
        1,
        0.0,
        ThresholdKind::Hard,
        &mut out,
        &mut ws,
    )
    .unwrap_err();
    assert!(matches!(err, MathError::InsufficientDataAlgo { .. }));

    let x_bad = vec![1.0, 2.0, f64::NAN, 4.0];
    let mut ws = ModwtD4DenoiseWorkspace::with_capacity(4, 1).unwrap();
    let err = modwt_d4_denoise_into_with_workspace(
        &x_bad,
        1,
        0.0,
        ThresholdKind::Hard,
        &mut out,
        &mut ws,
    )
    .unwrap_err();
    assert!(matches!(err, MathError::InvalidData(_)));

    let x = vec![1.0, 2.0, 3.0, 4.0];
    let mut out_short = vec![0.0f64; 3];
    let mut ws = ModwtD4DenoiseWorkspace::with_capacity(4, 1).unwrap();
    let err = modwt_d4_denoise_into_with_workspace(
        &x,
        1,
        0.0,
        ThresholdKind::Hard,
        &mut out_short,
        &mut ws,
    )
    .unwrap_err();
    assert!(matches!(err, MathError::InvalidParameter { .. }));
}

#[test]
fn test_modwt_d4_denoise_numerical_stability_large_offset() -> MathResult<()> {
    let n = 4096usize;
    let levels = 8usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        x.push(1e12 + 0.1 * (i as f64 * 0.01).sin());
    }
    let mut out = vec![0.0f64; n];
    let mut ws = ModwtD4DenoiseWorkspace::with_capacity(n, levels)?;
    modwt_d4_denoise_into_with_workspace(&x, levels, 0.05, ThresholdKind::Soft, &mut out, &mut ws)?;
    assert!(out.iter().all(|v| v.is_finite()));
    Ok(())
}
