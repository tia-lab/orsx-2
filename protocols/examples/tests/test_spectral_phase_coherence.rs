use crate::signal::spectral::{
    phase_locking_value_from_phase_with_config, phase_locking_value_from_signals_with_workspace,
    PhaseLockingValueConfig, PhaseLockingValueWorkspace,
};
use crate::MathError;

fn sin_series(n: usize, cycles: f64, phase: f64) -> Vec<f64> {
    let mut x = vec![0.0; n];
    let two_pi = std::f64::consts::TAU;
    for i in 0..n {
        let t = i as f64 / (n.saturating_sub(1) as f64);
        x[i] = (two_pi * cycles * t + phase).sin();
    }
    x
}

#[test]
fn test_plv_from_phase_constant_offset_is_one() {
    let n = 512;
    let phase_x: Vec<f64> = (0..n).map(|i| 0.01 * (i as f64)).collect();
    let delta = 1.234;
    let phase_y: Vec<f64> = phase_x.iter().map(|p| *p - delta).collect();

    let cfg = PhaseLockingValueConfig {
        min_amplitude: 0.0,
        min_samples: 10,
    };
    let plv = phase_locking_value_from_phase_with_config(&phase_x, &phase_y, &cfg).unwrap();
    assert!((plv - 1.0).abs() <= 1e-12);
}

#[test]
fn test_plv_from_phase_balanced_opposite_offsets_is_zero_like() {
    let n = 400;
    let base: Vec<f64> = (0..n).map(|i| 0.03 * (i as f64)).collect();
    let mut px = vec![0.0; n];
    let mut py = vec![0.0; n];
    for i in 0..n {
        px[i] = base[i];
        py[i] = base[i]
            + if i % 2 == 0 {
                0.0
            } else {
                std::f64::consts::PI
            };
    }
    let cfg = PhaseLockingValueConfig::default();
    let plv = phase_locking_value_from_phase_with_config(&px, &py, &cfg).unwrap();
    assert!(plv <= 1e-12, "plv={plv}");
}

#[test]
fn test_plv_from_signals_identical_sinusoids_is_high() {
    let n = 1024;
    let x = sin_series(n, 5.0, 0.0);
    let y = sin_series(n, 5.0, 0.0);
    let cfg = PhaseLockingValueConfig {
        min_amplitude: 1e-6,
        min_samples: 100,
    };

    let mut ws = PhaseLockingValueWorkspace::with_capacity(n);
    let plv = phase_locking_value_from_signals_with_workspace(&x, &y, &cfg, &mut ws).unwrap();
    assert!(plv >= 0.99, "plv={plv}");
}

#[test]
fn test_plv_from_signals_constant_phase_shift_is_high() {
    let n = 1024;
    let shift = 0.7;
    let x = sin_series(n, 7.0, 0.0);
    let y = sin_series(n, 7.0, shift);
    let cfg = PhaseLockingValueConfig {
        min_amplitude: 1e-6,
        min_samples: 100,
    };

    let mut ws = PhaseLockingValueWorkspace::with_capacity(n);
    let plv = phase_locking_value_from_signals_with_workspace(&x, &y, &cfg, &mut ws).unwrap();
    assert!(plv >= 0.98, "plv={plv}");
}

#[test]
fn test_plv_rejects_invalid_inputs() {
    let cfg = PhaseLockingValueConfig::default();
    let mut ws = PhaseLockingValueWorkspace::with_capacity(4);

    let x = vec![1.0, 2.0, 3.0, 4.0];
    let y = vec![1.0, 2.0, 3.0];
    assert!(matches!(
        phase_locking_value_from_signals_with_workspace(&x, &y, &cfg, &mut ws),
        Err(MathError::InvalidData(_))
    ));

    let y = vec![1.0, f64::NAN, 3.0, 4.0];
    assert!(matches!(
        phase_locking_value_from_signals_with_workspace(&x, &y, &cfg, &mut ws),
        Err(MathError::InvalidData(_))
    ));
}

#[test]
fn test_plv_amplitude_gating_can_fail_insufficient_data() {
    let n = 256;
    let x = sin_series(n, 3.0, 0.0);
    let y = sin_series(n, 3.0, 0.1);
    let cfg = PhaseLockingValueConfig {
        min_amplitude: 1e6,
        min_samples: 8,
    };
    let mut ws = PhaseLockingValueWorkspace::with_capacity(n);
    assert!(matches!(
        phase_locking_value_from_signals_with_workspace(&x, &y, &cfg, &mut ws),
        Err(MathError::InsufficientDataAlgo { .. })
    ));
}
