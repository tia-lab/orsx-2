use crate::signal::mse::{
    coarse_grain_mean_into, multiscale_sample_entropy_into_with_workspace, MseWorkspace,
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
fn test_coarse_grain_mean_matches_manual() -> MathResult<()> {
    let x = vec![1.0f64, 2.0, 3.0, 4.0, 5.0, 6.0];
    let mut out = vec![0.0f64; 3];
    coarse_grain_mean_into(&x, 2, &mut out)?;
    assert_eq!(out, vec![1.5, 3.5, 5.5]);
    Ok(())
}

#[test]
fn test_mse_scale1_matches_sampen_on_original() -> MathResult<()> {
    let x = gen_seeded(500, 9);
    let mut out = vec![0.0f64; 4];
    let mut ws = MseWorkspace::with_capacity(x.len());
    multiscale_sample_entropy_into_with_workspace(&x, 2, 1, 0.5, 4, 10, &mut out, &mut ws)?;

    // scale=1 coarse-grain is identity, so MSE[0] equals SampEn on original series.
    let se = crate::signal::entropy::sample_entropy_chebyshev(
        &x,
        2,
        1,
        0.5,
        ws.sample_entropy_workspace_mut(),
    )?;
    assert!((out[0] - se).abs() <= 1e-12, "mse0={} se={}", out[0], se);
    Ok(())
}

#[test]
fn test_mse_failure_contracts_and_no_panic() {
    let r = std::panic::catch_unwind(|| {
        let x = vec![1.0f64, f64::NAN, 2.0, 3.0];
        let mut out = vec![0.0f64; 1];
        let mut ws = MseWorkspace::with_capacity(x.len());
        let _ =
            multiscale_sample_entropy_into_with_workspace(&x, 2, 1, 0.5, 1, 5, &mut out, &mut ws);
    });
    assert!(r.is_ok());

    let x = vec![1.0f64; 64];
    let mut out = vec![0.0f64; 2];
    let mut ws = MseWorkspace::with_capacity(x.len());
    let err = multiscale_sample_entropy_into_with_workspace(&x, 2, 1, 0.5, 2, 1, &mut out, &mut ws)
        .unwrap_err();
    assert!(matches!(err, MathError::InvalidParameter { .. }));

    let err =
        multiscale_sample_entropy_into_with_workspace(&x, 2, 1, 0.5, 2, 10, &mut out[..1], &mut ws)
            .unwrap_err();
    assert!(matches!(err, MathError::InvalidParameter { .. }));
}
