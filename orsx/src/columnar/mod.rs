mod orsxcol;
mod types;

pub use orsxcol::{decode_orsxcol_v1, decode_orsxcol_v1_into, encode_orsxcol_v1_into};
pub use types::{
    ColumnarAutoConfig, ColumnarBatch, ColumnarBatchReader, ColumnarField, ColumnarReadConfig,
    ColumnarReaderMode, ColumnarSchema, ColumnarType, CopyBinaryBatchReader,
    CopyBinaryBatchReaderConfig, RowWiseBatchReader,
};

pub trait OrsxColumnar {
    fn columnar_schema() -> crate::Result<ColumnarSchema>;
}
