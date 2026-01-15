use orsx::prelude::*;
use sqlx::Connection;
use uuid::Uuid;

async fn read_indexes(pool: &sqlx::PgPool, table_name: &str) -> Vec<(String, bool, String, Vec<String>)> {
    sqlx::query_as(
        r#"
        SELECT
          ic.relname AS index_name,
          i.indisunique AS is_unique,
          am.amname AS method,
          array_agg(a.attname ORDER BY k.ord) AS columns
        FROM pg_catalog.pg_index i
        JOIN pg_catalog.pg_class t ON t.oid = i.indrelid
        JOIN pg_catalog.pg_namespace n ON n.oid = t.relnamespace
        JOIN pg_catalog.pg_class ic ON ic.oid = i.indexrelid
        JOIN pg_catalog.pg_am am ON am.oid = ic.relam
        JOIN LATERAL unnest(i.indkey) WITH ORDINALITY AS k(attnum, ord) ON true
        JOIN pg_catalog.pg_attribute a ON a.attrelid = t.oid AND a.attnum = k.attnum
        WHERE n.nspname = 'public'
          AND t.relname = $1
          AND i.indisvalid
          AND i.indisready
        GROUP BY i.indexrelid, ic.relname, i.indisunique, am.amname
        ORDER BY ic.relname
        "#,
    )
    .bind(table_name)
    .fetch_all(pool)
    .await
    .unwrap()
}

fn count_matching(defs: &[(String, bool, String, Vec<String>)], unique: bool, method: &str, cols: &[&str]) -> usize {
    defs.iter()
        .filter(|(_, u, m, c)| {
            *u == unique
                && m == method
                && c.len() == cols.len()
                && c.iter().zip(cols.iter()).all(|(a, b)| a == b)
        })
        .count()
}

#[derive(OrsxMigrate)]
#[orsx_table("idx_uniques_v1")]
struct UniqueV1 {
    #[orsx_column(primary_key)]
    id: String,
    email: String,
}

#[derive(OrsxMigrate)]
#[orsx_table("idx_uniques_v2")]
struct UniqueV2 {
    #[orsx_column(primary_key)]
    id: String,
    #[orsx_column(unique)]
    email: String,
}

#[tokio::test]
async fn adding_unique_is_detected_and_idempotent() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    let table = format!("orsx2_unique_add_{}", Uuid::new_v4().simple());
    let mut conn = sqlx::PgConnection::connect(&url).await.unwrap();
    sqlx::query(&format!(
        "DROP TABLE IF EXISTS {} CASCADE",
        orsx::quote_identifier(&table)
    ))
    .execute(&mut conn)
    .await
    .unwrap();

    let dummy_v1 = UniqueV1 {
        id: "x".into(),
        email: "e".into(),
    };
    Migrations::init(&pool, &[(dummy_v1, Some(&table))])
        .await
        .unwrap();

    // Now migrate to v2 (adds uniqueness on email).
    let dummy_v2 = UniqueV2 {
        id: "x".into(),
        email: "e".into(),
    };
    Migrations::init(&pool, &[(dummy_v2, Some(&table))])
        .await
        .unwrap();

    let defs = read_indexes(&pool, &table).await;
    assert_eq!(count_matching(&defs, true, "btree", &["email"]), 1);

    // Idempotent rerun: still exactly one matching unique index.
    let dummy_v2b = UniqueV2 {
        id: "x".into(),
        email: "e".into(),
    };
    Migrations::init(&pool, &[(dummy_v2b, Some(&table))])
        .await
        .unwrap();
    let defs2 = read_indexes(&pool, &table).await;
    assert_eq!(count_matching(&defs2, true, "btree", &["email"]), 1);
}

#[derive(OrsxMigrate)]
#[orsx_table("idx_composite_unique", index(columns("tenant_id", "email"), unique))]
struct CompositeUnique {
    #[orsx_column(primary_key)]
    id: String,
    tenant_id: String,
    email: String,
}

