use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use orsx::prelude::*;
use sqlx::PgPool;
use std::time::Duration;

#[derive(OrsxMigrate, sqlx::FromRow, Clone, serde::Serialize)]
#[orsx_table("bench_batch_records")]
struct BenchBatchRecord {
    #[orsx_column(primary_key)]
    id: String,
    name: String,
    value: i64,
    price: f64,
    description: String,
    active: bool,
    created_at: i64,
}

async fn setup_bench_db() -> PgPool {
    let url = std::env::var("BENCH_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/orsx_bench".to_string());

    let pool = PgPool::connect(&url)
        .await
        .expect("Failed to connect to benchmark database");

    // Drop and recreate table
    sqlx::query("DROP TABLE IF EXISTS bench_batch_records CASCADE")
        .execute(&pool)
        .await
        .expect("Failed to drop table");

    sqlx::query(&BenchBatchRecord::create_table_sql())
        .execute(&pool)
        .await
        .expect("Failed to create table");

    pool
}

fn create_bench_records(count: usize) -> Vec<BenchBatchRecord> {
    (0..count)
        .map(|i| BenchBatchRecord {
            id: uuid::Uuid::new_v4().to_string(),
            name: format!("Bench Record {}", i),
            value: i as i64,
            price: i as f64 * 1.5,
            description: format!(
                "This is a benchmark record with index {} for performance testing",
                i
            ),
            active: i % 2 == 0,
            created_at: chrono::Utc::now().timestamp_millis(),
        })
        .collect()
}

