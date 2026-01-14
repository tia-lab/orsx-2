use crate::signal::filtering::savgol::{
    compute_savgol_coeffs_general, savgol_apply_into_with_workspace, EdgeMode, SavGolWorkspace,
};
use crate::{MathError, MathResult};

fn gen_poly(n: usize, a0: f64, a1: f64, a2: f64) -> Vec<f64> {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f64;
        out.push(a0 + a1 * t + a2 * t * t);
    }
    out
}

#[test]
fn test_savgol_rejects_invalid_parameters() {
    let x = vec![1.0; 11];
    let mut out = vec![0.0; 11];
    let mut ws = SavGolWorkspace::with_capacity(11);

    assert!(savgol_apply_into_with_workspace(
        &x,
        2,
        1,
        0,
        1.0,
        EdgeMode::Nearest,
        &mut out,
        &mut ws
    )
    .is_err());
    assert!(savgol_apply_into_with_workspace(
        &x,
        10,
        1,
        0,
        1.0,
        EdgeMode::Nearest,
        &mut out,
        &mut ws
    )
    .is_err());
    assert!(savgol_apply_into_with_workspace(
        &x,
        11,
        0,
        0,
        1.0,
        EdgeMode::Nearest,
        &mut out,
        &mut ws
    )
    .is_err());
    assert!(savgol_apply_into_with_workspace(
        &x,
        11,
        11,
        0,
        1.0,
        EdgeMode::Nearest,
        &mut out,
        &mut ws
    )
    .is_err());
    assert!(savgol_apply_into_with_workspace(
        &x,
        11,
        3,
        4,
        1.0,
        EdgeMode::Nearest,
        &mut out,
        &mut ws
    )
    .is_err());
    assert!(savgol_apply_into_with_workspace(
        &x,
        11,
        3,
        1,
        0.0,
        EdgeMode::Nearest,
        &mut out,
        &mut ws
    )
    .is_err());
}

#[test]
fn test_savgol_rejects_non_finite_and_shapes() {
    let x = vec![1.0, 2.0, f64::NAN, 4.0, 5.0, 6.0, 7.0];
    let mut out = vec![0.0; 7];
    let mut ws = SavGolWorkspace::with_capacity(5);
    let err =
        savgol_apply_into_with_workspace(&x, 5, 2, 0, 1.0, EdgeMode::Nearest, &mut out, &mut ws)
            .unwrap_err();
    assert!(matches!(err, MathError::InvalidData(_)));

    let x = vec![1.0; 7];
    let mut out_short = vec![0.0; 6];
    let err = savgol_apply_into_with_workspace(
        &x,
        5,
        2,
        0,
        1.0,
        EdgeMode::Nearest,
        &mut out_short,
        &mut ws,
    )
    .unwrap_err();
    assert!(matches!(err, MathError::InvalidParameter { .. }));

    let x_short = vec![1.0; 4];
    let mut out2 = vec![0.0; 4];
    let err = savgol_apply_into_with_workspace(
        &x_short,
        5,
        2,
        0,
        1.0,
        EdgeMode::Nearest,
        &mut out2,
        &mut ws,
    )
    .unwrap_err();
    assert!(matches!(err, MathError::InvalidParameter { .. }));
}

#[test]
fn test_savgol_exact_on_quadratic_interior_for_smoothing() -> MathResult<()> {
    let n = 200usize;
    let x = gen_poly(n, 3.0, -1.7, 0.05);
    let mut out = vec![0.0f64; n];
    let window_len = 11usize;
    let poly_order = 2usize;
    let mut ws = SavGolWorkspace::with_capacity(window_len);

    savgol_apply_into_with_workspace(
        &x,
        window_len,
        poly_order,
        0,
        1.0,
        EdgeMode::Nearest,
        &mut out,
        &mut ws,
    )?;

    let half = (window_len - 1) / 2;
    for i in half..(n - half) {
        let d = (out[i] - x[i]).abs();
        assert!(d <= 1e-10, "i={i} d={d} out={} x={}", out[i], x[i]);
    }
    Ok(())
}

#[test]
fn test_savgol_derivative_linear_is_exact_interior() -> MathResult<()> {
    let n = 200usize;
    let slope = 2.25;
    let x = gen_poly(n, 1.0, slope, 0.0);
    let mut out = vec![0.0f64; n];
    let window_len = 9usize;
    let poly_order = 2usize;
    let mut ws = SavGolWorkspace::with_capacity(window_len);

    savgol_apply_into_with_workspace(
        &x,
        window_len,
        poly_order,
        1,
        1.0,
        EdgeMode::Nearest,
        &mut out,
        &mut ws,
    )?;

    let half = (window_len - 1) / 2;
    for i in half..(n - half) {
        let d = (out[i] - slope).abs();
        assert!(d <= 1e-10, "i={i} d={d} out={}", out[i]);
    }
    Ok(())
}

