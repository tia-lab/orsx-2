use crate::signal::multifractal::mfdfa::{
    calculate_mfdfa, calculate_mfdfa_into_with_workspace, MfDfaWorkspace,
};
use crate::signal::multifractal::wtmm::{
    calculate_wtmm_partition_functions, calculate_wtmm_partition_functions_into_with_workspace,
    WtmmWorkspace,
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
fn test_mfdfa_workspace_equivalence_and_determinism() -> MathResult<()> {
    let x = gen_seeded(1024, 10);
    let scales = [32usize, 64usize];
    let q = [0.0, 2.0];
    let alloc = calculate_mfdfa(&x, &scales, &q, 1)?;

    let mut ws = MfDfaWorkspace::with_capacity(x.len());
    let mut out = vec![0.0f64; q.len() * scales.len()];
    for _ in 0..20 {
        calculate_mfdfa_into_with_workspace(&x, &scales, &q, 1, &mut out, &mut ws)?;
        for q_idx in 0..q.len() {
            for s_idx in 0..scales.len() {
                let a = alloc.fluctuation_functions[q_idx][s_idx];
                let b = out[q_idx * scales.len() + s_idx];
                assert!((a - b).abs() <= 1e-10);
                assert!(b.is_finite() && b >= 0.0);
            }
        }
    }
    Ok(())
}

#[test]
fn test_wtmm_workspace_equivalence_and_determinism() -> MathResult<()> {
    let x = gen_seeded(512, 11);
    let scales = [2.0, 4.0, 8.0];
    let q = [0.0, 2.0];
    let alloc = calculate_wtmm_partition_functions(&x, &scales, &q)?;

    let mut ws = WtmmWorkspace::with_capacity(x.len());
    let mut out = vec![0.0f64; q.len() * scales.len()];
    for _ in 0..10 {
        calculate_wtmm_partition_functions_into_with_workspace(&x, &scales, &q, &mut out, &mut ws)?;
        for q_idx in 0..q.len() {
            for s_idx in 0..scales.len() {
                let a = alloc[q_idx][s_idx];
                let b = out[q_idx * scales.len() + s_idx];
                assert!((a - b).abs() <= 1e-10);
                assert!(b.is_finite() && b >= 0.0);
            }
        }
    }
    Ok(())
}
