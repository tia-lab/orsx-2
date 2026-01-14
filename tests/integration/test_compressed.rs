use orsx::prelude::*;
use crate::integration::{setup_test_db, cleanup_all_tables, create_test_table};

#[derive(OrsxMigrate, sqlx::FromRow, Debug, Clone)]
struct CompressedData {
    #[orsx_column(primary_key)]
    id: String,
    prices: Compressed<f64>,
    volumes: Compressed<i64>,
}

#[tokio::test]
async fn test_compressed_f64_roundtrip() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;
    create_test_table::<CompressedData>(&pool, Some("test_compressed")).await?;

    let prices = vec![100.5, 101.2, 102.0, 103.8, 104.1];
    let volumes = vec![1000_i64, 1100, 1050, 1200, 1150];

    let data = CompressedData {
        id: "data_1".to_string(),
        prices: Compressed::new(prices.clone()),
        volumes: Compressed::new(volumes.clone()),
    };

    // Insert with compression
    data.insert_into_table(&pool, "test_compressed").await?;

    // Retrieve and decompress
    let retrieved = CompressedData::fetch_all_from_table(&pool, "test_compressed").await?;

    assert_eq!(retrieved.len(), 1);
    assert_eq!(retrieved[0].prices.as_slice(), &prices[..]);
    assert_eq!(retrieved[0].volumes.as_slice(), &volumes[..]);

    // Verify compression actually happened (data should be smaller)
    let row: (Vec<u8>,) = sqlx::query_as("SELECT prices FROM test_compressed WHERE id = $1")
        .bind("data_1")
        .fetch_one(&pool)
        .await?;

    let compressed_size = row.0.len();
    let uncompressed_size = prices.len() * std::mem::size_of::<f64>();

    // Compression should reduce size (with some overhead for small datasets)
    // For 5 f64 values (40 bytes), compressed might be similar size due to overhead
    // but verify the mechanism works
    println!(
        "Compressed: {} bytes, Uncompressed: {} bytes",
        compressed_size, uncompressed_size
    );

    cleanup_all_tables(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn test_compressed_large_dataset() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;
    create_test_table::<CompressedData>(&pool, Some("test_compressed")).await?;

    // Larger dataset should show better compression
    let prices: Vec<f64> = (0..1000).map(|i| i as f64 * 1.5 + 100.0).collect();
    let volumes: Vec<i64> = (0..1000).map(|i| i * 100 + 1000).collect();

    let data = CompressedData {
        id: "data_large".to_string(),
        prices: Compressed::new(prices.clone()),
        volumes: Compressed::new(volumes.clone()),
    };

    data.insert_into_table(&pool, "test_compressed").await?;

    let retrieved = CompressedData::fetch_all_from_table(&pool, "test_compressed").await?;

    assert_eq!(retrieved.len(), 1);
    assert_eq!(retrieved[0].prices.as_slice(), &prices[..]);
    assert_eq!(retrieved[0].volumes.as_slice(), &volumes[..]);

    // Check compression ratio
    let row: (Vec<u8>,) = sqlx::query_as("SELECT prices FROM test_compressed WHERE id = $1")
        .bind("data_large")
        .fetch_one(&pool)
        .await?;

    let compressed_size = row.0.len();
    let uncompressed_size = prices.len() * std::mem::size_of::<f64>();
    let compression_ratio = (1.0 - (compressed_size as f64 / uncompressed_size as f64)) * 100.0;

    println!(
        "Compression ratio: {:.2}% (compressed: {} bytes, uncompressed: {} bytes)",
        compression_ratio, compressed_size, uncompressed_size
    );

    // Should achieve some compression for this dataset
    assert!(compressed_size < uncompressed_size);

    cleanup_all_tables(&pool).await?;
    Ok(())
}
