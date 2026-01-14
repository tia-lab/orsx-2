use orsx::prelude::*;

#[derive(orsx::OrsxMigrate, sqlx::FromRow)]
#[orsx_table("orsx2_smoke_compressed")]
struct Row {
    #[orsx_column(primary_key)]
    id: String,
    values: Compressed<f64>,
}

#[tokio::test]
async fn compression_db_roundtrip() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    sqlx::query("DROP TABLE IF EXISTS orsx2_smoke_compressed CASCADE")
        .execute(&pool)
        .await
        .unwrap();

    let dummy = Row {
        id: "x".to_string(),
        values: Compressed::new(vec![]),
    };
    Migrations::init(&pool, &[(dummy, None)]).await.unwrap();

    let vals: Vec<f64> = (0..10_000).map(|i| (i as f64) * 0.25).collect();
    let row = Row {
        id: "row1".to_string(),
        values: Compressed::new(vals.clone()),
    };

    sqlx::query("INSERT INTO orsx2_smoke_compressed (id, values) VALUES ($1,$2)")
        .bind(&row.id)
        .bind(&row.values)
        .execute(&pool)
        .await
        .unwrap();

    let got: Row = sqlx::query_as("SELECT id, values FROM orsx2_smoke_compressed WHERE id=$1")
        .bind("row1")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(got.values.as_slice(), &vals[..]);
}

