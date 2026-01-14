use crate::core::autocorrelation::AutocorrelationNormalization;
use crate::signal::dcca::{dcca_rho_into_with_workspace, DccaWorkspace};
use crate::signal::mse::{multiscale_sample_entropy_into_with_workspace, MseWorkspace};
use crate::signal::multifractal::mfdfa::{calculate_mfdfa_into_with_workspace, MfDfaWorkspace};
use crate::signal::multifractal::wtmm::{
    calculate_wtmm_partition_functions_into_with_workspace, WtmmWorkspace,
};
use crate::signal::rqa::{rqa_metrics_with_workspace, RqaConfig, RqaSampling, RqaWorkspace};
use crate::signal::spectral::autocorrelation_fft::{
    calculate_autocorrelation_fft_into, AutocorrelationFftWorkspace,
};
use crate::signal::spectral::coherence::{magnitude_squared_coherence_into, CoherenceWorkspace};
use crate::signal::spectral::periodogram::{calculate_periodogram_into, PeriodogramWorkspace};
use crate::signal::spectral::time_varying::{
    stft_periodograms_flat_into_with_workspace, StftPeriodogramWorkspace,
};
use crate::signal::types::DetrendMethod;
use crate::signal::zero_crossing::{sign_run_stats, zero_crossing_rate, ZeroHandling};

#[test]
fn test_failure_contracts_return_err_not_panic() {
    let r = std::panic::catch_unwind(|| {
        // Periodogram: out len mismatch
        let x = vec![1.0f64; 64];
        let mut ws = PeriodogramWorkspace::with_capacity(x.len());
        let mut out = vec![0.0f64; 63];
        assert!(calculate_periodogram_into(&x, DetrendMethod::None, &mut out, &mut ws).is_err());

        // FFT ACF: out len mismatch
        let mut ws_ac = AutocorrelationFftWorkspace::with_capacity(x.len());
        let mut out_ac = vec![0.0f64; 3];
        assert!(calculate_autocorrelation_fft_into(
            &x,
            10,
            AutocorrelationNormalization::Biased,
            &mut out_ac,
            &mut ws_ac
        )
        .is_err());

        // Coherence: out len mismatch
        let y = vec![2.0f64; 64];
        let mut ws_c = CoherenceWorkspace::with_capacity(x.len());
        let mut out_c = vec![0.0f64; 63];
        assert!(magnitude_squared_coherence_into(
            &x,
            &y,
            DetrendMethod::RemoveMean,
            &mut out_c,
            &mut ws_c
        )
        .is_err());

        // STFT flat: out too small
        let mut ws_s = StftPeriodogramWorkspace::with_capacity(32);
        let mut out_s = vec![0.0f64; 10];
        assert!(stft_periodograms_flat_into_with_workspace(
            &x,
            32,
            16,
            DetrendMethod::RemoveMean,
            10,
            &mut out_s,
            &mut ws_s
        )
        .is_err());

        // MF-DFA: out len mismatch
        let mut ws_m = MfDfaWorkspace::with_capacity(512);
        let x_m = vec![0.0f64; 512];
        let scales = [32usize, 64usize];
        let q = [0.0, 2.0];
        let mut out_m = vec![0.0f64; 3];
        assert!(
            calculate_mfdfa_into_with_workspace(&x_m, &scales, &q, 1, &mut out_m, &mut ws_m)
                .is_err()
        );

        // WTMM: out len mismatch
        let mut ws_w = WtmmWorkspace::with_capacity(256);
        let x_w = vec![0.0f64; 256];
        let scales_w = [2.0, 4.0];
        let mut out_w = vec![0.0f64; 1];
        assert!(calculate_wtmm_partition_functions_into_with_workspace(
            &x_w, &scales_w, &q, &mut out_w, &mut ws_w
        )
        .is_err());

        // RQA: invalid epsilon should return Err, not panic.
        let mut ws_rqa = RqaWorkspace::default();
        let cfg = RqaConfig {
            embed_dim: 2,
            delay: 1,
            epsilon: 0.0,
            diag_min_len: 2,
            vert_min_len: 2,
            include_diagonal_in_recurrence_rate: false,
            sampling: RqaSampling::DeterministicSubsample { max_templates: 64 },
        };
        assert!(rqa_metrics_with_workspace(&x, &cfg, &mut ws_rqa).is_err());

        // DCCA: reject scale too large / mismatched output without panicking.
        let y_ok = vec![2.0f64; x.len()];
        let scales = [1000usize];
        let mut out_rho = vec![0.0f64; 1];
        let mut ws_dcca = DccaWorkspace::with_capacity(x.len());
        assert!(
            dcca_rho_into_with_workspace(&x, &y_ok, &scales, &mut out_rho, &mut ws_dcca).is_err()
        );

        // MSE: invalid max_scale (cap) should Err, not panic.
        let mut out_mse = vec![0.0f64; 2];
        let mut ws_mse = MseWorkspace::with_capacity(x.len());
        assert!(multiscale_sample_entropy_into_with_workspace(
            &x,
            2,
            1,
            0.5,
            2,
            1,
            &mut out_mse,
            &mut ws_mse
        )
        .is_err());

        // Zero-crossing: length<2 should Err, not panic.
        let x1 = vec![1.0f64];
        assert!(zero_crossing_rate(&x1, ZeroHandling::AsZero).is_err());
        assert!(sign_run_stats(&x1, ZeroHandling::AsZero).is_ok());
    });
    assert!(r.is_ok());
}
