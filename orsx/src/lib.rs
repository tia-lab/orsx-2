pub mod error;
pub mod config;
pub mod indexes;
pub mod migrations;
pub mod schema;
pub mod types;
pub mod compression;
pub mod compressed;
pub mod columnar;

pub use error::{Error, Result};
pub use config::Config;
pub use indexes::{IndexInfo, IndexType};
pub use migrations::Migrations;
pub use schema::{ColumnSpec, OrsxMigrate, TableSpec};
pub use types::FieldType;
pub use compressed::{Compressed, CompressedWorkspace};
pub use columnar::{
    ColumnarBatch, ColumnarField, ColumnarSchema, ColumnarType, CopyBinaryBatchReader,
};

pub use sqlx;
pub use jiff::Timestamp;
pub use jiff_sqlx::Timestamp as SqlxTimestamp;

pub fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

pub use orsx_macros::OrsxMigrate;

pub mod prelude {
    pub use crate::{quote_identifier, ColumnSpec, Config, Error, FieldType, IndexInfo, IndexType};
    pub use crate::{Migrations, OrsxMigrate, Result, TableSpec};
    pub use crate::{Compressed, CompressedWorkspace};
    pub use crate::{
        ColumnarBatch, ColumnarField, ColumnarSchema, ColumnarType, CopyBinaryBatchReader,
    };
    pub use crate::Timestamp;
    pub use crate::SqlxTimestamp;
    pub use sqlx;
}
