use crate::signal::spectral::coherence::magnitude_squared_coherence;
use crate::signal::types::DetrendMethod;
use crate::MathResult;

#[test]
fn test_coherence_identity_is_one_everywhere() -> MathResult<()> {
    let n = 64usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        x.push((i as f64 * 0.2).sin() + 0.3 * (i as f64 * 0.05).cos());
    }
    let c = magnitude_squared_coherence(&x, &x, DetrendMethod::RemoveMean)?;
    for (k, &v) in c.iter().enumerate() {
        assert!(v.is_finite());
        assert!((v - 1.0).abs() <= 1e-10, "k={k} v={v}");
    }
    Ok(())
}

#[test]
fn test_coherence_rejects_non_finite_or_len_mismatch() {
    let x = vec![1.0, 2.0, 3.0, 4.0];
    let y = vec![1.0, 2.0, 3.0];
    assert!(magnitude_squared_coherence(&x, &y, DetrendMethod::None).is_err());
    let y2 = vec![1.0, f64::NAN, 3.0, 4.0];
    assert!(magnitude_squared_coherence(&x, &y2, DetrendMethod::None).is_err());
}
