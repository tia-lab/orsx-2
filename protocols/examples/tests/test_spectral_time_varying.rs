use crate::signal::spectral::time_varying::{stft_periodograms, stft_periodograms_windowed};
use crate::signal::types::{DetrendMethod, WindowFunction};
use crate::MathResult;

#[test]
fn test_stft_periodograms_window_count_and_shapes() -> MathResult<()> {
    let n = 256usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        x.push((i as f64 * 0.1).sin() + 0.01 * (i as f64));
    }
    let windows = stft_periodograms(&x, 64, 32, DetrendMethod::RemoveMean, 100)?;
    assert!(!windows.is_empty());
    for w in windows.iter() {
        assert_eq!(w.len(), 64);
        assert!(w.iter().all(|v| v.is_finite() && *v >= 0.0));
    }
    Ok(())
}

#[test]
fn test_stft_periodograms_rejects_invalid() {
    let x = vec![1.0, 2.0, 3.0, 4.0];
    assert!(stft_periodograms(&x, 3, 1, DetrendMethod::None, 10).is_err());
    assert!(stft_periodograms(&x, 4, 0, DetrendMethod::None, 10).is_err());
    let x2 = vec![1.0, f64::NAN, 3.0, 4.0, 5.0, 6.0];
    assert!(stft_periodograms(&x2, 4, 1, DetrendMethod::None, 10).is_err());
}

#[test]
fn test_stft_windowed_rectangular_matches_unwindowed() -> MathResult<()> {
    let n = 512usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        x.push((i as f64 * 0.07).sin() + 0.05 * (i as f64));
    }

    let a = stft_periodograms(&x, 64, 16, DetrendMethod::RemoveMean, 10)?;
    let b = stft_periodograms_windowed(
        &x,
        64,
        16,
        WindowFunction::Rectangular,
        DetrendMethod::RemoveMean,
        10,
    )?;

    assert_eq!(a.len(), b.len());
    for (row_a, row_b) in a.iter().zip(b.iter()) {
        assert_eq!(row_a.len(), row_b.len());
        for (va, vb) in row_a.iter().zip(row_b.iter()) {
            assert!((va - vb).abs() <= 1e-12);
        }
    }
    Ok(())
}

#[test]
fn test_stft_windowed_hann_changes_spectrum_deterministically() -> MathResult<()> {
    let n = 256usize;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        x.push((i as f64 * 0.17).sin() + 0.001 * (i as f64));
    }

    let a = stft_periodograms(&x, 64, 32, DetrendMethod::RemoveMean, 10)?;
    let b = stft_periodograms_windowed(
        &x,
        64,
        32,
        WindowFunction::Hann,
        DetrendMethod::RemoveMean,
        10,
    )?;

    assert_eq!(a.len(), b.len());
    // Hann window should change at least one bin for at least one window.
    let mut any_change = false;
    for (row_a, row_b) in a.iter().zip(b.iter()) {
        for (va, vb) in row_a.iter().zip(row_b.iter()) {
            if (va - vb).abs() > 1e-12 {
                any_change = true;
                break;
            }
        }
        if any_change {
            break;
        }
    }
    assert!(any_change);

    // Determinism: second call equals first call exactly (same inputs, no RNG).
    let c = stft_periodograms_windowed(
        &x,
        64,
        32,
        WindowFunction::Hann,
        DetrendMethod::RemoveMean,
        10,
    )?;
    assert_eq!(b, c);
    Ok(())
}
