use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use orsx::prelude::*;
use sqlx::PgPool;

#[derive(OrsxMigrate, sqlx::FromRow, Clone, serde::Serialize)]
#[orsx_table("bench_records")]
struct BenchRecord {
    #[orsx_column(primary_key)]
    id: String,
    name: String,
    value: i64,
    price: f64,
}

async fn setup_bench_db() -> PgPool {
    let url = std::env::var("BENCH_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/orso_bench".to_string());
    PgPool::connect(&url)
        .await
        .expect("Failed to connect to benchmark database")
}

async fn setup_table(pool: &PgPool) {
    sqlx::query("DROP TABLE IF EXISTS bench_records CASCADE")
        .execute(pool)
        .await
        .expect("Failed to drop table");

    sqlx::query(&BenchRecord::create_table_sql())
        .execute(pool)
        .await
        .expect("Failed to create table");
}

fn bench_insert_single(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let pool = runtime.block_on(setup_bench_db());
    runtime.block_on(setup_table(&pool));

    c.bench_function("insert_single_v2", |b| {
        b.to_async(&runtime).iter(|| async {
            let record = BenchRecord {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Test Record".to_string(),
                value: 12345,
                price: 99.99,
            };

            black_box(
                record
                    .insert_into_table(&pool, "bench_records")
                    .await
                    .expect("Insert failed"),
            );
        });
    });
}

fn bench_insert_batch(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let pool = runtime.block_on(setup_bench_db());

    let mut group = c.benchmark_group("insert_batch");

    for size in [10, 100, 1000].iter() {
        runtime.block_on(setup_table(&pool));

        group.bench_with_input(BenchmarkId::new("v2", size), size, |b, &size| {
            b.to_async(&runtime).iter(|| async {
                for i in 0..size {
                    let record = BenchRecord {
                        id: format!("bench_{}", i),
                        name: format!("Record {}", i),
                        value: i,
                        price: i as f64 * 1.5,
                    };

                    record
                        .insert_into_table(&pool, "bench_records")
                        .await
                        .expect("Insert failed");
                }
            });
        });

        // Clean up after each size
        runtime.block_on(setup_table(&pool));
    }

    group.finish();
}

criterion_group!(benches, bench_insert_single, bench_insert_batch);
criterion_main!(benches);
