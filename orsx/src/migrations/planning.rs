use crate::{quote_identifier, indexes::IndexInfo, schema::TableSpec};

use super::introspection::{ColumnInfo, TableSchema};
use super::config::MigrationConfig;
use super::introspection::IndexIdentity;

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

pub fn filter_ignored_diffs(cfg: &MigrationConfig, diffs: Vec<SchemaDiff>) -> Vec<SchemaDiff> {
    diffs.into_iter()
        // Column order is only enforced when explicitly requested.
        .filter(|d| cfg.enforce_column_order || !matches!(d, SchemaDiff::PositionChanged { .. }))
        // Extra columns in DB are only rejected when explicitly requested.
        .filter(|d| cfg.enforce_exact_columns || !matches!(d, SchemaDiff::ColumnRemoved(_)))
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
    let idx_name = derive_index_name(table_name, index);
    format!(
        "CREATE {unique}INDEX IF NOT EXISTS {} ON {} USING {} ({cols})",
        quote_identifier(&idx_name),
        quote_identifier(table_name),
        index.index_type.to_sql()
    )
}

pub async fn apply_safe_alters(
    pool: &sqlx::PgPool,
    table_name: &str,
    _spec: &TableSpec,
    cfg: &MigrationConfig,
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
                if cfg.enforce_column_order {
                    // Adding a column via ALTER TABLE appends at the end; for strict order we require rewrite.
                    continue;
                }
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

                    // Ensure uniqueness semantics via a unique index. This is idempotent by
                    // semantics: if an equivalent unique index already exists (any name), skip.
                    let existing_indexes =
                        super::introspection::read_table_index_identities(pool, table_name).await?;
                    if existing_indexes.iter().any(|e| {
                        e.unique
                            && e.method == "btree"
                            && e.columns.len() == 1
                            && e.columns[0] == *column
                    }) {
                        continue;
                    }
                    let idx_name = derive_unique_column_index_name(table_name, column.as_str());
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

    let _ = current;
    Ok(())
}

pub async fn ensure_indexes_concurrently(
    pool: &sqlx::PgPool,
    table_name: &str,
    spec: &TableSpec,
) -> crate::Result<()> {
    // Ensure declared indexes exist. Use CONCURRENTLY to avoid long write blocks on large tables.
    let existing_indexes = super::introspection::read_table_index_identities(pool, table_name).await?;

    let mut idxs_sorted: Vec<&IndexInfo> = spec.indexes.iter().collect();
    idxs_sorted.sort_by(|a, b| index_sort_key(a).cmp(&index_sort_key(b)));

    for idx in idxs_sorted {
        if has_equivalent_index(&existing_indexes, idx) {
            continue;
        }
        let sql = create_index_sql_concurrently(table_name, idx);
        let mut conn = pool.acquire().await?;
        sqlx::query(&sql).execute(&mut *conn).await?;
    }
    Ok(())
}

pub fn validate_strictness(cfg: &MigrationConfig, diffs: &[SchemaDiff]) -> crate::Result<()> {
    if cfg.enforce_exact_columns && !cfg.allow_destructive_drops {
        if diffs.iter().any(|d| matches!(d, SchemaDiff::ColumnRemoved(_))) {
            return Err(crate::Error::MigrationNeeded(
                "database has extra columns; enable allow_destructive_drops to rewrite and remove them (backup is kept)"
                    .to_string(),
            ));
        }
    }
    Ok(())
}

pub async fn apply_safe_renames(
    pool: &sqlx::PgPool,
    table_name: &str,
    spec: &TableSpec,
    cfg: &MigrationConfig,
    current: &TableSchema,
) -> crate::Result<Option<TableSchema>> {
    if !cfg.allow_column_renames {
        return Ok(None);
    }

    let mut renames: Vec<(&'static str, &'static str)> = spec
        .columns
        .iter()
        .filter_map(|c| c.rename_from.map(|from| (from, c.name)))
        .collect();
    if renames.is_empty() {
        return Ok(None);
    }
    renames.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(b.1)));

    let mut current_cols: std::collections::HashSet<String> =
        current.columns.iter().map(|c| c.name.clone()).collect();

    let mut any = false;
    for (from, to) in renames {
        if current_cols.contains(to) {
            continue;
        }
        if !current_cols.contains(from) {
            continue;
        }
        let sql = format!(
            "ALTER TABLE {} RENAME COLUMN {} TO {}",
            quote_identifier(table_name),
            quote_identifier(from),
            quote_identifier(to),
        );
        sqlx::query(&sql).execute(pool).await?;
        current_cols.remove(from);
        current_cols.insert(to.to_string());
        any = true;
    }

    if any {
        let updated = super::introspection::read_table_schema(pool, table_name).await?;
        return Ok(Some(updated));
    }
    Ok(None)
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
    let idx_name = derive_index_name(table_name, index);
    format!(
        "CREATE {unique}INDEX CONCURRENTLY IF NOT EXISTS {} ON {} USING {} ({cols})",
        quote_identifier(&idx_name),
        quote_identifier(table_name),
        index.index_type.to_sql()
    )
}

pub(crate) fn derive_index_name(table_name: &str, index: &IndexInfo) -> String {
    // Names must be unique within the schema. When the same struct spec is applied to multiple
    // table names, index names must incorporate the table name (otherwise they collide).
    let method = index.index_type.to_sql().to_lowercase();
    let cols = index.columns.join("_");
    let unique = if index.unique { "uq" } else { "ix" };

    let mut base = if index.name.is_empty() {
        format!("orsx_{unique}_{table_name}_{method}_{cols}")
    } else if index.name.contains(table_name) {
        // Assume the user already provided a table-specific name.
        index.name.to_string()
    } else {
        // Treat the provided name as a label but still make it table-specific.
        format!("{}_{}", index.name, table_name)
    };

    // Postgres identifier limit: 63 bytes.
    if base.len() <= 63 {
        return base;
    }

    // Stable hash suffix based on canonical identity.
    let canon = format!(
        "table={table_name}|method={method}|unique={}|cols={}",
        index.unique,
        index.columns.join(",")
    );
    let hash = crc32fast::hash(canon.as_bytes());
    let suffix = format!("_{hash:08x}");
    let max_prefix = 63usize.saturating_sub(suffix.len());
    base.truncate(max_prefix);
    base.push_str(&suffix);
    base
}

fn index_sort_key(idx: &IndexInfo) -> (u8, &'static str, String) {
    let u = if idx.unique { 0 } else { 1 };
    let m = idx.index_type.to_sql();
    let cols = idx.columns.join(",");
    (u, m, cols)
}

fn has_equivalent_index(existing: &[IndexIdentity], idx: &IndexInfo) -> bool {
    let want_method = idx.index_type.to_sql().to_lowercase();
    existing.iter().any(|e| {
        e.unique == idx.unique
            && e.method == want_method
            && e.columns.len() == idx.columns.len()
            && e.columns
                .iter()
                .zip(idx.columns.iter())
                .all(|(a, b)| a == b)
    })
}

fn derive_unique_column_index_name(table_name: &str, column: &str) -> String {
    let mut base = format!("orsx_uq_{table_name}_{column}");
    if base.len() <= 63 {
        return base;
    }
    let canon = format!("table={table_name}|method=btree|unique=true|cols={column}");
    let hash = crc32fast::hash(canon.as_bytes());
    let suffix = format!("_{hash:08x}");
    let max_prefix = 63usize.saturating_sub(suffix.len());
    base.truncate(max_prefix);
    base.push_str(&suffix);
    base
}
