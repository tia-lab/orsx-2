use crate::{quote_identifier, OrsxMigrate};
use sqlx::PgPool;

// Helper trait for dynamic table name operations
// Uses runtime SQL validation (sqlx::query_as) for dynamic table names
//
// Note: This trait provides runtime table name support at the cost of compile-time SQL verification.
// For static table names, use sqlx::query! directly for compile-time safety.
#[async_trait::async_trait]
pub trait TableQuery: OrsxMigrate + Sized + Unpin + Send
where
    for<'r> Self: sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
    // Insert record into specified table
    async fn insert_into_table(&self, pool: &PgPool, table: &str) -> sqlx::Result<()>
    where
        Self: Sync,
    {
        // Extract field metadata from OrsxMigrate trait
        let field_names = Self::field_names();

        // Build INSERT SQL with placeholders
        let columns = field_names.to_vec();
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("${}", i)).collect();

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            quote_identifier(table),
            columns.join(", "),
            placeholders.join(", ")
        );

        // Create query and bind all field values using generated method
        let query = sqlx::query(&sql);
        let query = self.bind_values_to_query(query);

        // Execute INSERT
        query.execute(pool).await?;

        Ok(())
    }

    // Fetch all records from specified table
    async fn fetch_all_from_table(pool: &PgPool, table: &str) -> sqlx::Result<Vec<Self>> {
        let sql = format!("SELECT * FROM {}", quote_identifier(table));
        sqlx::query_as(&sql).fetch_all(pool).await
    }

    // Count records in specified table
    async fn count_in_table(pool: &PgPool, table: &str) -> sqlx::Result<i64> {
        let sql = format!("SELECT COUNT(*) as count FROM {}", quote_identifier(table));
        let row: (i64,) = sqlx::query_as(&sql).fetch_one(pool).await?;
        Ok(row.0)
    }

    // Update record in specified table (WHERE clause matches primary key)
    async fn update_in_table(&self, pool: &PgPool, table: &str) -> sqlx::Result<u64>
    where
        Self: Sync,
    {
        let field_names = Self::field_names();
        let pk_field = Self::primary_key();

        if pk_field.is_none() {
            return Err(sqlx::Error::Protocol(
                "No primary key defined for update".into(),
            ));
        }

        let pk_field = pk_field.unwrap();

        // Build SET clause (exclude primary key from SET, use it in WHERE)
        let set_parts: Vec<String> = field_names
            .iter()
            .filter(|&name| *name != pk_field)
            .enumerate()
            .map(|(idx, name)| format!("{} = ${}", name, idx + 1))
            .collect();

        let where_param_num = set_parts.len() + 1;

        let sql = format!(
            "UPDATE {} SET {} WHERE {} = ${}",
            quote_identifier(table),
            set_parts.join(", "),
            pk_field,
            where_param_num
        );

        // Bind values in correct order (non-PK fields for SET, PK for WHERE)
        let query = sqlx::query(&sql);
        let query = self.bind_values_for_update(query);

        let result = query.execute(pool).await?;
        Ok(result.rows_affected())
    }

    // Delete record from specified table by primary key
    async fn delete_from_table(pool: &PgPool, table: &str, id: &str) -> sqlx::Result<u64>
    where
        Self: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
    {
        let pk_field = Self::primary_key();

        if pk_field.is_none() {
            return Err(sqlx::Error::Protocol(
                "No primary key defined for delete".into(),
            ));
        }

        let pk_field = pk_field.unwrap();

        let sql = format!(
            "DELETE FROM {} WHERE {} = $1",
            quote_identifier(table),
            pk_field
        );

        let result = sqlx::query(&sql).bind(id).execute(pool).await?;
        Ok(result.rows_affected())
    }

    // Find record by primary key
    async fn find_by_id_in_table(pool: &PgPool, table: &str, id: &str) -> sqlx::Result<Option<Self>>
    where
        Self: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
    {
        let pk_field = Self::primary_key();

        if pk_field.is_none() {
            return Err(sqlx::Error::Protocol(
                "No primary key defined for find_by_id".into(),
            ));
        }

        let pk_field = pk_field.unwrap();

        let sql = format!(
            "SELECT * FROM {} WHERE {} = $1",
            quote_identifier(table),
            pk_field
        );

        sqlx::query_as(&sql).bind(id).fetch_optional(pool).await
    }
}

// Blanket implementation for any type implementing required traits
impl<T> TableQuery for T
where
    T: OrsxMigrate + Unpin + Send,
    for<'r> T: sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
}
