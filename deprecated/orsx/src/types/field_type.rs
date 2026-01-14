use serde::{Deserialize, Serialize};

// PostgreSQL field type mapping
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    Text,
    Integer,
    BigInt,
    Real,
    DoublePrecision,
    Boolean,
    Timestamp,
    Bytea,
    Jsonb,
    Vector(usize), // pgvector with dimensions
    // PostgreSQL array types
    IntegerArray,
    BigIntArray,
    DoublePrecisionArray,
    TextArray,
}

impl FieldType {
    // Convert FieldType to PostgreSQL type string
    pub fn to_sql(&self) -> String {
        match self {
            FieldType::Text => "TEXT".to_string(),
            FieldType::Integer => "INTEGER".to_string(),
            FieldType::BigInt => "BIGINT".to_string(),
            FieldType::Real => "REAL".to_string(),
            FieldType::DoublePrecision => "DOUBLE PRECISION".to_string(),
            FieldType::Boolean => "BOOLEAN".to_string(),
            FieldType::Timestamp => "TIMESTAMPTZ".to_string(),
            FieldType::Bytea => "BYTEA".to_string(),
            FieldType::Jsonb => "JSONB".to_string(),
            FieldType::Vector(dim) => format!("vector({})", dim),
            FieldType::IntegerArray => "INTEGER[]".to_string(),
            FieldType::BigIntArray => "BIGINT[]".to_string(),
            FieldType::DoublePrecisionArray => "DOUBLE PRECISION[]".to_string(),
            FieldType::TextArray => "TEXT[]".to_string(),
        }
    }
}
