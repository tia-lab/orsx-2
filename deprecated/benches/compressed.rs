use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use orsx::prelude::*;
use sqlx::PgPool;

#[derive(OrsxMigrate, sqlx::FromRow, Clone, serde::Serialize)]
#[orsx_table("compressed_bench")]
struct CompressedBench {
    #[orsx_column(primary_key)]
    id: String,
    prices: Compressed<f64>,
    volumes: Compressed<i64>,
}

async fn setup_bench_db() -> PgPool {
    let url = std::env::var("BENCH_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/orso_bench".to_string());
    PgPool::connect(&url)
        .await
        .expect("Failed to connect to benchmark database")
}

async fn setup_compressed_table(pool: &PgPool) {
    sqlx::query("DROP TABLE IF EXISTS compressed_bench CASCADE")
        .execute(pool)
        .await
        .expect("Failed to drop table");

    sqlx::query(&CompressedBench::create_table_sql())
        .execute(pool)
        .await
        .expect("Failed to create table");
}

fn bench_compressed_insert(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let pool = runtime.block_on(setup_bench_db());

    let mut group = c.benchmark_group("compressed_insert");

    // Test with different data sizes
    for size in [100, 1000, 10000].iter() {
        runtime.block_on(setup_compressed_table(&pool));

        group.bench_with_input(BenchmarkId::new("v2", size), size, |b, &size| {
            b.to_async(&runtime).iter(|| async {
                let prices: Vec<f64> = (0..size).map(|i| i as f64 * 1.5).collect();
                let volumes: Vec<i64> = (0..size).map(|i| i * 100).collect();

                let record = CompressedBench {
                    id: uuid::Uuid::new_v4().to_string(),
                    prices: Compressed::new(prices),
                    volumes: Compressed::new(volumes),
                };

                black_box(
                    record
                        .insert_into_table(&pool, "compressed_bench")
                        .await
                        .expect("Insert failed"),
                );
            });
        });

        runtime.block_on(setup_compressed_table(&pool));
    }

    group.finish();
}

fn bench_compressed_roundtrip(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let pool = runtime.block_on(setup_bench_db());
    runtime.block_on(setup_compressed_table(&pool));

    c.bench_function("compressed_roundtrip_1000_v2", |b| {
        b.to_async(&runtime).iter(|| async {
            let prices: Vec<f64> = (0..1000).map(|i| i as f64 * 1.5).collect();
            let volumes: Vec<i64> = (0..1000).map(|i| i * 100).collect();

            let record = CompressedBench {
                id: "roundtrip_test".to_string(),
                prices: Compressed::new(prices.clone()),
                volumes: Compressed::new(volumes.clone()),
            };

            // INSERT (compression happens here)
            record
                .insert_into_table(&pool, "compressed_bench")
                .await
                .expect("Insert failed");

            // SELECT (decompression happens here)
            let retrieved = CompressedBench::fetch_all_from_table(&pool, "compressed_bench")
                .await
                .expect("SELECT failed");

            black_box(retrieved);

            // Cleanup
            sqlx::query("DELETE FROM compressed_bench WHERE id = 'roundtrip_test'")
                .execute(&pool)
                .await
                .expect("DELETE failed");
        });
    });
}

criterion_group!(benches, bench_compressed_insert, bench_compressed_roundtrip);
criterion_main!(benches);
