use crate::signal::multifractal::mfdfa::calculate_mfdfa;
use crate::MathResult;

#[test]
fn test_mfdfa_constant_series_gives_near_zero_fluctuations() -> MathResult<()> {
    let x = vec![1.0f64; 512];
    let scales = [32usize, 64usize];
    let q = [0.0, 2.0];
    let out = calculate_mfdfa(&x, &scales, &q, 1)?;
    for (qi, fq) in out.fluctuation_functions.iter().enumerate() {
        for (si, &v) in fq.iter().enumerate() {
            assert!(v.is_finite(), "q_idx={qi} s_idx={si} v={v}");
            assert!(v <= 1e-8, "q_idx={qi} s_idx={si} v={v}");
        }
    }
    Ok(())
}

#[test]
fn test_mfdfa_negative_q_rejects_zero_fluctuations() {
    let x = vec![1.0f64; 512];
    let scales = [32usize, 64usize];
    let q = [-2.0];
    assert!(calculate_mfdfa(&x, &scales, &q, 1).is_err());
}

#[test]
fn test_mfdfa_rejects_invalid_inputs() {
    let x = vec![1.0f64; 128];
    let scales = [33usize];
    let q = [2.0];
    assert!(calculate_mfdfa(&x, &scales, &q, 1).is_err()); // scale too small / scale > n/4 constraints

    let x_ok = vec![1.0f64; 128];
    let scales_ok = [32usize];
    let q_ok = [2.0];
    assert!(calculate_mfdfa(&x_ok, &scales_ok, &q_ok, 0).is_err()); // invalid degree
    let mut x2 = vec![1.0f64; 128];
    x2[1] = f64::NAN;
    let scales2 = [32usize];
    assert!(calculate_mfdfa(&x2, &scales2, &q, 1).is_err());
}
