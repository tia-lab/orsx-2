use sqlx::PgPool;
use std::sync::Once;

static INIT: Once = Once::new();

// Setup test database connection
pub async fn setup_test_db() -> Result<PgPool, Box<dyn std::error::Error>> {
    // Initialize logging once
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    });

    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/orso_v2_test".to_string());

    let pool = PgPool::connect(&database_url).await?;
    Ok(pool)
}

// Cleanup all test tables
pub async fn cleanup_all_tables(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let tables = vec![
        "test_users",
        "test_posts",
        "test_regime_data",
        "test_records",
        "test_nullable",
        "test_compressed",
        "test_custom_table_1h",
        "test_custom_table_4h",
        "test_indexed",
        "regime_trend_1h",
        "regime_trend_4h",
        "regime_trend_12h",
        "regime_trend_1d",
    ];

    for table in tables {
        let _ = sqlx::query(&format!("DROP TABLE IF EXISTS \"{}\" CASCADE", table))
            .execute(pool)
            .await;
    }

    Ok(())
}

// Create test table from OrsxMigrate
pub async fn create_test_table<T: orsx::OrsxMigrate>(
    pool: &PgPool,
    table_name: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let name = table_name.unwrap_or(T::table_name());
    let sql = T::create_table_sql().replace(T::table_name(), name);
    sqlx::query(&sql).execute(pool).await?;
    Ok(())
}
