use crate::{quote_identifier, indexes::IndexInfo, schema::TableSpec};

use super::introspection::{ColumnInfo, TableSchema};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaDiff {
    ColumnAdded(String),
    ColumnRemoved(String),
    TypeChanged { column: String, from: String, to: String },
    NullabilityChanged { column: String, from: bool, to: bool },
    ConstraintChanged { column: String, kind: &'static str, from: bool, to: bool },
    PositionChanged { column: String, from: i32, to: i32 },
}

pub fn expected_schema_from_spec(table_name: &str, spec: &TableSpec) -> TableSchema {
    TableSchema {
        table_name: table_name.to_string(),
        columns: spec
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| ColumnInfo {
                name: c.name.to_string(),
                sql_type: normalize_type(&c.ty.to_sql()),
                nullable: c.nullable && !c.primary_key,
                position: i as i32,
                is_primary_key: c.primary_key,
                is_unique: c.unique || c.primary_key,
            })
            .collect(),
    }
}

pub fn diff_schema(current: &TableSchema, expected: &TableSchema) -> Vec<SchemaDiff> {
    let mut diffs = Vec::new();

    let current_map: std::collections::HashMap<&str, &ColumnInfo> =
        current.columns.iter().map(|c| (c.name.as_str(), c)).collect();
    let expected_map: std::collections::HashMap<&str, &ColumnInfo> =
        expected.columns.iter().map(|c| (c.name.as_str(), c)).collect();

    for exp in &expected.columns {
        match current_map.get(exp.name.as_str()) {
            None => diffs.push(SchemaDiff::ColumnAdded(exp.name.clone())),
            Some(cur) => {
                let cur_ty = normalize_type(&cur.sql_type);
                let exp_ty = normalize_type(&exp.sql_type);
                if cur_ty != exp_ty {
                    diffs.push(SchemaDiff::TypeChanged {
                        column: exp.name.clone(),
                        from: cur.sql_type.clone(),
                        to: exp.sql_type.clone(),
                    });
                }
                if cur.nullable != exp.nullable {
                    diffs.push(SchemaDiff::NullabilityChanged {
                        column: exp.name.clone(),
                        from: cur.nullable,
                        to: exp.nullable,
                    });
                }
                if cur.is_primary_key != exp.is_primary_key {
                    diffs.push(SchemaDiff::ConstraintChanged {
                        column: exp.name.clone(),
                        kind: "PRIMARY KEY",
                        from: cur.is_primary_key,
                        to: exp.is_primary_key,
                    });
                }
                if cur.is_unique != exp.is_unique {
                    diffs.push(SchemaDiff::ConstraintChanged {
                        column: exp.name.clone(),
                        kind: "UNIQUE",
                        from: cur.is_unique,
                        to: exp.is_unique,
                    });
                }
                if cur.position != exp.position {
                    diffs.push(SchemaDiff::PositionChanged {
                        column: exp.name.clone(),
                        from: cur.position,
                        to: exp.position,
                    });
                }
            }
        }
    }

    for cur in &current.columns {
        if !expected_map.contains_key(cur.name.as_str()) {
            diffs.push(SchemaDiff::ColumnRemoved(cur.name.clone()));
        }
    }

    diffs
}

pub fn filter_ignored_diffs(diffs: Vec<SchemaDiff>) -> Vec<SchemaDiff> {
    diffs.into_iter()
        // Column order is not a compatibility requirement for Postgres, and forcing it is expensive.
        .filter(|d| !matches!(d, SchemaDiff::PositionChanged { .. }))
        // Extra columns in DB are tolerated by default (non-destructive; avoids data loss).
        .filter(|d| !matches!(d, SchemaDiff::ColumnRemoved(_)))
        .collect()
}

fn normalize_type(s: &str) -> String {
    match s.to_uppercase().as_str() {
        "TIMESTAMP WITH TIME ZONE" | "TIMESTAMPTZ" => "TIMESTAMPTZ".to_string(),
        "CHARACTER VARYING" | "VARCHAR" => "VARCHAR".to_string(),
        "INT" | "INT4" | "INTEGER" => "INTEGER".to_string(),
        "INT8" | "BIGINT" => "BIGINT".to_string(),
        "FLOAT8" | "DOUBLE PRECISION" => "DOUBLE PRECISION".to_string(),
        "FLOAT4" | "REAL" => "REAL".to_string(),
        other => other.to_string(),
    }
}

pub fn create_index_sql(table_name: &str, index: &IndexInfo) -> String {
    let unique = if index.unique { "UNIQUE " } else { "" };
    let cols = index
        .columns
        .iter()
        .map(|c| quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "CREATE {unique}INDEX IF NOT EXISTS {} ON {} USING {} ({cols})",
        quote_identifier(index.name),
        quote_identifier(table_name),
        index.index_type.to_sql()
    )
}

