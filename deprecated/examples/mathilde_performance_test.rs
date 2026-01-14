/// Performance test against real MATHILDE database
/// Tests fetching 10k rows from indicators_regime_1h table
use sqlx::PgPool;
use std::time::Instant;

// Minimal struct matching key fields from indicators_regime_1h
// Only includes essential fields to minimize parsing overhead during test
#[derive(sqlx::FromRow, Debug, Clone)]
#[allow(dead_code)]
struct RegimeIndicator {
    id: String,
    pair: String,
    timestamp: String,
    timeframe: String,
    current_close: f64,
    current_volume: f64,

    // Trend indicators
    sma_20: Option<f64>,
    ema_9: Option<f64>,
    ema_21: Option<f64>,
    ema_50: Option<f64>,
    ema_200: Option<f64>,

    // ADX indicators
    adx_14: Option<f64>,
    plus_di: Option<f64>,
    minus_di: Option<f64>,

    // Trend phase
    trend_phase: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== orso-postgres V2 Performance Test ===");
    println!("Testing against MATHILDE production database\n");

    // Connect to MATHILDE database
    let database_url = "postgresql://postgres:mathilde_dev_pass@localhost:1354/mathilde";
    println!(
        "Connecting to: {}",
        database_url.split('@').last().unwrap_or("database")
    );

    let pool = PgPool::connect(database_url).await?;
    println!("✅ Connected successfully\n");

    // Count total rows
    println!("📊 Counting total rows in indicators_regime_1h...");
    let start = Instant::now();
    let total_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM indicators_regime_1h")
        .fetch_one(&pool)
        .await?;
    let count_time = start.elapsed();
    println!("   Total rows: {}", total_count.0);
    println!("   Count time: {:?}\n", count_time);

    // Test 1: Fetch 100 rows (warm-up)
    println!("🔥 Test 1: Warm-up - Fetching 100 rows...");
    let start = Instant::now();
    let rows: Vec<RegimeIndicator> = sqlx::query_as(
        "SELECT id, pair, timestamp, timeframe, current_close, current_volume,
                sma_20, ema_9, ema_21, ema_50, ema_200,
                adx_14, plus_di, minus_di, trend_phase
         FROM indicators_regime_1h
         LIMIT 100",
    )
    .fetch_all(&pool)
    .await?;
    let warm_up_time = start.elapsed();
    println!("   Fetched {} rows", rows.len());
    println!("   Time: {:?}\n", warm_up_time);

    // Test 2: Fetch 1,000 rows
    println!("⚡ Test 2: Fetching 1,000 rows...");
    let start = Instant::now();
    let rows: Vec<RegimeIndicator> = sqlx::query_as(
        "SELECT id, pair, timestamp, timeframe, current_close, current_volume,
                sma_20, ema_9, ema_21, ema_50, ema_200,
                adx_14, plus_di, minus_di, trend_phase
         FROM indicators_regime_1h
         LIMIT 1000",
    )
    .fetch_all(&pool)
    .await?;
    let fetch_1k_time = start.elapsed();
    println!("   Fetched {} rows", rows.len());
    println!("   Time: {:?}", fetch_1k_time);
    println!("   Per-row: {:?}\n", fetch_1k_time / 1000);

    // Test 3: Fetch 10,000 rows (MAIN TEST)
    println!("🚀 Test 3: Fetching 10,000 rows (MAIN TEST)...");
    let start = Instant::now();
    let rows: Vec<RegimeIndicator> = sqlx::query_as(
        "SELECT id, pair, timestamp, timeframe, current_close, current_volume,
                sma_20, ema_9, ema_21, ema_50, ema_200,
                adx_14, plus_di, minus_di, trend_phase
         FROM indicators_regime_1h
         LIMIT 10000",
    )
    .fetch_all(&pool)
    .await?;
    let fetch_10k_time = start.elapsed();
    println!("   Fetched {} rows", rows.len());
    println!("   Time: {:?}", fetch_10k_time);
    println!("   Per-row: {:?}", fetch_10k_time / 10000);
    println!(
        "   Throughput: {:.0} rows/sec\n",
        10000.0 / fetch_10k_time.as_secs_f64()
    );

