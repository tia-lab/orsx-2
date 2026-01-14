use crate::signal::filtering::biquad::{
    biquad_highpass_butterworth, biquad_lowpass_butterworth, BiquadCoeffs, BiquadDf2T,
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
fn test_biquad_coeff_failure_contracts() {
    assert!(biquad_lowpass_butterworth(0.0).is_err());
    assert!(biquad_lowpass_butterworth(0.5).is_err());
    assert!(biquad_lowpass_butterworth(-0.1).is_err());
    assert!(biquad_lowpass_butterworth(f64::NAN).is_err());

    assert!(biquad_highpass_butterworth(0.0).is_err());
    assert!(biquad_highpass_butterworth(0.5).is_err());
}

#[test]
fn test_biquad_rejects_unstable_coeffs() {
    let unstable = BiquadCoeffs {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 1.0, // violates 1-a2 > 0
    };
    let err = BiquadDf2T::new(unstable).unwrap_err();
    assert!(matches!(err, MathError::InvalidParameter { .. }));
}

#[test]
fn test_biquad_identity_is_exact() -> MathResult<()> {
    let x = gen_seeded(512, 7);
    let mut y = vec![0.0f64; x.len()];

    let mut f = BiquadDf2T::new(BiquadCoeffs::identity())?;
    f.apply_into(&x, &mut y)?;
    assert_eq!(x, y);

    f.reset_state();
    let mut z = x.clone();
    f.apply_in_place(&mut z)?;
    assert_eq!(x, z);
    Ok(())
}

#[test]
fn test_biquad_linearity_scaling() -> MathResult<()> {
    let x = gen_seeded(1000, 11);
    let c = -3.25f64;
    let mut x2 = Vec::with_capacity(x.len());
    for &v in x.iter() {
        x2.push(c * v);
    }

    let coeffs = biquad_lowpass_butterworth(0.05)?;

    let mut y1 = vec![0.0f64; x.len()];
    let mut y2 = vec![0.0f64; x.len()];

    let mut f1 = BiquadDf2T::new(coeffs)?;
    f1.apply_into(&x, &mut y1)?;

    let mut f2 = BiquadDf2T::new(coeffs)?;
    f2.apply_into(&x2, &mut y2)?;

    for (a, b) in y1.iter().zip(y2.iter()) {
        assert!((*b - c * (*a)).abs() <= 1e-12);
    }
    Ok(())
}

#[test]
fn test_biquad_filters_produce_finite_output() -> MathResult<()> {
    let x = gen_seeded(10_000, 123);
    let mut out = vec![0.0f64; x.len()];

    let coeffs_lp = biquad_lowpass_butterworth(0.05)?;
    let mut lp = BiquadDf2T::new(coeffs_lp)?;
    lp.apply_into(&x, &mut out)?;
    assert!(out.iter().all(|v| v.is_finite()));

    let coeffs_hp = biquad_highpass_butterworth(0.05)?;
    let mut hp = BiquadDf2T::new(coeffs_hp)?;
    hp.apply_into(&x, &mut out)?;
    assert!(out.iter().all(|v| v.is_finite()));
    Ok(())
}

#[test]
fn test_biquad_failure_contracts_and_no_panic() {
    let r = std::panic::catch_unwind(|| {
        let x = vec![1.0, 2.0, f64::NAN];
        let mut out = vec![0.0; 3];
        let coeffs = BiquadCoeffs::identity();
        let mut f = BiquadDf2T::new(coeffs).expect("identity biquad must be valid");
        let _ = f.apply_into(&x, &mut out);
    });
    assert!(r.is_ok());

    let x = vec![1.0, 2.0, 3.0];
    let mut out_short = vec![0.0; 2];
    let mut f = BiquadDf2T::new(BiquadCoeffs::identity()).expect("identity biquad must be valid");
    assert!(f.apply_into(&x, &mut out_short).is_err());

    let err = f.apply_into(&[], &mut []).unwrap_err();
    assert!(matches!(err, MathError::InsufficientDataAlgo { .. }));
}

#[test]
fn test_biquad_determinism_and_state_reset() -> MathResult<()> {
    let x = gen_seeded(2000, 99);
    let coeffs = biquad_lowpass_butterworth(0.07)?;

    let mut y1 = vec![0.0f64; x.len()];
    let mut y2 = vec![0.0f64; x.len()];

    let mut f = BiquadDf2T::new(coeffs)?;
    f.apply_into(&x, &mut y1)?;
    f.reset_state();
    f.apply_into(&x, &mut y2)?;

    assert_eq!(y1, y2);
    Ok(())
}
