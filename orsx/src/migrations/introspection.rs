use crate::Result;
use sqlx::PgPool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnInfo {
    pub name: String,
    pub sql_type: String,
    pub nullable: bool,
    pub position: i32,
    pub is_primary_key: bool,
    pub is_unique: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSchema {
    pub table_name: String,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexIdentity {
    pub method: String,
    pub unique: bool,
    pub columns: Vec<String>,
}

pub async fn table_exists(pool: &PgPool, table_name: &str) -> Result<bool> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
          SELECT 1
          FROM pg_catalog.pg_class c
          JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
          WHERE n.nspname = 'public'
            AND c.relkind IN ('r','p')
            AND c.relname = $1
        )
        "#,
    )
    .bind(table_name)
    .fetch_one(pool)
    .await?;

    Ok(exists)
}

pub async fn read_table_index_identities(pool: &PgPool, table_name: &str) -> Result<Vec<IndexIdentity>> {
    // Scope: public schema only (v1).
    //
    // We consider only "ready + valid" indexes to avoid treating in-progress concurrent builds
    // as satisfying the schema.
    let rows = sqlx::query_as::<_, (bool, String, Vec<String>)>(
        r#"
        SELECT
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
        GROUP BY i.indexrelid, i.indisunique, am.amname
        "#,
    )
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(unique, method, columns)| IndexIdentity {
            unique,
            method,
            columns,
        })
        .collect())
}

pub async fn read_table_schema(pool: &PgPool, table_name: &str) -> Result<TableSchema> {
    // Read columns + types + nullability + position.
    let cols = sqlx::query_as::<_, (String, String, bool, i32)>(
        r#"
        SELECT
          a.attname AS column_name,
          pg_catalog.format_type(a.atttypid, a.atttypmod) AS sql_type,
          NOT a.attnotnull AS is_nullable,
          a.attnum::int4 AS position
        FROM pg_catalog.pg_attribute a
        JOIN pg_catalog.pg_class c ON c.oid = a.attrelid
        JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'public'
          AND c.relname = $1
          AND a.attnum > 0
          AND NOT a.attisdropped
        ORDER BY a.attnum
        "#,
    )
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    let mut columns: Vec<ColumnInfo> = cols
        .into_iter()
        .map(|(name, sql_type, nullable, pos)| ColumnInfo {
            name,
            sql_type: sql_type.to_uppercase(),
            nullable,
            position: pos - 1,
            is_primary_key: false,
            is_unique: false,
        })
        .collect();

    // PK/unique constraints (column-level, single-column only for now).
    let constraints = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT
          a.attname AS column_name,
          ct.contype::text AS contype
        FROM pg_catalog.pg_constraint ct
        JOIN pg_catalog.pg_class c ON c.oid = ct.conrelid
        JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
        JOIN pg_catalog.pg_attribute a ON a.attrelid = c.oid
        WHERE n.nspname = 'public'
          AND c.relname = $1
          AND ct.contype IN ('p','u')
          AND array_length(ct.conkey, 1) = 1
          AND ct.conkey[1] = a.attnum
        "#,
    )
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    for (col, typ) in constraints {
        if let Some(c) = columns.iter_mut().find(|x| x.name == col) {
            match typ.as_str() {
                "p" => {
                    c.is_primary_key = true;
                    c.is_unique = true;
                    c.nullable = false;
                }
                "u" => c.is_unique = true,
                _ => {}
            }
        }
    }

    // Mark single-column unique indexes as unique semantics too (even if not declared as constraints).
    // This keeps planning aligned with real Postgres uniqueness guarantees and avoids requiring
    // `ALTER TABLE ... ADD CONSTRAINT ... USING INDEX` during online-safe operations.
    let unique_index_cols = sqlx::query_as::<_, (String,)>(
        r#"
        SELECT a.attname AS column_name
        FROM pg_catalog.pg_index i
        JOIN pg_catalog.pg_class t ON t.oid = i.indrelid
        JOIN pg_catalog.pg_namespace n ON n.oid = t.relnamespace
        JOIN LATERAL unnest(i.indkey) WITH ORDINALITY AS k(attnum, ord) ON true
        JOIN pg_catalog.pg_attribute a ON a.attrelid = t.oid AND a.attnum = k.attnum
        WHERE n.nspname = 'public'
          AND t.relname = $1
          AND i.indisunique
          AND array_length(i.indkey, 1) = 1
        "#,
    )
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    for (col,) in unique_index_cols {
        if let Some(c) = columns.iter_mut().find(|x| x.name == col) {
            c.is_unique = true;
        }
    }

    Ok(TableSchema {
        table_name: table_name.to_string(),
        columns,
    })
}
