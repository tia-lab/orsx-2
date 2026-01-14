// Schema comparison logic - detect differences between expected and actual schemas
use super::introspection::ColumnInfo;
use std::collections::HashMap;

// Normalize PostgreSQL type names to canonical form
// PostgreSQL has type aliases that are functionally identical but have different names
// e.g., "TIMESTAMP WITH TIME ZONE" and "TIMESTAMPTZ" are the same type
fn normalize_postgres_type(sql_type: &str) -> String {
    match sql_type.to_uppercase().as_str() {
        "TIMESTAMP WITH TIME ZONE" | "TIMESTAMPTZ" => "TIMESTAMPTZ".to_string(),
        "CHARACTER VARYING" | "VARCHAR" => "VARCHAR".to_string(),
        "INTEGER" | "INT" | "INT4" => "INTEGER".to_string(),
        "BIGINT" | "INT8" => "BIGINT".to_string(),
        "DOUBLE PRECISION" | "FLOAT8" => "DOUBLE PRECISION".to_string(),
        "REAL" | "FLOAT4" => "REAL".to_string(),
        "BOOLEAN" | "BOOL" => "BOOLEAN".to_string(),
        "CHARACTER" | "CHAR" => "CHARACTER".to_string(),
        t => t.to_string(),
    }
}

// Schema difference types
#[derive(Debug, Clone)]
pub enum SchemaDifference {
    ColumnAdded {
        name: String,
        sql_type: String,
        nullable: bool,
    },
    ColumnRemoved {
        name: String,
    },
    TypeChanged {
        column: String,
        old_type: String,
        new_type: String,
    },
    NullabilityChanged {
        column: String,
        was_nullable: bool,
        now_nullable: bool,
    },
    ConstraintChanged {
        column: String,
        constraint_type: String,
        old_value: bool,
        new_value: bool,
    },
    ColumnOrderChanged {
        column: String,
        old_position: i32,
        new_position: i32,
    },
}

// Schema comparison result
#[derive(Debug, Clone)]
pub struct SchemaComparison {
    pub needs_migration: bool,
    pub differences: Vec<SchemaDifference>,
    pub current_columns: Vec<ColumnInfo>,
    pub expected_columns: Vec<ColumnInfo>,
}

// Compare current (DB) schema with expected (Orso trait) schema
pub fn compare_schemas(current: &[ColumnInfo], expected: &[ColumnInfo]) -> SchemaComparison {
    let mut differences = Vec::new();
    let mut needs_migration = false;

    // Create maps for easier lookup
    let current_map: HashMap<String, &ColumnInfo> =
        current.iter().map(|c| (c.name.clone(), c)).collect();
    let expected_map: HashMap<String, &ColumnInfo> =
        expected.iter().map(|c| (c.name.clone(), c)).collect();

    // Check for missing or changed columns in expected schema
    for expected_col in expected {
        match current_map.get(&expected_col.name) {
            Some(current_col) => {
                // Column exists - check for differences

                // Type mismatch - normalize types before comparison
                // This handles PostgreSQL type aliases (e.g., TIMESTAMPTZ vs TIMESTAMP WITH TIME ZONE)
                let current_normalized = normalize_postgres_type(&current_col.sql_type);
                let expected_normalized = normalize_postgres_type(&expected_col.sql_type);

                if current_normalized != expected_normalized {
                    differences.push(SchemaDifference::TypeChanged {
                        column: expected_col.name.clone(),
                        old_type: current_col.sql_type.clone(),
                        new_type: expected_col.sql_type.clone(),
                    });
                    needs_migration = true;
                }

                // Nullability mismatch
                if current_col.nullable != expected_col.nullable {
                    differences.push(SchemaDifference::NullabilityChanged {
                        column: expected_col.name.clone(),
                        was_nullable: current_col.nullable,
                        now_nullable: expected_col.nullable,
                    });
                    needs_migration = true;
                }

                // Unique constraint mismatch
                if current_col.is_unique != expected_col.is_unique {
                    differences.push(SchemaDifference::ConstraintChanged {
                        column: expected_col.name.clone(),
                        constraint_type: "UNIQUE".to_string(),
                        old_value: current_col.is_unique,
                        new_value: expected_col.is_unique,
                    });
                    needs_migration = true;
                }

                // Primary key mismatch
                if current_col.is_primary_key != expected_col.is_primary_key {
                    differences.push(SchemaDifference::ConstraintChanged {
                        column: expected_col.name.clone(),
                        constraint_type: "PRIMARY KEY".to_string(),
                        old_value: current_col.is_primary_key,
                        new_value: expected_col.is_primary_key,
                    });
                    needs_migration = true;
                }

                // Position mismatch - column order changed
                if current_col.position != expected_col.position {
                    differences.push(SchemaDifference::ColumnOrderChanged {
                        column: expected_col.name.clone(),
                        old_position: current_col.position,
                        new_position: expected_col.position,
                    });
                    needs_migration = true;
                }
            }
            None => {
                // Column doesn't exist in current schema - needs to be added
                differences.push(SchemaDifference::ColumnAdded {
                    name: expected_col.name.clone(),
                    sql_type: expected_col.sql_type.clone(),
                    nullable: expected_col.nullable,
                });
                needs_migration = true;
            }
        }
    }

    // Check for columns that exist in DB but not in expected schema (removed)
    for current_col in current {
        if !expected_map.contains_key(&current_col.name) {
            differences.push(SchemaDifference::ColumnRemoved {
                name: current_col.name.clone(),
            });
            needs_migration = true;
        }
    }

    SchemaComparison {
        needs_migration,
        differences,
        current_columns: current.to_vec(),
        expected_columns: expected.to_vec(),
    }
}

impl SchemaDifference {
    // Human-readable description
    pub fn describe(&self) -> String {
        match self {
            SchemaDifference::ColumnAdded {
                name,
                sql_type,
                nullable,
            } => {
                format!(
                    "Add column '{}' ({}, {})",
                    name,
                    sql_type,
                    if *nullable { "NULL" } else { "NOT NULL" }
                )
            }
            SchemaDifference::ColumnRemoved { name } => {
                format!("Remove column '{}'", name)
            }
            SchemaDifference::TypeChanged {
                column,
                old_type,
                new_type,
            } => {
                format!(
                    "Change column '{}' type: {} → {}",
                    column, old_type, new_type
                )
            }
            SchemaDifference::NullabilityChanged {
                column,
                was_nullable,
                now_nullable,
            } => {
                format!(
                    "Change column '{}' nullability: {} → {}",
                    column,
                    if *was_nullable { "NULL" } else { "NOT NULL" },
                    if *now_nullable { "NULL" } else { "NOT NULL" }
                )
            }
            SchemaDifference::ConstraintChanged {
                column,
                constraint_type,
                old_value,
                new_value,
            } => {
                format!(
                    "Change column '{}' {}: {} → {}",
                    column, constraint_type, old_value, new_value
                )
            }
            SchemaDifference::ColumnOrderChanged {
                column,
                old_position,
                new_position,
            } => {
                format!(
                    "Change column '{}' position: {} → {}",
                    column, old_position, new_position
                )
            }
        }
    }
}
