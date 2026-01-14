use crate::signal::dcca::{dcca_rho_into_with_workspace, DccaWorkspace};
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

fn dcca_rho_reference_slow(x: &[f64], y: &[f64], scale: usize) -> MathResult<f64> {
    if x.len() != y.len() {
        return Err(MathError::InvalidParameter {
            parameter: "y".to_string(),
            value: y.len() as f64,
            constraint: format!("must have length n={}", x.len()),
        });
    }
    let n = x.len();
    if n < 4 || scale < 4 {
        return Err(MathError::InvalidParameter {
            parameter: "scale".to_string(),
            value: scale as f64,
            constraint: "invalid scale".to_string(),
        });
    }
    if x.iter().any(|v| !v.is_finite()) || y.iter().any(|v| !v.is_finite()) {
        return Err(MathError::InvalidData("non-finite".to_string()));
    }

    // Integrate profiles.
    let mean_x = x.iter().sum::<f64>() / (n as f64);
    let mean_y = y.iter().sum::<f64>() / (n as f64);
    let mut xp = vec![0.0f64; n];
    let mut yp = vec![0.0f64; n];
    let mut ax = 0.0f64;
    let mut ay = 0.0f64;
    for i in 0..n {
        ax += x[i] - mean_x;
        ay += y[i] - mean_y;
        xp[i] = ax;
        yp[i] = ay;
    }

    let ns = n / scale;
    if ns < 2 {
        return Err(MathError::InvalidParameter {
            parameter: "scale".to_string(),
            value: scale as f64,
            constraint: "ns<2".to_string(),
        });
    }

    fn detrend_stats(segx: &[f64], segy: &[f64]) -> MathResult<(f64, f64, f64)> {
        let s = segx.len();
        let s_f = s as f64;
        let x_idx_mean = (s_f - 1.0) / 2.0;
        let mx = segx.iter().sum::<f64>() / s_f;
        let my = segy.iter().sum::<f64>() / s_f;
        let mut sxx = 0.0f64;
        let mut sxyx = 0.0f64;
        let mut sxyy = 0.0f64;
        for i in 0..s {
            let t = (i as f64) - x_idx_mean;
            sxx += t * t;
            sxyx += t * (segx[i] - mx);
            sxyy += t * (segy[i] - my);
        }
        if !(sxx.is_finite() && sxx > 0.0) {
            return Err(MathError::NumericalInstability("sxx".to_string()));
        }
        let bx = sxyx / sxx;
        let ax = mx - bx * x_idx_mean;
        let by = sxyy / sxx;
        let ay = my - by * x_idx_mean;
        let mut rx2 = 0.0f64;
        let mut ry2 = 0.0f64;
        let mut rxry = 0.0f64;
        for i in 0..s {
            let t = i as f64;
            let rx = segx[i] - (ax + bx * t);
            let ry = segy[i] - (ay + by * t);
            rx2 += rx * rx;
            ry2 += ry * ry;
            rxry += rx * ry;
        }
        Ok((rx2 / s_f, ry2 / s_f, rxry / s_f))
    }

    let mut fx = 0.0f64;
    let mut fy = 0.0f64;
    let mut fxy = 0.0f64;
    let mut windows = 0u64;
    for w in 0..ns {
        let start = w * scale;
        let end = start + scale;
        let (vx, vy, vxy) = detrend_stats(&xp[start..end], &yp[start..end])?;
        fx += vx;
        fy += vy;
        fxy += vxy;
        windows += 1;
    }
    for w in 0..ns {
        let end = n - w * scale;
        let start = end - scale;
        let (vx, vy, vxy) = detrend_stats(&xp[start..end], &yp[start..end])?;
        fx += vx;
        fy += vy;
        fxy += vxy;
        windows += 1;
    }
    let inv = 1.0 / (windows as f64);
    fx *= inv;
    fy *= inv;
    fxy *= inv;
    if fx <= 0.0 || fy <= 0.0 {
        return Err(MathError::CalculationError("zero variance".to_string()));
    }
    Ok(fxy / (fx * fy).sqrt())
}

