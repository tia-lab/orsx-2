use crate::signal::entropy::{
    permutation_entropy_into_with_workspace, sample_entropy_chebyshev,
    sample_entropy_chebyshev_exact_sorted_window, PermutationEntropyWorkspace,
    SampleEntropyWorkspace,
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
fn test_permutation_entropy_failure_contracts_and_no_panic() {
    let r = std::panic::catch_unwind(|| {
        let x = vec![1.0, f64::NAN, 2.0, 3.0];
        let mut ws = PermutationEntropyWorkspace::with_capacity(3);
        let _ = permutation_entropy_into_with_workspace(&x, 3, 1, &mut ws);
    });
    assert!(r.is_ok());

    let x = vec![1.0, 2.0, 3.0];
    let mut ws = PermutationEntropyWorkspace::with_capacity(3);
    assert!(permutation_entropy_into_with_workspace(&x, 1, 1, &mut ws).is_err());
    assert!(permutation_entropy_into_with_workspace(&x, 3, 0, &mut ws).is_err());
    assert!(permutation_entropy_into_with_workspace(&x, 8, 1, &mut ws).is_err());
}

#[test]
fn test_permutation_entropy_constant_is_zero() -> MathResult<()> {
    let x = vec![7.0f64; 512];
    let mut ws = PermutationEntropyWorkspace::with_capacity(5);
    let (h, hn) = permutation_entropy_into_with_workspace(&x, 5, 1, &mut ws)?;
    assert!(h.abs() <= 1e-12, "h={h}");
    assert!(hn.abs() <= 1e-12, "hn={hn}");
    Ok(())
}

#[test]
fn test_permutation_entropy_in_range() -> MathResult<()> {
    let x = gen_seeded(2000, 11);
    let mut ws = PermutationEntropyWorkspace::with_capacity(5);
    let (h, hn) = permutation_entropy_into_with_workspace(&x, 5, 1, &mut ws)?;
    assert!(h.is_finite() && h >= 0.0);
    assert!(hn.is_finite() && hn >= 0.0 && hn <= 1.0);
    Ok(())
}

#[test]
fn test_permutation_entropy_tie_handling_is_deterministic() -> MathResult<()> {
    // Many ties by construction.
    let mut x = vec![0.0f64; 1000];
    for i in 0..x.len() {
        x[i] = (i % 3) as f64;
    }
    let mut ws = PermutationEntropyWorkspace::with_capacity(4);
    let (h1, hn1) = permutation_entropy_into_with_workspace(&x, 4, 1, &mut ws)?;
    let (h2, hn2) = permutation_entropy_into_with_workspace(&x, 4, 1, &mut ws)?;
    assert_eq!(h1, h2);
    assert_eq!(hn1, hn2);
    Ok(())
}

#[test]
fn test_sample_entropy_failure_contracts_and_no_panic() {
    let r = std::panic::catch_unwind(|| {
        let x = vec![1.0, f64::NAN, 2.0, 3.0];
        let mut ws = SampleEntropyWorkspace::default();
        let _ = sample_entropy_chebyshev(&x, 2, 1, 0.5, &mut ws);
    });
    assert!(r.is_ok());

    let x = vec![1.0, 2.0, 3.0, 4.0];
    let mut ws = SampleEntropyWorkspace::default();
    assert!(sample_entropy_chebyshev(&x, 0, 1, 0.5, &mut ws).is_err());
    assert!(sample_entropy_chebyshev(&x, 2, 0, 0.5, &mut ws).is_err());
    assert!(sample_entropy_chebyshev(&x, 2, 1, -1.0, &mut ws).is_err());
}

#[test]
fn test_sample_entropy_constant_matches_closed_form() -> MathResult<()> {
    // For a constant series and r=0, every pair matches.
    // With the standard SampEn definition where the number of template vectors differs
    // between m and m+1, the result is:
    //   SampEn = -ln( C(N_{m+1}, 2) / C(N_m, 2) )
    // where:
    //   N_m   = n - (m-1)*tau
    //   N_{m+1} = n - m*tau
    let n = 300usize;
    let m = 2usize;
    let tau = 1usize;
    let x = vec![5.0f64; n];
    let mut ws = SampleEntropyWorkspace::default();
    let se = sample_entropy_chebyshev(&x, m, tau, 0.0, &mut ws)?;

    let nm = (n - (m - 1) * tau) as f64;
    let nm1 = (n - m * tau) as f64;
    let b = nm * (nm - 1.0) * 0.5;
    let a = nm1 * (nm1 - 1.0) * 0.5;
    let expected = -(a / b).ln();

    assert!(
        (se - expected).abs() <= 1e-12,
        "se={se} expected={expected}"
    );
    Ok(())
}

#[test]
fn test_sample_entropy_errors_when_no_matches() {
    // Strict tolerance so that (almost surely) no pair matches for m+1.
    let x = gen_seeded(500, 7);
    let mut ws = SampleEntropyWorkspace::default();
    let err = sample_entropy_chebyshev(&x, 2, 1, 1e-12, &mut ws).unwrap_err();
    assert!(matches!(err, MathError::CalculationError(_)));
}

#[test]
fn test_sample_entropy_auto_fast_path_matches_baseline() -> MathResult<()> {
    // Choose n large enough to trigger the grid fast path, but keep m small (typical use).
    let x = gen_seeded(2000, 77);
    let m = 2usize;
    let tau = 1usize;
    let r = 0.2f64;

    let mut ws = SampleEntropyWorkspace::with_capacity(x.len());
    let a = sample_entropy_chebyshev(&x, m, tau, r, &mut ws)?;
    let b = sample_entropy_chebyshev_exact_sorted_window(&x, m, tau, r, &mut ws)?;

    let abs_err = (a - b).abs();
    let rel_err = abs_err / b.abs().max(1.0);
    assert!(
        abs_err <= 1e-10 || rel_err <= 1e-10,
        "a={a} b={b} abs_err={abs_err:e} rel_err={rel_err:e}"
    );
    Ok(())
}
