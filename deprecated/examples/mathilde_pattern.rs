use anyhow::Result;
use orsx::migrations::Migrations;
use orsx::prelude::*;
use sqlx::PgPool;

// MATHILDE regime indicator struct
#[derive(OrsxMigrate, sqlx::FromRow, Debug, Clone, serde::Serialize)]
#[orsx_table("regime_trend")]
struct RegimeTrend {
    #[orsx_column(primary_key)]
    id: String,
    pair: String,
    timeframe: String,
    trend_score: f64,
    prices: Compressed<f64>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== orso-postgres V2: MATHILDE Multi-Timeframe Pattern ===\n");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/orso_example".to_string());

    println!("Connecting to database...");
    let pool = PgPool::connect(&database_url).await?;

    // MATHILDE pattern: Create tables for different timeframes
    let timeframes = vec!["1h", "4h", "12h", "1d"];

    println!("Creating multi-timeframe tables...");
    for tf in &timeframes {
        let table_name = format!("regime_trend_{}", tf);

        let dummy = RegimeTrend {
            id: String::new(),
            pair: String::new(),
            timeframe: String::new(),
            trend_score: 0.0,
            prices: Compressed::new(vec![]),
        };

        Migrations::init(&pool, &[(dummy, Some(&table_name))]).await?;
        println!("✓ Created table: {}", table_name);
    }
    println!();

    // Insert data for each timeframe
    println!("Inserting regime data for BTCUSDT...");
    let prices = vec![100.0, 101.5, 102.0, 103.5, 104.0, 105.0];

    for tf in &timeframes {
        let table_name = format!("regime_trend_{}", tf);

        let trend = RegimeTrend {
            id: format!("trend_{}", tf),
            pair: "BTCUSDT".to_string(),
            timeframe: tf.to_string(),
            trend_score: 0.75,
            prices: Compressed::new(prices.clone()),
        };

        trend.insert_into_table(&pool, &table_name).await?;
        println!("✓ Inserted data into: {}", table_name);
    }
    println!();

    // Query data from specific timeframe
    println!("Querying 1h timeframe data...");
    let data_1h = RegimeTrend::fetch_all_from_table(&pool, "regime_trend_1h").await?;
    println!("✓ Found {} records in 1h timeframe", data_1h.len());

    // Compressed data is automatically decompressed
    if let Some(trend) = data_1h.first() {
        println!("✓ Trend score: {}", trend.trend_score);
        println!("✓ Prices (decompressed): {:?}", trend.prices.as_slice());
    }
    println!();

    // Query across all timeframes
    println!("Counting records across all timeframes...");
    for tf in &timeframes {
        let table_name = format!("regime_trend_{}", tf);
        let count = RegimeTrend::count_in_table(&pool, &table_name).await?;
        println!("✓ {} timeframe: {} records", tf, count);
    }
    println!();

    // Update a specific timeframe
    println!("Updating 4h timeframe trend score...");
    let mut trend_4h = RegimeTrend::fetch_all_from_table(&pool, "regime_trend_4h")
        .await?
        .into_iter()
        .next()
        .unwrap();

    trend_4h.trend_score = 0.85;
    trend_4h.update_in_table(&pool, "regime_trend_4h").await?;
    println!("✓ Updated trend score to 0.85\n");

    // Delete from specific timeframe
    println!("Deleting from 1d timeframe...");
    let deleted = RegimeTrend::delete_from_table(&pool, "regime_trend_1d", "trend_1d").await?;
    println!("✓ Deleted {} rows from 1d timeframe\n", deleted);

    println!("=== MATHILDE Pattern Example completed successfully! ===");
    println!("\nKey features demonstrated:");
    println!("- Same struct, multiple tables (one per timeframe)");
    println!("- Compressed price arrays for efficient storage");
    println!("- Full CRUD operations per timeframe");
    println!("- Table isolation (updates to 4h don't affect 1h)");

    Ok(())
}
