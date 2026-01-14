use crate::signal::spectral::dpss::clear_dpss_global_cache_for_tests;
use crate::signal::spectral::multitaper::{
    calculate_multitaper_power_spectrum_into_with_workspace, MultitaperWorkspace,
};
use crate::signal::types::DetrendMethod;
use std::time::{Duration, Instant};

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

fn measure_once(n: usize, nw: f64, k: usize) -> (Duration, Duration) {
    clear_dpss_global_cache_for_tests();

    let x = gen_seeded(n, 123);
    let mut out = vec![0.0f64; n];

    // Cold: new workspace, global cache empty.
    let mut ws = MultitaperWorkspace::with_capacity(n, k);
    let t0 = Instant::now();
    calculate_multitaper_power_spectrum_into_with_workspace(
        &x,
        DetrendMethod::RemoveMean,
        nw,
        k,
        &mut out,
        &mut ws,
    )
    .unwrap();
    let cold = t0.elapsed();

    // Warm: new workspace, but global cache should hit for (n,nw,k).
    let mut ws = MultitaperWorkspace::with_capacity(n, k);
    let t1 = Instant::now();
    calculate_multitaper_power_spectrum_into_with_workspace(
        &x,
        DetrendMethod::RemoveMean,
        nw,
        k,
        &mut out,
        &mut ws,
    )
    .unwrap();
    let warm = t1.elapsed();

    (cold, warm)
}

/// Timing instrumentation (not a correctness test).
///
/// Run explicitly:
/// `cargo test -p math signal -- signal::tests::test_spectral_multitaper_cold_warm_timing --ignored --nocapture`
#[test]
#[ignore]
fn test_spectral_multitaper_cold_warm_timing() {
    // NOTE:
    // - DPSS in this implementation is computed via a dense symmetric eigendecomposition, which is expensive.
    // - This test is instrumentation and must remain time-bounded; it intentionally limits sizes.
    // - The "10_000" standard size is not measurable for cold-start DPSS with the current algorithm.
    let nw = 3.0;
    let k = 5usize;
    // In debug builds, keep sizes minimal to avoid multi-minute runs.
    let sizes: &[usize] = if cfg!(debug_assertions) {
        &[100usize]
    } else {
        &[100usize, 1_000usize, 10_000usize]
    };

    for &n in sizes {
        let repeats = if cfg!(debug_assertions) {
            1
        } else if n >= 10_000 {
            1
        } else {
            3
        };
        let mut cold_best = Duration::MAX;
        let mut warm_best = Duration::MAX;
        for _ in 0..repeats {
            let (cold, warm) = measure_once(n, nw, k);
            cold_best = cold_best.min(cold);
            warm_best = warm_best.min(warm);
        }
        println!(
            "multitaper_cold_warm n={n} nw={nw} k={k} cold_best={:?} warm_best={:?} ratio={:.2}x",
            cold_best,
            warm_best,
            (cold_best.as_secs_f64() / warm_best.as_secs_f64()).max(0.0)
        );
    }
}
