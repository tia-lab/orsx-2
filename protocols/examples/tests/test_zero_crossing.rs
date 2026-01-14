use crate::signal::zero_crossing::{sign_run_stats, zero_crossing_rate, ZeroHandling};
use crate::{MathError, MathResult};

#[test]
fn test_zero_crossing_rate_basic_as_zero() -> MathResult<()> {
    // signs: + + - - + => changes at 2 and 4 => 2/(4)=0.5
    let x = vec![1.0, 2.0, -1.0, -2.0, 3.0];
    let zcr = zero_crossing_rate(&x, ZeroHandling::AsZero)?;
    assert!((zcr - 0.5).abs() <= 1e-15);
    Ok(())
}

#[test]
fn test_zero_crossing_rate_zero_handling_differs() -> MathResult<()> {
    // Sequence includes zeros; AsZero counts transitions into/out of zero.
    let x = vec![1.0, 0.0, -1.0];
    let z_as = zero_crossing_rate(&x, ZeroHandling::AsZero)?;
    let z_cf = zero_crossing_rate(&x, ZeroHandling::CarryForward)?;
    let z_pos = zero_crossing_rate(&x, ZeroHandling::MapToPositive)?;

    assert!((z_as - 1.0).abs() <= 1e-15); // + -> 0 -> - : two changes /2 =1
    assert!((z_cf - 0.5).abs() <= 1e-15); // + -> + -> - : one change /2 =0.5
    assert!((z_pos - 0.5).abs() <= 1e-15); // + -> + -> - : one change /2 =0.5
    Ok(())
}

#[test]
fn test_sign_run_stats_counts_runs() -> MathResult<()> {
    // labels AsZero: + + + 0 0 - - + => runs: [3,+], [2,0], [2,-], [1,+]
    let x = vec![1.0, 2.0, 3.0, 0.0, 0.0, -1.0, -2.0, 4.0];
    let st = sign_run_stats(&x, ZeroHandling::AsZero)?;
    assert_eq!(st.runs, 4);
    assert_eq!(st.pos_runs, 2);
    assert_eq!(st.neg_runs, 1);
    assert_eq!(st.zero_runs, 1);
    assert_eq!(st.max_run_length, 3);
    assert!((st.mean_run_length - 2.0).abs() <= 1e-15);
    Ok(())
}

#[test]
fn test_sign_run_stats_carry_forward_removes_zero_runs() -> MathResult<()> {
    let x = vec![1.0, 0.0, 0.0, -1.0, 0.0, 2.0];
    // CarryForward: + + + - - + => runs: [3,+], [2,-], [1,+]
    let st = sign_run_stats(&x, ZeroHandling::CarryForward)?;
    assert_eq!(st.runs, 3);
    assert_eq!(st.zero_runs, 0);
    assert_eq!(st.pos_runs, 2);
    assert_eq!(st.neg_runs, 1);
    assert_eq!(st.max_run_length, 3);
    Ok(())
}

#[test]
fn test_zero_crossing_failure_contracts_and_no_panic() {
    let r = std::panic::catch_unwind(|| {
        let x = vec![1.0, f64::NAN, 2.0];
        let _ = zero_crossing_rate(&x, ZeroHandling::AsZero);
        let _ = sign_run_stats(&x, ZeroHandling::AsZero);
    });
    assert!(r.is_ok());

    let x = vec![1.0f64];
    let err = zero_crossing_rate(&x, ZeroHandling::AsZero).unwrap_err();
    assert!(matches!(err, MathError::InsufficientDataAlgo { .. }));
}
