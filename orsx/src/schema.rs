use crate::{indexes::IndexInfo, quote_identifier, types::FieldType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnSpec {
    pub name: &'static str,
    /// If set, this column is allowed to be renamed from the given existing DB column name.
    /// Used to perform `ALTER TABLE ... RENAME COLUMN ...` or to map source columns during online rewrite.
    pub rename_from: Option<&'static str>,
    pub ty: FieldType,
    pub nullable: bool,
    pub primary_key: bool,
    pub unique: bool,
    pub default_sql: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSpec {
    pub table_name: &'static str,
    pub columns: &'static [ColumnSpec],
    pub indexes: &'static [IndexInfo],
}

pub trait OrsxMigrate: Send + Sync {
    fn spec() -> TableSpec;

    fn table_name() -> &'static str {
        Self::spec().table_name
    }

    fn create_table_sql(table_name_override: Option<&str>) -> String {
        let spec = Self::spec();
        let table_name = table_name_override.unwrap_or(spec.table_name);

        let mut lines: Vec<String> = Vec::with_capacity(spec.columns.len() + 4);

        for col in spec.columns {
            let mut line = format!(
                "{} {}",
                quote_identifier(col.name),
                col.ty.to_sql()
            );

            if let Some(default_sql) = col.default_sql {
                line.push_str(" DEFAULT ");
                line.push_str(default_sql);
            }

            if col.primary_key {
                line.push_str(" PRIMARY KEY");
            }
            if col.unique && !col.primary_key {
                line.push_str(" UNIQUE");
            }
            if !col.nullable && !col.primary_key {
                line.push_str(" NOT NULL");
            }

            lines.push(line);
        }

        format!(
            "CREATE TABLE IF NOT EXISTS {} (\n  {}\n)",
            quote_identifier(table_name),
            lines.join(",\n  ")
        )
    }
}
