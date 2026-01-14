use crate::{error::Result, quote_identifier};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

// Index metadata for migrations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub index_type: IndexType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexType {
    BTree,
    Hash,
    Gin,
    Gist,
}

impl IndexType {
    pub fn to_sql(&self) -> &'static str {
        match self {
            IndexType::BTree => "BTREE",
            IndexType::Hash => "HASH",
            IndexType::Gin => "GIN",
            IndexType::Gist => "GIST",
        }
    }
}

impl IndexInfo {
    // Generate CREATE INDEX SQL
    pub fn to_create_sql(&self, table_name: &str) -> String {
        let unique_clause = if self.unique { "UNIQUE " } else { "" };
        let using_clause = format!("USING {}", self.index_type.to_sql());
        let columns_clause = self.columns.join(", ");

        format!(
            "CREATE {}INDEX IF NOT EXISTS {} ON {} {} ({})",
            unique_clause,
            quote_identifier(&self.name),
            quote_identifier(table_name),
            using_clause,
            columns_clause
        )
    }

    // Generate DROP INDEX SQL
    pub fn to_drop_sql(&self) -> String {
        format!("DROP INDEX IF EXISTS {}", quote_identifier(&self.name))
    }
}

// Create index in database
pub async fn create_index(pool: &PgPool, index: &IndexInfo, table_name: &str) -> Result<()> {
    let sql = index.to_create_sql(table_name);

    sqlx::query(&sql)
        .execute(pool)
        .await
        .map_err(|e| crate::Error::Migration {
            message: format!("Failed to create index '{}': {}", index.name, e),
            sql: Some(sql.clone()),
            context: Some("create_index".to_string()),
        })?;

    Ok(())
}

// Drop index from database
pub async fn drop_index(pool: &PgPool, index: &IndexInfo) -> Result<()> {
    let sql = index.to_drop_sql();

    sqlx::query(&sql)
        .execute(pool)
        .await
        .map_err(|e| crate::Error::Migration {
            message: format!("Failed to drop index '{}': {}", index.name, e),
            sql: Some(sql.clone()),
            context: Some("drop_index".to_string()),
        })?;

    Ok(())
}

// Read existing indexes from database
pub async fn introspect_indexes(pool: &PgPool, table_name: &str) -> Result<Vec<IndexInfo>> {
    let query = r#"
        SELECT
            i.indexname AS index_name,
            array_agg(a.attname ORDER BY array_position(ix.indkey, a.attnum)) AS columns,
            ix.indisunique AS is_unique,
            am.amname AS index_type
        FROM
            pg_indexes i
            JOIN pg_class c ON c.relname = i.indexname
            JOIN pg_index ix ON ix.indexrelid = c.oid
            JOIN pg_class t ON t.oid = ix.indrelid
            JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey)
            JOIN pg_am am ON am.oid = c.relam
        WHERE
            i.tablename = $1
            AND i.schemaname = 'public'
        GROUP BY
            i.indexname, ix.indisunique, am.amname
    "#;

    let rows = sqlx::query(query)
        .bind(table_name)
        .fetch_all(pool)
        .await
        .map_err(|e| crate::Error::Migration {
            message: format!(
                "Failed to introspect indexes for table '{}': {}",
                table_name, e
            ),
            sql: Some(query.to_string()),
            context: Some("introspect_indexes".to_string()),
        })?;

    let mut indexes = Vec::new();

    for row in rows {
        let index_name: String = row.try_get("index_name")?;
        let columns: Vec<String> = row.try_get("columns")?;
        let is_unique: bool = row.try_get("is_unique")?;
        let index_type_str: String = row.try_get("index_type")?;

        let index_type = match index_type_str.to_lowercase().as_str() {
            "btree" => IndexType::BTree,
            "hash" => IndexType::Hash,
            "gin" => IndexType::Gin,
            "gist" => IndexType::Gist,
            _ => IndexType::BTree, // Default
        };

        indexes.push(IndexInfo {
            name: index_name,
            columns,
            unique: is_unique,
            index_type,
        });
    }

    Ok(indexes)
}

// Compare indexes and create missing ones
pub async fn ensure_indexes(
    pool: &PgPool,
    table_name: &str,
    expected_indexes: &[IndexInfo],
) -> Result<Vec<String>> {
    let existing_indexes = introspect_indexes(pool, table_name).await?;
    let mut changes = Vec::new();

    for expected in expected_indexes {
        let exists = existing_indexes.iter().any(|existing| {
            existing.name == expected.name
                && existing.columns == expected.columns
                && existing.unique == expected.unique
        });

        if !exists {
            create_index(pool, expected, table_name).await?;
            changes.push(format!("Created index: {}", expected.name));
        }
    }

    Ok(changes)
}

#[derive(Debug)]
pub enum IndexDifference {
    NewIndex(IndexInfo),
    RemovedIndex(String),
}

// Compare desired vs actual indexes
pub fn compare_indexes(desired: &[IndexInfo], actual: &[IndexInfo]) -> Vec<IndexDifference> {
    let mut differences = Vec::new();

    // Find new indexes
    for desired_idx in desired {
        if !actual.iter().any(|a| a.name == desired_idx.name) {
            differences.push(IndexDifference::NewIndex(desired_idx.clone()));
        }
    }

    // Find removed indexes
    for actual_idx in actual {
        if !desired.iter().any(|d| d.name == actual_idx.name) {
            differences.push(IndexDifference::RemovedIndex(actual_idx.name.clone()));
        }
    }

    differences
}
