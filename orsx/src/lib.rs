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
    ColumnarAutoConfig, ColumnarBatch, ColumnarBatchReader, ColumnarField, ColumnarReaderMode,
    ColumnarSchema, ColumnarType, CopyBinaryBatchReader, RowWiseBatchReader,
    RowWiseBatchReaderConfig,
};

pub use sqlx;
pub use jiff::Timestamp;
pub use jiff_sqlx::Timestamp as SqlxTimestamp;

pub fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

pub use orsx_macros::OrsxMigrate;
pub use orsx_macros::OrsxColumnar;

pub mod prelude {
    pub use crate::{quote_identifier, ColumnSpec, Config, Error, FieldType, IndexInfo, IndexType};
    pub use crate::{Migrations, OrsxMigrate, Result, TableSpec};
    pub use crate::OrsxColumnar;
    pub use crate::{Compressed, CompressedWorkspace};
    pub use crate::{
        ColumnarAutoConfig, ColumnarBatch, ColumnarBatchReader, ColumnarField, ColumnarReaderMode,
        ColumnarSchema, ColumnarType, CopyBinaryBatchReader, RowWiseBatchReader,
        RowWiseBatchReaderConfig,
    };
    pub use crate::Timestamp;
    pub use crate::SqlxTimestamp;
    pub use sqlx;
}
