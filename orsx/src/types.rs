use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    Text,
    Uuid,
    Integer,
    BigInt,
    Real,
    DoublePrecision,
    Boolean,
    TimestampTz,
    Bytea,
    Jsonb,
    Vector(usize),
    IntegerArray,
    BigIntArray,
    DoublePrecisionArray,
    TextArray,
}

impl FieldType {
    pub fn to_sql(&self) -> String {
        match self {
            FieldType::Text => "TEXT".to_string(),
            FieldType::Uuid => "UUID".to_string(),
            FieldType::Integer => "INTEGER".to_string(),
            FieldType::BigInt => "BIGINT".to_string(),
            FieldType::Real => "REAL".to_string(),
            FieldType::DoublePrecision => "DOUBLE PRECISION".to_string(),
            FieldType::Boolean => "BOOLEAN".to_string(),
            FieldType::TimestampTz => "TIMESTAMPTZ".to_string(),
            FieldType::Bytea => "BYTEA".to_string(),
            FieldType::Jsonb => "JSONB".to_string(),
            FieldType::Vector(dim) => format!("vector({dim})"),
            FieldType::IntegerArray => "INTEGER[]".to_string(),
            FieldType::BigIntArray => "BIGINT[]".to_string(),
            FieldType::DoublePrecisionArray => "DOUBLE PRECISION[]".to_string(),
            FieldType::TextArray => "TEXT[]".to_string(),
        }
    }
}
