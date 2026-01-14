use crate::signal::multifractal::wtmm::calculate_wtmm_partition_functions;
use crate::MathResult;

#[test]
fn test_wtmm_partition_shapes_and_finiteness() -> MathResult<()> {
    let n = 128usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        x.push((i as f64 * 0.15).sin());
    }
    let scales = [2.0, 4.0, 8.0];
    let q = [0.0, 2.0];
    let z = calculate_wtmm_partition_functions(&x, &scales, &q)?;
    assert_eq!(z.len(), q.len());
    for row in z.iter() {
        assert_eq!(row.len(), scales.len());
        assert!(row.iter().all(|v| v.is_finite() && *v >= 0.0));
    }
    Ok(())
}

#[test]
fn test_wtmm_rejects_invalid() {
    let x = vec![1.0, 2.0, 3.0, 4.0];
    let scales = [0.0];
    let q = [2.0];
    assert!(calculate_wtmm_partition_functions(&x, &scales, &q).is_err());
    let x2 = vec![
        1.0,
        f64::NAN,
        2.0,
        3.0,
        4.0,
        5.0,
        6.0,
        7.0,
        8.0,
        9.0,
        10.0,
        11.0,
        12.0,
        13.0,
        14.0,
        15.0,
    ];
    let scales2 = [2.0];
    assert!(calculate_wtmm_partition_functions(&x2, &scales2, &q).is_err());
}