#[test]
fn test_savgol_second_derivative_quadratic_is_constant_interior() -> MathResult<()> {
    let n = 200usize;
    let a2 = 0.125;
    let x = gen_poly(n, 0.0, 0.0, a2);
    let mut out = vec![0.0f64; n];
    let window_len = 11usize;
    let poly_order = 2usize;
    let mut ws = SavGolWorkspace::with_capacity(window_len);

    savgol_apply_into_with_workspace(
        &x,
        window_len,
        poly_order,
        2,
        1.0,
        EdgeMode::Nearest,
        &mut out,
        &mut ws,
    )?;

    let expected = 2.0 * a2;
    let half = (window_len - 1) / 2;
    for i in half..(n - half) {
        let d = (out[i] - expected).abs();
        assert!(d <= 1e-10, "i={i} d={d} out={}", out[i]);
    }
    Ok(())
}

#[test]
fn test_savgol_delta_scales_derivative() -> MathResult<()> {
    let n = 200usize;
    let slope = -3.0;
    let delta = 0.5;
    let x = gen_poly(n, 1.0, slope * delta, 0.0);

    let mut out = vec![0.0f64; n];
    let window_len = 9usize;
    let poly_order = 2usize;
    let mut ws = SavGolWorkspace::with_capacity(window_len);

    savgol_apply_into_with_workspace(
        &x,
        window_len,
        poly_order,
        1,
        delta,
        EdgeMode::Nearest,
        &mut out,
        &mut ws,
    )?;

    let half = (window_len - 1) / 2;
    for i in half..(n - half) {
        let d = (out[i] - slope).abs();
        assert!(d <= 1e-10, "i={i} d={d} out={}", out[i]);
    }
    Ok(())
}

#[test]
fn test_savgol_determinism_and_workspace_reuse() -> MathResult<()> {
    let n = 1000usize;
    let mut x = Vec::with_capacity(n);
    let mut s = 7u64;
    for _ in 0..n {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let u = ((s >> 11) as f64) * (1.0 / ((1u64 << 53) as f64));
        x.push(2.0 * u - 1.0);
    }

    let window_len = 11usize;
    let poly_order = 3usize;
    let mut ws = SavGolWorkspace::with_capacity(window_len);
    let mut a = vec![0.0f64; n];
    let mut b = vec![0.0f64; n];

    for _ in 0..5 {
        savgol_apply_into_with_workspace(
            &x,
            window_len,
            poly_order,
            0,
            1.0,
            EdgeMode::Nearest,
            &mut a,
            &mut ws,
        )?;
        savgol_apply_into_with_workspace(
            &x,
            window_len,
            poly_order,
            0,
            1.0,
            EdgeMode::Nearest,
            &mut b,
            &mut ws,
        )?;
        assert_eq!(a, b);
    }
    Ok(())
}

#[test]
fn test_savgol_numerical_stability_large_offset() -> MathResult<()> {
    let n = 1000usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        x.push(1e12 + (i as f64) * 1e-3);
    }
    let mut out = vec![0.0f64; n];
    let mut ws = SavGolWorkspace::with_capacity(11);
    savgol_apply_into_with_workspace(&x, 11, 3, 0, 1.0, EdgeMode::Nearest, &mut out, &mut ws)?;
    assert!(out.iter().all(|v| v.is_finite()));
    Ok(())
}

#[test]
fn test_savgol_no_panic_on_error_paths() {
    let r = std::panic::catch_unwind(|| {
        let x = vec![1.0; 4];
        let mut out = vec![0.0; 4];
        let mut ws = SavGolWorkspace::with_capacity(5);
        let _ = savgol_apply_into_with_workspace(
            &x,
            5,
            2,
            0,
            1.0,
            EdgeMode::Nearest,
            &mut out,
            &mut ws,
        );
    });
    assert!(r.is_ok());
}

#[test]
fn test_savgol_precomputed_coeffs_match_general_for_w11_p3_delta1() -> MathResult<()> {
    let window_len = 11usize;
    let poly_order = 3usize;
    let delta = 1.0;
    for deriv_order in 0..=2usize {
        let general = compute_savgol_coeffs_general(window_len, poly_order, deriv_order, delta)?;
        let mut ws = SavGolWorkspace::with_capacity(window_len);
        let mut dummy_out = vec![0.0f64; window_len];
        let x = vec![0.0f64; window_len];
        savgol_apply_into_with_workspace(
            &x,
            window_len,
            poly_order,
            deriv_order,
            delta,
            EdgeMode::Nearest,
            &mut dummy_out,
            &mut ws,
        )?;
        // Workspace coefficients are not directly exposed; compare via applying to basis vectors.
        // Use the fact that applying to delta impulses yields coefficients under Nearest interior.
        for j in 0..window_len {
            let mut e = vec![0.0f64; window_len];
            e[j] = 1.0;
            let mut out = vec![0.0f64; window_len];
            savgol_apply_into_with_workspace(
                &e,
                window_len,
                poly_order,
                deriv_order,
                delta,
                EdgeMode::Nearest,
                &mut out,
                &mut ws,
            )?;
            // Compare the center output: it is exactly the coefficient at position j.
            let c = out[window_len / 2];
            let expected = general[j];
            assert!(
                (c - expected).abs() <= 1e-12,
                "deriv={deriv_order} j={j} c={c} expected={expected}"
            );
        }
    }
    Ok(())
}