pub async fn apply_safe_alters(
    pool: &sqlx::PgPool,
    table_name: &str,
    spec: &TableSpec,
    current: &TableSchema,
    expected: &TableSchema,
    diffs: &[SchemaDiff],
) -> crate::Result<()> {
    // Deterministic ordering: apply in a stable sequence by diff kind and column name.
    let mut diffs_sorted = diffs.to_vec();
    diffs_sorted.sort_by(|a, b| diff_sort_key(a).cmp(&diff_sort_key(b)));

    for diff in diffs_sorted {
        match diff {
            SchemaDiff::ColumnAdded(col) => {
                let spec = expected
                    .columns
                    .iter()
                    .find(|c| c.name == col)
                    .ok_or_else(|| crate::Error::Other("expected column missing".to_string()))?;

                // Only allow adding nullable columns or columns with no NOT NULL requirement.
                if !spec.nullable && !spec.is_primary_key {
                    // Not safe as an ALTER for big tables; leave for online/offline rewrite path.
                    continue;
                }

                let sql = format!(
                    "ALTER TABLE {} ADD COLUMN IF NOT EXISTS {} {}{}",
                    quote_identifier(table_name),
                    quote_identifier(&spec.name),
                    spec.sql_type,
                    if spec.nullable { "" } else { " NOT NULL" }
                );
                sqlx::query(&sql).execute(pool).await?;
            }
            SchemaDiff::NullabilityChanged { column, from, to } => {
                // Allow loosening nullability (DROP NOT NULL). Tightening requires scan/validation.
                if from && !to {
                    // Not safe without explicit validation; leave for rewrite path.
                    continue;
                }
                if !from && to {
                    let sql = format!(
                        "ALTER TABLE {} ALTER COLUMN {} DROP NOT NULL",
                        quote_identifier(table_name),
                        quote_identifier(&column),
                    );
                    sqlx::query(&sql).execute(pool).await?;
                }
            }
            SchemaDiff::ConstraintChanged { column, kind, from: _, to } => {
                // Only add UNIQUE via a unique index; never drop constraints by default.
                if kind == "UNIQUE" && to {
                    // Find expected uniqueness from expected schema.
                    let exp = expected
                        .columns
                        .iter()
                        .find(|c| c.name == column)
                        .ok_or_else(|| crate::Error::Other("expected column missing".to_string()))?;
                    if !exp.is_unique {
                        continue;
                    }

                    // Deterministic names. Create unique index to satisfy uniqueness semantics.
                    let idx_name = format!("orsx_uq_{table_name}_{column}");
                    let create_idx = format!(
                        "CREATE UNIQUE INDEX CONCURRENTLY IF NOT EXISTS {} ON {} ({})",
                        quote_identifier(&idx_name),
                        quote_identifier(table_name),
                        quote_identifier(&column),
                    );
                    // `CONCURRENTLY` must run outside an explicit transaction block.
                    // Use a dedicated acquired connection to avoid accidental transaction usage.
                    let mut conn = pool.acquire().await?;
                    sqlx::query(&create_idx).execute(&mut *conn).await?;
                }
            }
            SchemaDiff::ColumnRemoved(_)
            | SchemaDiff::TypeChanged { .. }
            | SchemaDiff::PositionChanged { .. } => {
                // Leave for rewrite path.
                continue;
            }
        }
    }

    // Ensure declared indexes exist. Use CONCURRENTLY to avoid long write blocks on large tables.
    for idx in spec.indexes {
        let sql = create_index_sql_concurrently(table_name, idx);
        let mut conn = pool.acquire().await?;
        sqlx::query(&sql).execute(&mut *conn).await?;
    }

    let _ = current;
    Ok(())
}

fn diff_sort_key(d: &SchemaDiff) -> (u8, String) {
    match d {
        SchemaDiff::ColumnAdded(c) => (0, c.clone()),
        SchemaDiff::NullabilityChanged { column, .. } => (1, column.clone()),
        SchemaDiff::ConstraintChanged { column, kind, .. } => (2, format!("{kind}:{column}")),
        SchemaDiff::TypeChanged { column, .. } => (3, column.clone()),
        SchemaDiff::ColumnRemoved(c) => (4, c.clone()),
        SchemaDiff::PositionChanged { column, .. } => (5, column.clone()),
    }
}

fn create_index_sql_concurrently(table_name: &str, index: &IndexInfo) -> String {
    let unique = if index.unique { "UNIQUE " } else { "" };
    let cols = index
        .columns
        .iter()
        .map(|c| quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "CREATE {unique}INDEX CONCURRENTLY IF NOT EXISTS {} ON {} USING {} ({cols})",
        quote_identifier(index.name),
        quote_identifier(table_name),
        index.index_type.to_sql()
    )
}
