use crate::{quote_identifier, OrsxMigrate};
use sqlx::postgres::PgRow;
use sqlx::PgPool;

// Batch operations trait for high-performance bulk database operations
// Automatically selects optimal strategy based on batch size
#[async_trait::async_trait]
pub trait BatchTableQuery: OrsxMigrate + Sized + Unpin + Send
where
    for<'r> Self: sqlx::FromRow<'r, PgRow>,
{
    // Batch insert with automatic strategy selection based on size
    async fn batch_insert_into_table(
        records: &[Self],
        pool: &PgPool,
        table: &str,
    ) -> sqlx::Result<()>
    where
        Self: Sync,
    {
        if records.is_empty() {
            return Ok(());
        }

        let record_count = records.len();

        // Strategy selection based on batch size
        match record_count {
            // Small batch: Use transaction with individual inserts
            1..=10 => Self::batch_insert_transaction(records, pool, table).await,

            // Medium batch: Use multi-value INSERT
            11..=1000 => Self::batch_insert_multi_value(records, pool, table).await,

            // Large batch: Use COPY for maximum performance
            _ => Self::batch_insert_copy(records, pool, table).await,
        }
    }

    // Transaction-based insert for small batches
    async fn batch_insert_transaction(
        records: &[Self],
        pool: &PgPool,
        table: &str,
    ) -> sqlx::Result<()>
    where
        Self: Sync,
    {
        let mut tx = pool.begin().await?;

        for record in records {
            // Use existing TableQuery trait method
            let field_names = Self::field_names();
            let placeholders: Vec<String> =
                (1..=field_names.len()).map(|i| format!("${}", i)).collect();

            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                quote_identifier(table),
                field_names.join(", "),
                placeholders.join(", ")
            );

            let query = sqlx::query(&sql);
            let query = record.bind_values_to_query(query);
            query.execute(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    // Multi-value INSERT for medium batches
    async fn batch_insert_multi_value(
        records: &[Self],
        pool: &PgPool,
        table: &str,
    ) -> sqlx::Result<()>
    where
        Self: Sync,
    {
        let field_names = Self::field_names();
        let field_count = field_names.len();

        // Build the SQL with placeholders for all records
        let mut sql = format!(
            "INSERT INTO {} ({}) VALUES ",
            quote_identifier(table),
            field_names.join(", ")
        );

        let mut value_groups = Vec::new();
        for i in 0..records.len() {
            let placeholders: Vec<String> = (1..=field_count)
                .map(|j| format!("${}", i * field_count + j))
                .collect();
            value_groups.push(format!("({})", placeholders.join(", ")));
        }

        sql.push_str(&value_groups.join(", "));

        // Bind all values in order
        let mut query = sqlx::query(&sql);
        for record in records {
            query = record.bind_values_to_query(query);
        }

        query.execute(pool).await?;
        Ok(())
    }

    // COPY-based insert for large batches
    async fn batch_insert_copy(records: &[Self], pool: &PgPool, table: &str) -> sqlx::Result<()>
    where
        Self: Sync,
    {
        let field_names = Self::field_names();

        // Prepare COPY command
        let _copy_sql = format!(
            "COPY {} ({}) FROM STDIN WITH (FORMAT CSV, HEADER false, DELIMITER ',', NULL 'NULL')",
            quote_identifier(table),
            field_names.join(", ")
        );

        // For now, fall back to multi-value for very large batches
        // Full binary COPY would require implementing binary encoding for each field type
        // This is a pragmatic approach that still provides good performance

        // Split into chunks of 1000 for multi-value inserts
        for chunk in records.chunks(1000) {
            Self::batch_insert_multi_value(chunk, pool, table).await?;
        }

        Ok(())
    }

    // Batch update with automatic strategy selection
    async fn batch_update_in_table(
        records: &[Self],
        pool: &PgPool,
        table: &str,
    ) -> sqlx::Result<u64>
    where
        Self: Sync,
    {
        if records.is_empty() {
            return Ok(0);
        }

        let pk_field = Self::primary_key();
        if pk_field.is_none() {
            return Err(sqlx::Error::Protocol(
                "No primary key defined for batch update".into(),
            ));
        }

        let record_count = records.len();

        match record_count {
            // Small batch: Use transaction with individual updates
            1..=10 => Self::batch_update_transaction(records, pool, table).await,

            // Larger batches: Use UPDATE with CASE statements
            _ => Self::batch_update_case(records, pool, table).await,
        }
    }

    // Transaction-based update for small batches
    async fn batch_update_transaction(
        records: &[Self],
        pool: &PgPool,
        table: &str,
    ) -> sqlx::Result<u64>
    where
        Self: Sync,
    {
        let mut tx = pool.begin().await?;
        let mut total_affected = 0u64;

        let field_names = Self::field_names();
        let pk_field = Self::primary_key().unwrap();

        for record in records {
            // Build UPDATE SQL
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

            let query = sqlx::query(&sql);
            let query = record.bind_values_for_update(query);

            let result = query.execute(&mut *tx).await?;
            total_affected += result.rows_affected();
        }

        tx.commit().await?;
        Ok(total_affected)
    }

    // CASE-based update for larger batches
    async fn batch_update_case(records: &[Self], pool: &PgPool, table: &str) -> sqlx::Result<u64>
    where
        Self: Sync,
    {
        // For simplicity, chunk and use transactions
        // A full CASE implementation would be more complex
        let mut total_affected = 0u64;

        for chunk in records.chunks(100) {
            let affected = Self::batch_update_transaction(chunk, pool, table).await?;
            total_affected += affected;
        }

        Ok(total_affected)
    }

    // Batch delete by IDs
    async fn batch_delete_from_table(
        pool: &PgPool,
        table: &str,
        ids: &[String],
    ) -> sqlx::Result<u64>
    where
        Self: for<'r> sqlx::FromRow<'r, PgRow>,
    {
        if ids.is_empty() {
            return Ok(0);
        }

        let pk_field = Self::primary_key();
        if pk_field.is_none() {
            return Err(sqlx::Error::Protocol(
                "No primary key defined for batch delete".into(),
            ));
        }

        let pk_field = pk_field.unwrap();

        match ids.len() {
            // Small batch: Use IN clause
            1..=100 => {
                let placeholders: Vec<String> =
                    (1..=ids.len()).map(|i| format!("${}", i)).collect();

                let sql = format!(
                    "DELETE FROM {} WHERE {} IN ({})",
                    quote_identifier(table),
                    pk_field,
                    placeholders.join(", ")
                );

                let mut query = sqlx::query(&sql);
                for id in ids {
                    query = query.bind(id);
                }

                let result = query.execute(pool).await?;
                Ok(result.rows_affected())
            }

            // Larger batch: Use ANY(ARRAY)
            _ => {
                let sql = format!(
                    "DELETE FROM {} WHERE {} = ANY($1)",
                    quote_identifier(table),
                    pk_field
                );

                let result = sqlx::query(&sql).bind(ids).execute(pool).await?;

                Ok(result.rows_affected())
            }
        }
    }

    // Batch upsert (INSERT ... ON CONFLICT DO UPDATE)
    async fn batch_upsert_into_table(
        records: &[Self],
        pool: &PgPool,
        table: &str,
        conflict_columns: &[&str],
        update_columns: &[&str],
    ) -> sqlx::Result<u64>
    where
        Self: Sync,
    {
        if records.is_empty() {
            return Ok(0);
        }

        let field_names = Self::field_names();

        // Build INSERT part
        let mut sql = format!(
            "INSERT INTO {} ({}) VALUES ",
            quote_identifier(table),
            field_names.join(", ")
        );

        // Add value placeholders
        let field_count = field_names.len();
        let mut value_groups = Vec::new();

        for i in 0..records.len() {
            let placeholders: Vec<String> = (1..=field_count)
                .map(|j| format!("${}", i * field_count + j))
                .collect();
            value_groups.push(format!("({})", placeholders.join(", ")));
        }

        sql.push_str(&value_groups.join(", "));

        // Add ON CONFLICT clause
        sql.push_str(&format!(
            " ON CONFLICT ({}) DO UPDATE SET ",
            conflict_columns
                .iter()
                .map(|c| quote_identifier(c))
                .collect::<Vec<_>>()
                .join(", ")
        ));

        // Add UPDATE SET clause
        let update_parts: Vec<String> = update_columns
            .iter()
            .map(|col| {
                format!(
                    "{} = EXCLUDED.{}",
                    quote_identifier(col),
                    quote_identifier(col)
                )
            })
            .collect();

        sql.push_str(&update_parts.join(", "));

        // Bind all values
        let mut query = sqlx::query(&sql);
        for record in records {
            query = record.bind_values_to_query(query);
        }

        let result = query.execute(pool).await?;
        Ok(result.rows_affected())
    }
}

// Blanket implementation for any type implementing required traits
impl<T> BatchTableQuery for T
where
    T: OrsxMigrate + Unpin + Send,
    for<'r> T: sqlx::FromRow<'r, PgRow>,
{
}
