use orsx::flatten::PgArgumentsVisitor;
use orsx::prelude::*;
use sqlx::Executor;

#[orsx::orsx_flatten_module]
mod outputs {
    #[derive(Clone)]
    pub struct Fam {
        pub a: f64,
    }

    #[orsx_table("orsx_flatten_db_integration")]
    #[orsx_processor_id("proc_a")]
    #[derive(Clone)]
    pub struct Out {
        #[orsx_column(primary_key)]
        pub id: String,
        pub pair: String,
        pub diag: Option<orsx::sqlx::types::JsonValue>,

        #[orsx_family(prefix = "ma_")]
        pub fam: Fam,
    }
}

#[tokio::test]
async fn flatten_migrations_and_insert_smoke() {
    let Ok(url) = std::env::var("ORSX_TEST_DATABASE_URL") else {
        return;
    };

    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    pool.execute("DROP TABLE IF EXISTS orsx_flatten_db_integration CASCADE")
        .await
        .unwrap();

    let row = outputs::Out {
        id: "id0".to_string(),
        pair: "BTCUSDT".to_string(),
        diag: Some(serde_json::json!({"k":"v"})),
        fam: outputs::Fam { a: 1.25 },
    };

    Migrations::init(&pool, &[(row.clone(), None)]).await.unwrap();

    // Verify JSONB column exists (mapping correctness).
    let data_type: (String,) = sqlx::query_as(
        "SELECT data_type FROM information_schema.columns WHERE table_name=$1 AND column_name=$2",
    )
    .bind("orsx_flatten_db_integration")
    .bind("diag")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(data_type.0, "jsonb");

    // Insert using the generated visitor binder order + PgArguments adapter.
    let cols = outputs::Out::COLUMNS_IN_ORDER;
    let mut cols_sql = String::new();
    let mut vals_sql = String::new();
    for (i, col) in cols.iter().enumerate() {
        if i > 0 {
            cols_sql.push_str(", ");
            vals_sql.push_str(", ");
        }
        cols_sql.push_str(&quote_identifier(col));
        vals_sql.push('$');
        vals_sql.push_str(&(i + 1).to_string());
    }
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        quote_identifier("orsx_flatten_db_integration"),
        cols_sql,
        vals_sql
    );

    let mut args_visitor = PgArgumentsVisitor::new();
    row.visit_values_in_order(&mut args_visitor).unwrap();
    let args = args_visitor.into_arguments();

    sqlx::query_with(&sql, args).execute(&pool).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*)::BIGINT FROM orsx_flatten_db_integration")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1);
}