#[test]
fn test_dcca_identical_series_rho_near_one() -> MathResult<()> {
    let n = 4096usize;
    let x = gen_seeded(n, 1);
    let y = x.clone();
    let scales = [64usize, 128usize, 256usize];
    let mut out = vec![0.0f64; scales.len()];
    let mut ws = DccaWorkspace::with_capacity(n);
    dcca_rho_into_with_workspace(&x, &y, &scales, &mut out, &mut ws)?;
    for &rho in out.iter() {
        assert!((rho - 1.0).abs() <= 1e-10, "rho={rho}");
    }
    Ok(())
}

#[test]
fn test_dcca_negated_series_rho_near_minus_one() -> MathResult<()> {
    let n = 4096usize;
    let x = gen_seeded(n, 2);
    let y: Vec<f64> = x.iter().map(|v| -v).collect();
    let scales = [64usize, 128usize, 256usize];
    let mut out = vec![0.0f64; scales.len()];
    let mut ws = DccaWorkspace::with_capacity(n);
    dcca_rho_into_with_workspace(&x, &y, &scales, &mut out, &mut ws)?;
    for &rho in out.iter() {
        assert!((rho + 1.0).abs() <= 1e-10, "rho={rho}");
    }
    Ok(())
}

#[test]
fn test_dcca_rejects_invalid_inputs() {
    let x = vec![1.0f64; 100];
    let y = vec![2.0f64; 99];
    let scales = [16usize];
    let mut out = vec![0.0f64; 1];
    let mut ws = DccaWorkspace::with_capacity(100);
    assert!(dcca_rho_into_with_workspace(&x, &y, &scales, &mut out, &mut ws).is_err());

    let y_ok = vec![2.0f64; 100];
    let scales_bad = [2usize];
    assert!(dcca_rho_into_with_workspace(&x, &y_ok, &scales_bad, &mut out, &mut ws).is_err());

    let scales_too_large = [80usize];
    assert!(dcca_rho_into_with_workspace(&x, &y_ok, &scales_too_large, &mut out, &mut ws).is_err());

    let x_nan = vec![1.0f64, f64::NAN, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
    let y_nan = vec![1.0f64; x_nan.len()];
    let scales_ok = [4usize];
    let mut out2 = vec![0.0f64; 1];
    let mut ws2 = DccaWorkspace::with_capacity(x_nan.len());
    assert!(dcca_rho_into_with_workspace(&x_nan, &y_nan, &scales_ok, &mut out2, &mut ws2).is_err());
}

#[test]
fn test_dcca_constant_series_errors_zero_variance() {
    let x = vec![3.0f64; 1024];
    let y = vec![3.0f64; 1024];
    let scales = [64usize];
    let mut out = vec![0.0f64; 1];
    let mut ws = DccaWorkspace::with_capacity(1024);
    let err = dcca_rho_into_with_workspace(&x, &y, &scales, &mut out, &mut ws).unwrap_err();
    assert!(matches!(err, MathError::CalculationError(_)));
}

#[test]
fn test_dcca_prefix_matches_reference_within_tolerance() -> MathResult<()> {
    let n = 2048usize;
    let x = gen_seeded(n, 3);
    let y = gen_seeded(n, 4);
    let scales = [32usize, 64usize, 128usize];
    let mut out = vec![0.0f64; scales.len()];
    let mut ws = DccaWorkspace::with_capacity(n);
    dcca_rho_into_with_workspace(&x, &y, &scales, &mut out, &mut ws)?;

    for (i, &s) in scales.iter().enumerate() {
        let ref_rho = dcca_rho_reference_slow(&x, &y, s)?;
        let got = out[i];
        let abs_err = (got - ref_rho).abs();
        let rel_err = abs_err / ref_rho.abs().max(1.0);
        assert!(
            abs_err <= 1e-10 || rel_err <= 1e-10,
            "scale={s} got={got} ref={ref_rho} abs_err={abs_err:e} rel_err={rel_err:e}"
        );
    }
    Ok(())
}
