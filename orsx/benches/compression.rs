use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use orsx::{Compressed, CompressedWorkspace};

fn make_f64(n: usize) -> Vec<f64> {
    (0..n).map(|i| 100.0 + (i as f64) * 0.01).collect()
}

fn bench_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");

    for n in [100usize, 1000, 10_000] {
        group.bench_function(format!("encode_f64_n{n}"), |b| {
            b.iter_batched(
                || {
                    let v = make_f64(n);
                    let c = Compressed::new(v);
                    let ws = CompressedWorkspace::with_capacity(n * 8);
                    (c, ws, Vec::<u8>::new())
                },
                |(c, mut ws, mut out)| {
                    c.encode_envelope_into(&mut out, &mut ws).unwrap();
                },
                BatchSize::SmallInput,
            );
        });

        group.bench_function(format!("decode_f64_n{n}"), |b| {
            b.iter_batched(
                || {
                    let v = make_f64(n);
                    let c = Compressed::new(v);
                    let mut ws = CompressedWorkspace::with_capacity(n * 8);
                    let mut bytes = Vec::new();
                    c.encode_envelope_into(&mut bytes, &mut ws).unwrap();
                    bytes
                },
                |bytes| {
                    let decoded = Compressed::<f64>::decode_envelope(&bytes).unwrap();
                    std::hint::black_box(decoded);
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_compression);
criterion_main!(benches);
