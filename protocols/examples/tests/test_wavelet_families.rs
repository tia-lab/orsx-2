use crate::core::variance::variance_biased;
use crate::signal::types::WaveletFamily;
use crate::signal::wavelets::{
    modwt_denoise_into_with_workspace, modwt_detail_level, ModwtDenoiseWorkspace, ThresholdKind,
};
use crate::MathResult;

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
fn test_modwt_detail_constant_is_near_zero_for_families() -> MathResult<()> {
    let x = vec![2.0f64; 512];
    for &family in &[
        WaveletFamily::ModwtD4,
        WaveletFamily::ModwtD6,
        WaveletFamily::ModwtD8,
    ] {
        let w = modwt_detail_level(&x, family, 1)?;
        let mean_sq = w.iter().map(|v| v * v).sum::<f64>() / (w.len() as f64);
        assert!(mean_sq <= 1e-12, "family={family:?} mean_sq={mean_sq:e}");
    }
    Ok(())
}

#[test]
fn test_modwt_denoise_threshold_zero_is_identity_for_families_pow2_and_non_pow2() -> MathResult<()>
{
    for &n in &[1024usize, 1000usize] {
        let levels = 6usize.min((usize::BITS as usize) - 1 - (n as u64).leading_zeros() as usize);
        let x = gen_seeded(n, 77);
        for &family in &[
            WaveletFamily::ModwtD4,
            WaveletFamily::ModwtD6,
            WaveletFamily::ModwtD8,
        ] {
            for &kind in &[ThresholdKind::Hard, ThresholdKind::Soft] {
                let mut out = vec![0.0f64; n];
                let mut ws = ModwtDenoiseWorkspace::with_capacity(n, levels)?;
                modwt_denoise_into_with_workspace(
                    &x, family, levels, 0.0, kind, &mut out, &mut ws,
                )?;
                let mut max_err = 0.0f64;
                for i in 0..n {
                    max_err = max_err.max((out[i] - x[i]).abs());
                }
                assert!(
                    max_err <= 5e-11,
                    "n={n} family={family:?} kind={kind:?} max_err={max_err:e}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn test_modwt_denoise_reduces_variance_for_noise_across_families() -> MathResult<()> {
    let n = 2048usize;
    let levels = 6usize;
    let x = gen_seeded(n, 123);
    let v_in = variance_biased(&x)?;
    for &family in &[
        WaveletFamily::ModwtD4,
        WaveletFamily::ModwtD6,
        WaveletFamily::ModwtD8,
    ] {
        let mut out = vec![0.0f64; n];
        let mut ws = ModwtDenoiseWorkspace::with_capacity(n, levels)?;
        modwt_denoise_into_with_workspace(
            &x,
            family,
            levels,
            0.25,
            ThresholdKind::Soft,
            &mut out,
            &mut ws,
        )?;
        let v_out = variance_biased(&out)?;
        assert!(v_out.is_finite() && v_out >= 0.0);
        assert!(
            v_out <= v_in + 1e-12,
            "family={family:?} v_in={v_in} v_out={v_out}"
        );
    }
    Ok(())
}