    // Test 4: Fetch 10k rows with ALL columns (realistic workload)
    println!("💪 Test 4: Fetching 10,000 rows with ALL columns...");
    let start = Instant::now();
    let rows: Vec<RegimeIndicator> = sqlx::query_as(
        "SELECT id, pair, timestamp, timeframe, current_close, current_volume,
                sma_20, ema_9, ema_21, ema_50, ema_200,
                adx_14, plus_di, minus_di, trend_phase
         FROM indicators_regime_1h
         ORDER BY timestamp DESC
         LIMIT 10000",
    )
    .fetch_all(&pool)
    .await?;
    let fetch_10k_sorted_time = start.elapsed();
    println!("   Fetched {} rows (sorted by timestamp)", rows.len());
    println!("   Time: {:?}", fetch_10k_sorted_time);
    println!("   Per-row: {:?}", fetch_10k_sorted_time / 10000);
    println!(
        "   Throughput: {:.0} rows/sec\n",
        10000.0 / fetch_10k_sorted_time.as_secs_f64()
    );

    // Test 5: Filtered query (typical MATHILDE use case)
    println!("🎯 Test 5: Filtered query - Fetch 10k rows for specific pair...");
    let start = Instant::now();
    let rows: Vec<RegimeIndicator> = sqlx::query_as(
        "SELECT id, pair, timestamp, timeframe, current_close, current_volume,
                sma_20, ema_9, ema_21, ema_50, ema_200,
                adx_14, plus_di, minus_di, trend_phase
         FROM indicators_regime_1h
         WHERE pair = 'BTCUSDT'
         ORDER BY timestamp DESC
         LIMIT 10000",
    )
    .fetch_all(&pool)
    .await?;
    let fetch_filtered_time = start.elapsed();
    println!("   Fetched {} rows for pair=BTCUSDT", rows.len());
    println!("   Time: {:?}", fetch_filtered_time);
    if !rows.is_empty() {
        println!("   Per-row: {:?}", fetch_filtered_time / rows.len() as u32);
        println!(
            "   Throughput: {:.0} rows/sec\n",
            rows.len() as f64 / fetch_filtered_time.as_secs_f64()
        );
    }

    // Summary
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║                    PERFORMANCE SUMMARY                   ║");
    println!("╠═══════════════════════════════════════════════════════════╣");
    println!(
        "║ COUNT query:             {:>8?}                      ║",
        count_time
    );
    println!(
        "║ Warm-up (100 rows):      {:>8?}                      ║",
        warm_up_time
    );
    println!(
        "║ 1,000 rows:              {:>8?}                      ║",
        fetch_1k_time
    );
    println!(
        "║ 10,000 rows:             {:>8?}                      ║",
        fetch_10k_time
    );
    println!(
        "║ 10,000 rows (sorted):    {:>8?}                      ║",
        fetch_10k_sorted_time
    );
    println!(
        "║ Filtered query:          {:>8?}                      ║",
        fetch_filtered_time
    );
    println!("╠═══════════════════════════════════════════════════════════╣");
    println!(
        "║ Per-row latency (10k):   {:>8?}                      ║",
        fetch_10k_time / 10000
    );
    println!(
        "║ Throughput (10k):        {:>8.0} rows/sec              ║",
        10000.0 / fetch_10k_time.as_secs_f64()
    );
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    // Sample data
    if !rows.is_empty() {
        println!("📋 Sample data (first row):");
        let sample = &rows[0];
        println!("   Pair: {}", sample.pair);
        println!("   Timestamp: {}", sample.timestamp);
        println!("   Close: {:.2}", sample.current_close);
        println!("   Volume: {:.2}", sample.current_volume);
        println!("   EMA 21: {:?}", sample.ema_21);
        println!("   ADX 14: {:?}", sample.adx_14);
        println!("   Trend Phase: {:?}\n", sample.trend_phase);
    }

    println!("✅ Performance test completed successfully!");

    Ok(())
}