#[tokio::test]
async fn composite_unique_is_created_on_existing_table_and_idempotent() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    let table = format!("orsx2_comp_uq_{}", Uuid::new_v4().simple());
    let mut conn = sqlx::PgConnection::connect(&url).await.unwrap();
    sqlx::query(&format!(
        "DROP TABLE IF EXISTS {} CASCADE",
        orsx::quote_identifier(&table)
    ))
    .execute(&mut conn)
    .await
    .unwrap();
    sqlx::query(&format!(
        "CREATE TABLE {} (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, email TEXT NOT NULL)",
        orsx::quote_identifier(&table)
    ))
    .execute(&mut conn)
    .await
    .unwrap();

    let dummy = CompositeUnique {
        id: "x".into(),
        tenant_id: "t".into(),
        email: "e".into(),
    };
    Migrations::init(&pool, &[(dummy, Some(&table))]).await.unwrap();

    let defs = read_indexes(&pool, &table).await;
    assert_eq!(
        count_matching(&defs, true, "btree", &["tenant_id", "email"]),
        1
    );

    // Idempotent rerun should not create another equivalent index with a different name.
    let dummy2 = CompositeUnique {
        id: "x".into(),
        tenant_id: "t".into(),
        email: "e".into(),
    };
    Migrations::init(&pool, &[(dummy2, Some(&table))]).await.unwrap();
    let defs2 = read_indexes(&pool, &table).await;
    assert_eq!(
        count_matching(&defs2, true, "btree", &["tenant_id", "email"]),
        1
    );
}

#[tokio::test]
async fn equivalent_existing_index_with_different_name_is_not_duplicated() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    let table = format!("orsx2_comp_uq_existing_{}", Uuid::new_v4().simple());
    let mut conn = sqlx::PgConnection::connect(&url).await.unwrap();
    sqlx::query(&format!(
        "DROP TABLE IF EXISTS {} CASCADE",
        orsx::quote_identifier(&table)
    ))
    .execute(&mut conn)
    .await
    .unwrap();
    sqlx::query(&format!(
        "CREATE TABLE {} (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, email TEXT NOT NULL)",
        orsx::quote_identifier(&table)
    ))
    .execute(&mut conn)
    .await
    .unwrap();
    sqlx::query(&format!(
        "CREATE UNIQUE INDEX {} ON {} (tenant_id, email)",
        orsx::quote_identifier(&format!("custom_uq_{}", Uuid::new_v4().simple())),
        orsx::quote_identifier(&table)
    ))
    .execute(&mut conn)
    .await
    .unwrap();

    let dummy = CompositeUnique {
        id: "x".into(),
        tenant_id: "t".into(),
        email: "e".into(),
    };
    Migrations::init(&pool, &[(dummy, Some(&table))]).await.unwrap();

    let defs = read_indexes(&pool, &table).await;
    assert_eq!(
        count_matching(&defs, true, "btree", &["tenant_id", "email"]),
        1
    );
}

#[derive(OrsxMigrate)]
#[orsx_table("idx_override")]
struct OverrideSpec {
    #[orsx_column(primary_key)]
    id: String,
    #[orsx_column(index)]
    email: String,
}

#[tokio::test]
async fn table_name_override_does_not_collide_on_index_names() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();

    let t1 = format!("orsx2_override_{}_a", Uuid::new_v4().simple());
    let t2 = format!("orsx2_override_{}_b", Uuid::new_v4().simple());
    let mut conn = sqlx::PgConnection::connect(&url).await.unwrap();
    sqlx::query(&format!(
        "DROP TABLE IF EXISTS {} CASCADE",
        orsx::quote_identifier(&t1)
    ))
    .execute(&mut conn)
    .await
    .unwrap();
    sqlx::query(&format!(
        "DROP TABLE IF EXISTS {} CASCADE",
        orsx::quote_identifier(&t2)
    ))
    .execute(&mut conn)
    .await
    .unwrap();

    let dummy = OverrideSpec {
        id: "x".into(),
        email: "e".into(),
    };

    // Apply the same spec to two different tables.
    Migrations::init(&pool, &[(dummy, Some(&t1))]).await.unwrap();
    let dummy2 = OverrideSpec {
        id: "x".into(),
        email: "e".into(),
    };
    Migrations::init(&pool, &[(dummy2, Some(&t2))]).await.unwrap();

    let defs1 = read_indexes(&pool, &t1).await;
    let defs2 = read_indexes(&pool, &t2).await;

    // Each table should have a btree index on email. Index names must be distinct globally.
    assert_eq!(count_matching(&defs1, false, "btree", &["email"]), 1);
    assert_eq!(count_matching(&defs2, false, "btree", &["email"]), 1);

    let names1: Vec<String> = defs1.into_iter().map(|(n, _, _, _)| n).collect();
    let names2: Vec<String> = defs2.into_iter().map(|(n, _, _, _)| n).collect();
    for n1 in &names1 {
        assert!(
            !names2.contains(n1),
            "index name collision across overridden tables: {n1}"
        );
    }
}
