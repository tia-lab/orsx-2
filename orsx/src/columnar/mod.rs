mod orsxcol;
mod orsxcol_v2;
mod types;

pub use orsxcol::{decode_orsxcol_v1, decode_orsxcol_v1_into, encode_orsxcol_v1_into};
pub use orsxcol_v2::{
    decode_orsxcol_v2, decode_orsxcol_v2_into, decode_orsxcol_v2_into_with_workspace,
    decode_orsxcol_v2_with_workspace,
    encode_orsxcol_v2_into, encode_orsxcol_v2_into_with_workspace, FixedEncodingId,
    OrsxcolV2DecodeWorkspace, OrsxcolV2EncodeWorkspace,
};
pub use types::{
    ColumnarAutoConfig, ColumnarBatch, ColumnarBatchReader, ColumnarField, ColumnarReadConfig,
    ColumnarReaderMode, ColumnarSchema, ColumnarType, CopyBinaryBatchReader,
    CopyBinaryBatchReaderConfig, RowWiseBatchReader, RowWiseBatchReaderConfig,
};

pub trait OrsxColumnar {
    fn columnar_schema() -> crate::Result<ColumnarSchema>;
}
