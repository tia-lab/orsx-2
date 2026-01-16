use criterion::{black_box, criterion_group, criterion_main, Criterion};
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
        .unwrap_or_else(|_| "postgresql::postgres:password@localhost/orso_bench".to_string());
    PgPool::connect(&url)
        .await
        .expect("Failed to connect to benchmark database")
}

async fn setup_table_with_data(pool: &PgPool, count: usize) {
    sqlx::query("DROP TABLE IF EXISTS bench_records CASCADE")
        .execute(pool)
        .await
        .expect("Failed to drop table");

    sqlx::query(&BenchRecord::create_table_sql())
        .execute(pool)
        .await
        .expect("Failed to create table");

    // Insert test data
    for i in 0..count {
        let record = BenchRecord {
            id: format!("bench_{}", i),
            name: format!("Record {}", i),
            value: i as i64,
            price: i as f64 * 1.5,
        };
        record
            .insert_into_table(pool, "bench_records")
            .await
            .expect("Failed to insert test data");
    }
}

fn bench_select_all(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let pool = runtime.block_on(setup_bench_db());

    // Setup: Insert 10,000 records
    runtime.block_on(setup_table_with_data(&pool, 10000));

    c.bench_function("select_all_10k_v2", |b| {
        b.to_async(&runtime).iter(|| async {
            let records = BenchRecord::fetch_all_from_table(&pool, "bench_records")
                .await
                .expect("SELECT failed");
            black_box(records);
        });
    });
}

fn bench_count(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let pool = runtime.block_on(setup_bench_db());

    // Setup: Insert 10,000 records
    runtime.block_on(setup_table_with_data(&pool, 10000));

    c.bench_function("count_10k_v2", |b| {
        b.to_async(&runtime).iter(|| async {
            let count = BenchRecord::count_in_table(&pool, "bench_records")
                .await
                .expect("COUNT failed");
            black_box(count);
        });
    });
}

criterion_group!(benches, bench_select_all, bench_count);
criterion_main!(benches);
