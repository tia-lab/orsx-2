use orsx::prelude::*;
use crate::integration::{setup_test_db, cleanup_all_tables, create_test_table};

#[derive(OrsxMigrate, sqlx::FromRow, Debug, Clone)]
#[orsx_table("regime_trend")]
struct RegimeTrend {
    #[orsx_column(primary_key)]
    id: String,
    pair: String,
    trend_score: f64,
}

#[tokio::test]
async fn test_insert_into_custom_table_name() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;

    // Create table with custom name (MATHILDE pattern: regime_trend_1h)
    create_test_table::<RegimeTrend>(&pool, Some("regime_trend_1h")).await?;

    let record = RegimeTrend {
        id: "trend_1".to_string(),
        pair: "BTCUSDT".to_string(),
        trend_score: 0.75,
    };

    // Insert into custom-named table
    record.insert_into_table(&pool, "regime_trend_1h").await?;

    // Verify
    let retrieved = RegimeTrend::fetch_all_from_table(&pool, "regime_trend_1h").await?;

    assert_eq!(retrieved.len(), 1);
    assert_eq!(retrieved[0].pair, "BTCUSDT");
    assert_eq!(retrieved[0].trend_score, 0.75);

    cleanup_all_tables(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn test_multi_timeframe_pattern() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Test MATHILDE's multi-timeframe table pattern
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;

    // Create tables for different timeframes
    let timeframes = vec!["1h", "4h", "12h", "1d"];

    for tf in &timeframes {
        let table_name = format!("regime_trend_{}", tf);
        create_test_table::<RegimeTrend>(&pool, Some(&table_name)).await?;
    }

    // Insert data into each timeframe
    for (i, tf) in timeframes.iter().enumerate() {
        let record = RegimeTrend {
            id: format!("trend_{}", i),
            pair: "BTCUSDT".to_string(),
            trend_score: i as f64 * 0.1,
        };

        let table_name = format!("regime_trend_{}", tf);
        record.insert_into_table(&pool, &table_name).await?;
    }

    // Verify all timeframes have data
    for tf in &timeframes {
        let table_name = format!("regime_trend_{}", tf);
        let count = RegimeTrend::count_in_table(&pool, &table_name).await?;
        assert_eq!(count, 1, "Timeframe {} should have 1 record", tf);
    }

    // Verify each timeframe has different data
    let data_1h = RegimeTrend::fetch_all_from_table(&pool, "regime_trend_1h").await?;
    let data_4h = RegimeTrend::fetch_all_from_table(&pool, "regime_trend_4h").await?;

    assert_eq!(data_1h[0].trend_score, 0.0);
    assert_eq!(data_4h[0].trend_score, 0.1);

    cleanup_all_tables(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn test_table_with_name_isolation() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Verify that data inserted into one custom table doesn't appear in another
    let pool = setup_test_db().await?;
    cleanup_all_tables(&pool).await?;

    // Create two separate tables
    create_test_table::<RegimeTrend>(&pool, Some("regime_trend_1h")).await?;
    create_test_table::<RegimeTrend>(&pool, Some("regime_trend_4h")).await?;

    // Insert into first table only
    let record = RegimeTrend {
        id: "trend_isolation".to_string(),
        pair: "ETHUSDT".to_string(),
        trend_score: 0.85,
    };

    record.insert_into_table(&pool, "regime_trend_1h").await?;

    // Verify first table has data
    let count_1h = RegimeTrend::count_in_table(&pool, "regime_trend_1h").await?;
    assert_eq!(count_1h, 1);

    // Verify second table is empty
    let count_4h = RegimeTrend::count_in_table(&pool, "regime_trend_4h").await?;
    assert_eq!(count_4h, 0);

    cleanup_all_tables(&pool).await?;
    Ok(())
}
