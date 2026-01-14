use anyhow::Result;
use orsx::migrations::Migrations;
use orsx::prelude::*;
use sqlx::PgPool;

#[derive(OrsxMigrate, sqlx::FromRow, Debug, Clone, serde::Serialize)]
#[orsx_table("market_data")]
struct MarketData {
    #[orsx_column(primary_key)]
    id: String,
    symbol: String,
    prices: Compressed<f64>,
    volumes: Compressed<i64>,
    trades: Compressed<i32>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== orso-postgres V2: Compression Example ===\n");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/orso_example".to_string());

    println!("Connecting to database...");
    let pool = PgPool::connect(&database_url).await?;

    // Create table
    println!("Creating market_data table...");
    let dummy = MarketData {
        id: String::new(),
        symbol: String::new(),
        prices: Compressed::new(vec![]),
        volumes: Compressed::new(vec![]),
        trades: Compressed::new(vec![]),
    };

    Migrations::init(&pool, &[(dummy, None)]).await?;
    println!("✓ Table created\n");

    // Generate large dataset
    println!("Generating large dataset...");
    let prices: Vec<f64> = (0..10000).map(|i| 100.0 + (i as f64 * 0.01)).collect();
    let volumes: Vec<i64> = (0..10000).map(|i| 1000000 + i * 1000).collect();
    let trades: Vec<i32> = (0..10000).map(|i| 500 + (i % 100)).collect();

    println!("✓ Generated 10,000 data points per field\n");

    // Calculate uncompressed size
    let uncompressed_size = (prices.len() * 8) + (volumes.len() * 8) + (trades.len() * 4);
    println!(
        "Uncompressed size: {} bytes ({:.2} KB)",
        uncompressed_size,
        uncompressed_size as f64 / 1024.0
    );

    // Insert with compression
    println!("Inserting with compression...");
    let data = MarketData {
        id: "btcusdt_data".to_string(),
        symbol: "BTCUSDT".to_string(),
        prices: Compressed::new(prices.clone()),
        volumes: Compressed::new(volumes.clone()),
        trades: Compressed::new(trades.clone()),
    };

    data.insert_into_table(&pool, MarketData::table_name())
        .await?;
    println!("✓ Data inserted\n");

    // Retrieve and decompress
    println!("Retrieving and decompressing...");
    let retrieved = MarketData::fetch_all_from_table(&pool, MarketData::table_name()).await?;

    if let Some(market_data) = retrieved.first() {
        println!("✓ Data retrieved successfully");
        println!("✓ Symbol: {}", market_data.symbol);
        println!("✓ Prices count: {}", market_data.prices.as_slice().len());
        println!("✓ Volumes count: {}", market_data.volumes.as_slice().len());
        println!("✓ Trades count: {}", market_data.trades.as_slice().len());

        // Verify data integrity
        let price_matches = market_data.prices.as_slice() == &prices[..];
        let volume_matches = market_data.volumes.as_slice() == &volumes[..];
        let trade_matches = market_data.trades.as_slice() == &trades[..];

        println!("\nData integrity check:");
        println!("✓ Prices match: {}", price_matches);
        println!("✓ Volumes match: {}", volume_matches);
        println!("✓ Trades match: {}", trade_matches);
    }
    println!();

    // Query compressed size from database
    println!("Measuring compression ratio...");
    let row: (i64, i64, i64) = sqlx::query_as(
        "SELECT pg_column_size(prices), pg_column_size(volumes), pg_column_size(trades) FROM market_data WHERE id = $1"
    )
    .bind("btcusdt_data")
    .fetch_one(&pool)
    .await?;

    let compressed_size = row.0 + row.1 + row.2;
    let compression_ratio = (1.0 - (compressed_size as f64 / uncompressed_size as f64)) * 100.0;

    println!(
        "Compressed size: {} bytes ({:.2} KB)",
        compressed_size,
        compressed_size as f64 / 1024.0
    );
    println!("Compression ratio: {:.1}%", compression_ratio);
    println!(
        "Space saved: {} bytes ({:.2} KB)",
        uncompressed_size as i64 - compressed_size,
        (uncompressed_size as i64 - compressed_size) as f64 / 1024.0
    );
    println!();

    println!("=== Compression Example completed successfully! ===");
    println!("\nKey features:");
    println!("- Automatic compression on INSERT");
    println!("- Automatic decompression on SELECT");
    println!("- Zero data loss");
    println!("- ~{:.0}% storage savings", compression_ratio);

    Ok(())
}