fn bench_insert_individual_vs_batch(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let pool = runtime.block_on(setup_bench_db());

    let mut group = c.benchmark_group("insert_comparison");
    group.measurement_time(Duration::from_secs(10));

    for size in [10, 100, 1000].iter() {
        let records = create_bench_records(*size);

        // Benchmark individual inserts
        group.throughput(Throughput::Elements(*size as u64));
        let pool_clone = pool.clone();
        group.bench_with_input(BenchmarkId::new("individual", size), size, |b, _| {
            b.to_async(&runtime).iter_batched(
                || {
                    runtime.block_on(setup_bench_db());
                    (pool_clone.clone(), records.clone())
                },
                |(pool, records)| async move {
                    for record in records {
                        record
                            .insert_into_table(&pool, "bench_batch_records")
                            .await
                            .expect("Insert failed");
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });

        // Benchmark batch inserts
        let pool_clone = pool.clone();
        group.bench_with_input(BenchmarkId::new("batch", size), size, |b, _| {
            b.to_async(&runtime).iter_batched(
                || {
                    runtime.block_on(setup_bench_db());
                    (pool_clone.clone(), records.clone())
                },
                |(pool, records)| async move {
                    BenchBatchRecord::batch_insert_into_table(
                        &records,
                        &pool,
                        "bench_batch_records",
                    )
                    .await
                    .expect("Batch insert failed");
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_insert_scaling(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let pool = runtime.block_on(setup_bench_db());

    let mut group = c.benchmark_group("batch_insert_scaling");
    group.measurement_time(Duration::from_secs(15));

    for size in [10, 50, 100, 500, 1000, 5000, 10000].iter() {
        let records = create_bench_records(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("batch_insert", size), size, |b, _| {
            b.to_async(&runtime).iter_batched(
                || {
                    runtime.block_on(setup_bench_db());
                    records.clone()
                },
                |records| async move {
                    let start = std::time::Instant::now();
                    BenchBatchRecord::batch_insert_into_table(
                        &records,
                        &pool,
                        "bench_batch_records",
                    )
                    .await
                    .expect("Batch insert failed");
                    black_box(start.elapsed())
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_update_comparison(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let pool = runtime.block_on(setup_bench_db());

    let mut group = c.benchmark_group("update_comparison");
    group.measurement_time(Duration::from_secs(10));

    for size in [10, 100, 500].iter() {
        // Pre-insert records for update
        let mut records = create_bench_records(*size);
        runtime.block_on(async {
            BenchBatchRecord::batch_insert_into_table(&records, &pool, "bench_batch_records")
                .await
                .expect("Initial insert failed");
        });

        // Modify records for update
        for record in &mut records {
            record.value *= 2;
            record.price *= 1.1;
        }

        group.throughput(Throughput::Elements(*size as u64));

        // Individual updates
        group.bench_with_input(BenchmarkId::new("individual", size), size, |b, _| {
            b.to_async(&runtime).iter_batched(
                || records.clone(),
                |records| async move {
                    for record in records {
                        record
                            .update_in_table(&pool, "bench_batch_records")
                            .await
                            .expect("Update failed");
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });

        // Batch updates
        group.bench_with_input(BenchmarkId::new("batch", size), size, |b, _| {
            b.to_async(&runtime).iter_batched(
                || records.clone(),
                |records| async move {
                    BenchBatchRecord::batch_update_in_table(&records, &pool, "bench_batch_records")
                        .await
                        .expect("Batch update failed");
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_delete_comparison(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let pool = runtime.block_on(setup_bench_db());

    let mut group = c.benchmark_group("delete_comparison");
    group.measurement_time(Duration::from_secs(10));

    for size in [10, 100, 500].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        // Individual deletes
        group.bench_with_input(BenchmarkId::new("individual", size), size, |b, _| {
            b.to_async(&runtime).iter_batched(
                || {
                    // Setup: Insert records to delete
                    let records = create_bench_records(*size);
                    runtime.block_on(async {
                        BenchBatchRecord::batch_insert_into_table(
                            &records,
                            &pool,
                            "bench_batch_records",
                        )
                        .await
                        .expect("Setup insert failed");
                    });
                    records.into_iter().map(|r| r.id).collect::<Vec<_>>()
                },
                |ids| async move {
                    for id in ids {
                        BenchBatchRecord::delete_from_table(&pool, "bench_batch_records", &id)
                            .await
                            .expect("Delete failed");
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });

        // Batch deletes
        group.bench_with_input(BenchmarkId::new("batch", size), size, |b, _| {
            b.to_async(&runtime).iter_batched(
                || {
                    // Setup: Insert records to delete
                    let records = create_bench_records(*size);
                    runtime.block_on(async {
                        BenchBatchRecord::batch_insert_into_table(
                            &records,
                            &pool,
                            "bench_batch_records",
                        )
                        .await
                        .expect("Setup insert failed");
                    });
                    records.into_iter().map(|r| r.id).collect::<Vec<_>>()
                },
                |ids| async move {
                    BenchBatchRecord::batch_delete_from_table(&pool, "bench_batch_records", &ids)
                        .await
                        .expect("Batch delete failed");
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

// Memory usage benchmark
fn bench_memory_usage(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let pool = runtime.block_on(setup_bench_db());

    let mut group = c.benchmark_group("memory_usage");
    group.measurement_time(Duration::from_secs(5));

    for size in [100, 1000, 10000].iter() {
        let records = create_bench_records(*size);

        group.bench_with_input(
            BenchmarkId::new("batch_insert_memory", size),
            size,
            |b, _| {
                b.to_async(&runtime).iter_batched(
                    || {
                        runtime.block_on(setup_bench_db());
                        records.clone()
                    },
                    |records| async move {
                        // Track memory before
                        let before = get_memory_usage();

                        BenchBatchRecord::batch_insert_into_table(
                            &records,
                            &pool,
                            "bench_batch_records",
                        )
                        .await
                        .expect("Batch insert failed");

                        // Track memory after
                        let after = get_memory_usage();
                        black_box(after - before)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn get_memory_usage() -> usize {
    // Simplified memory tracking - in production use a proper memory profiler
    // This is a placeholder that would need platform-specific implementation
    0
}

// MATHILDE-specific benchmark (600+ fields simulation)
fn bench_mathilde_pattern(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let pool = runtime.block_on(setup_bench_db());

    let mut group = c.benchmark_group("mathilde_pattern");
    group.measurement_time(Duration::from_secs(20));

    // Simulate MATHILDE's pattern: multiple timeframes with same data structure
    let timeframes = vec!["1h", "4h", "12h", "1d"];
    let record_counts = vec![650]; // MATHILDE has 600+ fields per record

    for count in record_counts {
        let records = create_bench_records(count);

        group.throughput(Throughput::Elements((count * timeframes.len()) as u64));
        group.bench_function(BenchmarkId::new("multi_timeframe_insert", count), |b| {
            b.to_async(&runtime).iter_batched(
                || {
                    runtime.block_on(setup_bench_db());
                    records.clone()
                },
                |records| async move {
                    let start = std::time::Instant::now();

                    // Insert into multiple tables (one per timeframe)
                    for _tf in &timeframes {
                        BenchBatchRecord::batch_insert_into_table(
                            &records,
                            &pool,
                            "bench_batch_records",
                        )
                        .await
                        .expect("Batch insert failed");

                        // Clear table for next iteration
                        sqlx::query("TRUNCATE TABLE bench_batch_records")
                            .execute(&pool)
                            .await
                            .expect("Truncate failed");
                    }

                    black_box(start.elapsed())
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_insert_individual_vs_batch,
    bench_insert_scaling,
    bench_update_comparison,
    bench_delete_comparison,
    bench_memory_usage,
    bench_mathilde_pattern
);
criterion_main!(benches);
