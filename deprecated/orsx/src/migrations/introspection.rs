// Schema introspection via PostgreSQL information_schema
use crate::{types::FieldType, Result};
use sqlx::PgPool;

// Column metadata from database
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub sql_type: String,
    pub nullable: bool,
    pub position: i32,
    pub is_unique: bool,
    pub is_primary_key: bool,
    pub foreign_key_reference: Option<String>,
    pub has_default: bool,
    pub is_compressed: bool,
}

// Complete table schema
#[derive(Debug, Clone)]
pub struct TableSchema {
    pub table_name: String,
    pub columns: Vec<ColumnInfo>,
}

// Check if table exists in public schema
pub async fn table_exists(pool: &PgPool, table_name: &str) -> Result<bool> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM information_schema.tables
         WHERE table_schema = 'public' AND table_name = $1)",
    )
    .bind(table_name)
    .fetch_one(pool)
    .await?;

    Ok(exists)
}

// Check if specific column exists in table
pub async fn column_exists(pool: &PgPool, table_name: &str, column_name: &str) -> Result<bool> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM information_schema.columns
         WHERE table_schema = 'public' AND table_name = $1 AND column_name = $2)",
    )
    .bind(table_name)
    .bind(column_name)
    .fetch_one(pool)
    .await?;

    Ok(exists)
}

// Read complete table schema from database
pub async fn read_table_schema(pool: &PgPool, table_name: &str) -> Result<TableSchema> {
    // Get column information
    let column_rows = sqlx::query_as::<_, (String, String, String, i32, Option<String>)>(
        "SELECT
            column_name,
            CASE
                WHEN data_type = 'ARRAY' THEN
                    (SELECT format_type(a.atttypid, a.atttypmod)
                     FROM pg_attribute a
                     JOIN pg_class c ON c.oid = a.attrelid
                     WHERE c.relname = $1 AND a.attname = column_name)
                ELSE data_type
            END as data_type,
            is_nullable,
            ordinal_position,
            column_default
        FROM information_schema.columns
        WHERE table_schema = 'public' AND table_name = $1
        ORDER BY ordinal_position",
    )
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    let mut columns = Vec::new();

    for (name, data_type, is_nullable, ordinal_position, column_default) in column_rows {
        let column_info = ColumnInfo {
            name: name.clone(),
            sql_type: data_type.to_uppercase(),
            nullable: is_nullable == "YES",
            position: ordinal_position - 1, // Convert to 0-indexed
            is_unique: false,
            is_primary_key: false,
            foreign_key_reference: None,
            has_default: column_default.is_some(),
            is_compressed: data_type.to_uppercase() == "BYTEA",
        };
        columns.push(column_info);
    }

    // Get constraint information (primary keys, unique constraints)
    let constraint_rows = sqlx::query_as::<_, (String, String)>(
        "SELECT kcu.column_name, tc.constraint_type
         FROM information_schema.table_constraints tc
         JOIN information_schema.key_column_usage kcu
         ON tc.constraint_name = kcu.constraint_name
         WHERE tc.table_schema = 'public' AND tc.table_name = $1
         AND tc.constraint_type IN ('PRIMARY KEY', 'UNIQUE')",
    )
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    // Update column flags based on constraints
    for (column_name, constraint_type) in constraint_rows {
        if let Some(col) = columns.iter_mut().find(|c| c.name == column_name) {
            match constraint_type.as_str() {
                "PRIMARY KEY" => {
                    col.is_primary_key = true;
                    col.is_unique = true;
                }
                "UNIQUE" => col.is_unique = true,
                _ => {}
            }
        }
    }

    // Get foreign key information
    let fk_rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT
            kcu.column_name,
            ccu.table_name AS referenced_table_name,
            ccu.column_name AS referenced_column_name
         FROM information_schema.referential_constraints rc
         JOIN information_schema.key_column_usage kcu
         ON rc.constraint_name = kcu.constraint_name
         JOIN information_schema.constraint_column_usage ccu
         ON rc.unique_constraint_name = ccu.constraint_name
         WHERE kcu.table_schema = 'public' AND kcu.table_name = $1",
    )
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    // Update foreign key references
    for (column_name, ref_table, ref_column) in fk_rows {
        if let Some(col) = columns.iter_mut().find(|c| c.name == column_name) {
            col.foreign_key_reference = Some(format!("{}.{}", ref_table, ref_column));
        }
    }

    Ok(TableSchema {
        table_name: table_name.to_string(),
        columns,
    })
}

// Map PostgreSQL type to FieldType
pub fn postgres_type_to_field_type(sql_type: &str) -> FieldType {
    match sql_type {
        "TEXT" | "VARCHAR" | "CHAR" | "CHARACTER VARYING" => FieldType::Text,
        "INTEGER" | "INT" | "INT4" => FieldType::Integer,
        "BIGINT" | "INT8" => FieldType::BigInt,
        "REAL" | "FLOAT4" => FieldType::Real,
        "DOUBLE PRECISION" | "FLOAT8" => FieldType::DoublePrecision,
        "BOOLEAN" | "BOOL" => FieldType::Boolean,
        "TIMESTAMP WITH TIME ZONE" | "TIMESTAMPTZ" => FieldType::Timestamp,
        "BYTEA" => FieldType::Bytea,
        s if s.starts_with("vector(") => {
            // Extract dimensions from vector(N)
            let dims = s
                .trim_start_matches("vector(")
                .trim_end_matches(')')
                .parse::<usize>()
                .unwrap_or(384);
            FieldType::Vector(dims)
        }
        _ => FieldType::Text, // Default fallback
    }
}
