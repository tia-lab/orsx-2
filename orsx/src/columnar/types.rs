use crate::{Error, Result};
use bytes::Bytes;
use futures_util::stream::BoxStream;
use futures_util::StreamExt;
use futures_util::TryStreamExt;
use sqlx::Column;
use sqlx::postgres::PgConnection;
use sqlx::postgres::PgRow;
use sqlx::Row;
// NOTE: We intentionally use a single contiguous buffer for COPY BINARY parsing to
// minimize per-field overhead on wide tables.

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ColumnarType {
    Bool,
    I16,
    I32,
    I64,
    F32,
    F64,
    Uuid,
    TimestampTzMicros,
    Utf8,
    Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnarField {
    pub name: Option<String>,
    pub ty: ColumnarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnarSchema {
    fields: Vec<ColumnarField>,
}

impl ColumnarSchema {
    pub fn new(fields: Vec<ColumnarField>) -> Result<Self> {
        if fields.is_empty() {
            return Err(Error::Other("columnar schema must have at least one field".to_string()));
        }
        Ok(Self { fields })
    }

    pub fn fields(&self) -> &[ColumnarField] {
        &self.fields
    }

    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct ColumnarReadConfig {
    pub validate_utf8: bool,
    pub var_inline_limit: usize,
}

impl Default for ColumnarReadConfig {
    fn default() -> Self {
        Self {
            validate_utf8: false,
            var_inline_limit: 64 * 1024,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CopyBinaryBatchReaderConfig {
    pub max_header_extension_bytes: usize,
    pub max_field_bytes: usize,
}

impl Default for CopyBinaryBatchReaderConfig {
    fn default() -> Self {
        Self {
            max_header_extension_bytes: 1024 * 1024,
            max_field_bytes: 256 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ValidityBitmap {
    pub(crate) bytes: Vec<u8>,
}

impl ValidityBitmap {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn prepare(&mut self, row_capacity: usize) -> Result<()> {
        let byte_len = row_capacity
            .checked_add(7)
            .ok_or_else(|| Error::Other("row capacity overflow".to_string()))?
            / 8;
        self.bytes.clear();
        self.bytes.resize(byte_len, 0);
        Ok(())
    }

    fn set(&mut self, row_idx: usize, is_valid: bool) -> Result<()> {
        let byte_idx = row_idx / 8;
        let bit_idx = row_idx % 8;
        if byte_idx >= self.bytes.len() {
            return Err(Error::Other("validity bitmap out of bounds".to_string()));
        }
        let mask = 1u8 << bit_idx;
        if is_valid {
            self.bytes[byte_idx] |= mask;
        } else {
            self.bytes[byte_idx] &= !mask;
        }
        Ok(())
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ColumnData {
    FixedBool {
        validity: ValidityBitmap,
        values: Vec<u8>,
    },
    FixedI16 {
        validity: ValidityBitmap,
        values: Vec<i16>,
    },
    FixedI32 {
        validity: ValidityBitmap,
        values: Vec<i32>,
    },
    FixedI64 {
        validity: ValidityBitmap,
        values: Vec<i64>,
    },
    FixedF32Bits {
        validity: ValidityBitmap,
        values: Vec<u32>,
    },
    FixedF64Bits {
        validity: ValidityBitmap,
        values: Vec<u64>,
    },
    FixedUuid {
        validity: ValidityBitmap,
        values: Vec<[u8; 16]>,
    },
    FixedTimestampMicros {
        validity: ValidityBitmap,
        values: Vec<i64>,
    },
    Var {
        ty: ColumnarType,
        validity: ValidityBitmap,
        offsets: Vec<u32>,
        data: Vec<u8>,
        chunks: Vec<Bytes>,
        total_len: u32,
    },
}

impl ColumnData {
    pub(crate) fn new(ty: ColumnarType) -> Result<Self> {
        match ty {
            ColumnarType::Utf8 | ColumnarType::Bytes => Ok(ColumnData::Var {
                ty,
                validity: ValidityBitmap::new(),
                offsets: vec![0],
                data: Vec::new(),
                chunks: Vec::new(),
                total_len: 0,
            }),
            ColumnarType::Bool => Ok(ColumnData::FixedBool {
                validity: ValidityBitmap::new(),
                values: Vec::new(),
            }),
            ColumnarType::I16 => Ok(ColumnData::FixedI16 {
                validity: ValidityBitmap::new(),
                values: Vec::new(),
            }),
            ColumnarType::I32 => Ok(ColumnData::FixedI32 {
                validity: ValidityBitmap::new(),
                values: Vec::new(),
            }),
            ColumnarType::I64 => Ok(ColumnData::FixedI64 {
                validity: ValidityBitmap::new(),
                values: Vec::new(),
            }),
            ColumnarType::F32 => Ok(ColumnData::FixedF32Bits {
                validity: ValidityBitmap::new(),
                values: Vec::new(),
            }),
            ColumnarType::F64 => Ok(ColumnData::FixedF64Bits {
                validity: ValidityBitmap::new(),
                values: Vec::new(),
            }),
            ColumnarType::Uuid => Ok(ColumnData::FixedUuid {
                validity: ValidityBitmap::new(),
                values: Vec::new(),
            }),
            ColumnarType::TimestampTzMicros => Ok(ColumnData::FixedTimestampMicros {
                validity: ValidityBitmap::new(),
                values: Vec::new(),
            }),
        }
    }

    fn ty(&self) -> ColumnarType {
        match self {
            ColumnData::FixedBool { .. } => ColumnarType::Bool,
            ColumnData::FixedI16 { .. } => ColumnarType::I16,
            ColumnData::FixedI32 { .. } => ColumnarType::I32,
            ColumnData::FixedI64 { .. } => ColumnarType::I64,
            ColumnData::FixedF32Bits { .. } => ColumnarType::F32,
            ColumnData::FixedF64Bits { .. } => ColumnarType::F64,
            ColumnData::FixedUuid { .. } => ColumnarType::Uuid,
            ColumnData::FixedTimestampMicros { .. } => ColumnarType::TimestampTzMicros,
            ColumnData::Var { ty, .. } => *ty,
        }
    }

    fn prepare(&mut self, row_capacity: usize) -> Result<()> {
        match self {
            ColumnData::FixedBool { validity, values } => {
                validity.prepare(row_capacity)?;
                values.clear();
                values.reserve(row_capacity);
            }
            ColumnData::FixedI16 { validity, values } => {
                validity.prepare(row_capacity)?;
                values.clear();
                values.reserve(row_capacity);
            }
            ColumnData::FixedI32 { validity, values } => {
                validity.prepare(row_capacity)?;
                values.clear();
                values.reserve(row_capacity);
            }
            ColumnData::FixedI64 { validity, values } => {
                validity.prepare(row_capacity)?;
                values.clear();
                values.reserve(row_capacity);
            }
            ColumnData::FixedF32Bits { validity, values } => {
                validity.prepare(row_capacity)?;
                values.clear();
                values.reserve(row_capacity);
            }
            ColumnData::FixedF64Bits { validity, values } => {
                validity.prepare(row_capacity)?;
                values.clear();
                values.reserve(row_capacity);
            }
            ColumnData::FixedUuid { validity, values } => {
                validity.prepare(row_capacity)?;
                values.clear();
                values.reserve(row_capacity);
            }
            ColumnData::FixedTimestampMicros { validity, values } => {
                validity.prepare(row_capacity)?;
                values.clear();
                values.reserve(row_capacity);
            }
            ColumnData::Var {
                validity,
                offsets,
                data,
                chunks,
                total_len,
                ..
            } => {
                validity.prepare(row_capacity)?;
                offsets.clear();
                offsets.push(0);
                offsets.reserve(
                    row_capacity
                        .checked_add(1)
                        .ok_or_else(|| Error::Other("var column capacity overflow".to_string()))?,
                );
                data.clear();
                chunks.clear();
                *total_len = 0;
            }
        }
        Ok(())
    }

    fn set_validity(&mut self, row_idx: usize, is_valid: bool) -> Result<()> {
        match self {
            ColumnData::FixedBool { validity, .. } => validity.set(row_idx, is_valid),
            ColumnData::FixedI16 { validity, .. } => validity.set(row_idx, is_valid),
            ColumnData::FixedI32 { validity, .. } => validity.set(row_idx, is_valid),
            ColumnData::FixedI64 { validity, .. } => validity.set(row_idx, is_valid),
            ColumnData::FixedF32Bits { validity, .. } => validity.set(row_idx, is_valid),
            ColumnData::FixedF64Bits { validity, .. } => validity.set(row_idx, is_valid),
            ColumnData::FixedUuid { validity, .. } => validity.set(row_idx, is_valid),
            ColumnData::FixedTimestampMicros { validity, .. } => validity.set(row_idx, is_valid),
            ColumnData::Var { validity, .. } => validity.set(row_idx, is_valid),
        }
    }

    fn validity_bytes(&self) -> &[u8] {
        match self {
            ColumnData::FixedBool { validity, .. } => validity.as_bytes(),
            ColumnData::FixedI16 { validity, .. } => validity.as_bytes(),
            ColumnData::FixedI32 { validity, .. } => validity.as_bytes(),
            ColumnData::FixedI64 { validity, .. } => validity.as_bytes(),
            ColumnData::FixedF32Bits { validity, .. } => validity.as_bytes(),
            ColumnData::FixedF64Bits { validity, .. } => validity.as_bytes(),
            ColumnData::FixedUuid { validity, .. } => validity.as_bytes(),
            ColumnData::FixedTimestampMicros { validity, .. } => validity.as_bytes(),
            ColumnData::Var { validity, .. } => validity.as_bytes(),
        }
    }

    fn fixed_values_len(&self) -> Option<usize> {
        match self {
            ColumnData::FixedBool { values, .. } => Some(values.len()),
            ColumnData::FixedI16 { values, .. } => Some(values.len()),
            ColumnData::FixedI32 { values, .. } => Some(values.len()),
            ColumnData::FixedI64 { values, .. } => Some(values.len()),
            ColumnData::FixedF32Bits { values, .. } => Some(values.len()),
            ColumnData::FixedF64Bits { values, .. } => Some(values.len()),
            ColumnData::FixedUuid { values, .. } => Some(values.len()),
            ColumnData::FixedTimestampMicros { values, .. } => Some(values.len()),
            ColumnData::Var { .. } => None,
        }
    }

    fn var_slices(&self) -> Option<(&[u32], &[u8])> {
        match self {
            // Var columns are chunked; use `var_chunks` and coalesce if required.
            ColumnData::Var { .. } => None,
            _ => None,
        }
    }

    fn var_chunks(&self) -> Option<(&[u32], &[Bytes], u32)> {
        match self {
            ColumnData::Var {
                offsets,
                chunks,
                total_len,
                ..
            } => Some((offsets.as_slice(), chunks.as_slice(), *total_len)),
            _ => None,
        }
    }

    fn var_inline_bytes(&self) -> Option<&[u8]> {
        match self {
            ColumnData::Var { data, .. } => Some(data.as_slice()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColumnarBatch {
    pub(crate) schema: ColumnarSchema,
    pub(crate) row_capacity: usize,
    pub(crate) row_count: usize,
    pub(crate) columns: Vec<ColumnData>,
}

impl ColumnarBatch {
    pub fn new(schema: ColumnarSchema, row_capacity: usize) -> Result<Self> {
        if row_capacity == 0 {
            return Err(Error::Other(
                "columnar batch row capacity must be > 0".to_string(),
            ));
        }
        let mut columns = Vec::with_capacity(schema.len());
        for f in schema.fields() {
            columns.push(ColumnData::new(f.ty)?);
        }
        let mut batch = Self {
            schema,
            row_capacity,
            row_count: 0,
            columns,
        };
        batch.prepare(row_capacity)?;
        Ok(batch)
    }

    pub fn schema(&self) -> &ColumnarSchema {
        &self.schema
    }

    pub fn row_count(&self) -> usize {
        self.row_count
    }

    pub fn row_capacity(&self) -> usize {
        self.row_capacity
    }

    pub fn column_type(&self, idx: usize) -> Option<ColumnarType> {
        self.columns.get(idx).map(|c| c.ty())
    }

    pub fn column_validity_bytes(&self, idx: usize) -> Option<&[u8]> {
        self.columns.get(idx).map(|c| c.validity_bytes())
    }

    pub fn fixed_values_bytes(&self, idx: usize) -> Option<&[u8]> {
        // This crate currently exposes typed accessors for fixed-width columns.
        // Keep `fixed_values_bytes` only for compatibility with earlier code paths,
        // by returning None.
        let _ = idx;
        None
    }

    pub fn fixed_values_len(&self, idx: usize) -> Option<usize> {
        self.columns.get(idx).and_then(|c| c.fixed_values_len())
    }

    pub fn fixed_bool_bytes(&self, idx: usize) -> Option<&[u8]> {
        match self.columns.get(idx)? {
            ColumnData::FixedBool { values, .. } => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn fixed_i16(&self, idx: usize) -> Option<&[i16]> {
        match self.columns.get(idx)? {
            ColumnData::FixedI16 { values, .. } => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn fixed_i32(&self, idx: usize) -> Option<&[i32]> {
        match self.columns.get(idx)? {
            ColumnData::FixedI32 { values, .. } => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn fixed_i64(&self, idx: usize) -> Option<&[i64]> {
        match self.columns.get(idx)? {
            ColumnData::FixedI64 { values, .. } => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn fixed_f32_bits(&self, idx: usize) -> Option<&[u32]> {
        match self.columns.get(idx)? {
            ColumnData::FixedF32Bits { values, .. } => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn fixed_f64_bits(&self, idx: usize) -> Option<&[u64]> {
        match self.columns.get(idx)? {
            ColumnData::FixedF64Bits { values, .. } => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn fixed_uuid_bytes(&self, idx: usize) -> Option<&[[u8; 16]]> {
        match self.columns.get(idx)? {
            ColumnData::FixedUuid { values, .. } => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn fixed_timestamp_micros(&self, idx: usize) -> Option<&[i64]> {
        match self.columns.get(idx)? {
            ColumnData::FixedTimestampMicros { values, .. } => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn var_slices(&self, idx: usize) -> Option<(&[u32], &[u8])> {
        self.columns.get(idx).and_then(|c| c.var_slices())
    }

    pub fn var_chunks(&self, idx: usize) -> Option<(&[u32], &[Bytes], u32)> {
        self.columns.get(idx).and_then(|c| c.var_chunks())
    }

    pub fn coalesce_var_into(&self, idx: usize, out: &mut Vec<u8>) -> Result<()> {
        let Some((offsets, chunks, total_len)) = self.var_chunks(idx) else {
            return Err(Error::Other("expected varlen column".to_string()));
        };
        let total_usize: usize = total_len
            .try_into()
            .map_err(|_| Error::Other("var column too large".to_string()))?;
        out.clear();
        out.reserve(total_usize);
        if let Some(inline) = self.columns.get(idx).and_then(|c| c.var_inline_bytes()) {
            out.extend_from_slice(inline);
        }
        for c in chunks {
            out.extend_from_slice(c.as_ref());
        }
        if out.len() != total_usize {
            return Err(Error::Other("var coalesce length mismatch".to_string()));
        }
        // Basic sanity: offsets last matches total.
        let last = *offsets.last().unwrap_or(&0);
        if last != total_len {
            return Err(Error::Other("var coalesce offsets mismatch".to_string()));
        }
        Ok(())
    }

    pub fn var_inline_bytes(&self, idx: usize) -> Option<&[u8]> {
        self.columns.get(idx).and_then(|c| c.var_inline_bytes())
    }

    pub fn prepare(&mut self, row_capacity: usize) -> Result<()> {
        if row_capacity == 0 {
            return Err(Error::Other(
                "columnar batch row capacity must be > 0".to_string(),
            ));
        }
        self.row_capacity = row_capacity;
        self.row_count = 0;
        for c in &mut self.columns {
            c.prepare(row_capacity)?;
        }
        Ok(())
    }

    pub fn current_row_index(&self) -> usize {
        self.row_count
    }

    pub fn push_null(&mut self, col_idx: usize) -> Result<()> {
        let row_idx = self.row_count;
        if row_idx >= self.row_capacity {
            return Err(Error::Other("columnar batch is full".to_string()));
        }
        let col = self
            .columns
            .get_mut(col_idx)
            .ok_or_else(|| Error::Other("column index out of bounds".to_string()))?;

        col.set_validity(row_idx, false)?;
        match col {
            ColumnData::FixedBool { values, .. } => values.push(0),
            ColumnData::FixedI16 { values, .. } => values.push(0),
            ColumnData::FixedI32 { values, .. } => values.push(0),
            ColumnData::FixedI64 { values, .. } => values.push(0),
            ColumnData::FixedF32Bits { values, .. } => values.push(0),
            ColumnData::FixedF64Bits { values, .. } => values.push(0),
            ColumnData::FixedUuid { values, .. } => values.push([0u8; 16]),
            ColumnData::FixedTimestampMicros { values, .. } => values.push(0),
            ColumnData::Var {
                offsets, total_len, ..
            } => {
                offsets.push(*total_len);
            }
        }
        Ok(())
    }

    pub fn push_bool(&mut self, col_idx: usize, v: bool) -> Result<()> {
        let row_idx = self.row_count;
        let col = self
            .columns
            .get_mut(col_idx)
            .ok_or_else(|| Error::Other("column index out of bounds".to_string()))?;
        col.set_validity(row_idx, true)?;
        match col {
            ColumnData::FixedBool { values, .. } => {
                values.push(if v { 1 } else { 0 });
                Ok(())
            }
            _ => Err(Error::Other("expected BOOL column".to_string())),
        }
    }

    pub fn push_i64(&mut self, col_idx: usize, v: i64) -> Result<()> {
        let row_idx = self.row_count;
        let col = self
            .columns
            .get_mut(col_idx)
            .ok_or_else(|| Error::Other("column index out of bounds".to_string()))?;
        col.set_validity(row_idx, true)?;
        match col {
            ColumnData::FixedI64 { values, .. } => {
                values.push(v);
                Ok(())
            }
            _ => Err(Error::Other("expected I64 column".to_string())),
        }
    }

    pub fn push_i16(&mut self, col_idx: usize, v: i16) -> Result<()> {
        let row_idx = self.row_count;
        let col = self
            .columns
            .get_mut(col_idx)
            .ok_or_else(|| Error::Other("column index out of bounds".to_string()))?;
        col.set_validity(row_idx, true)?;
        match col {
            ColumnData::FixedI16 { values, .. } => {
                values.push(v);
                Ok(())
            }
            _ => Err(Error::Other("expected I16 column".to_string())),
        }
    }

    pub fn push_i32(&mut self, col_idx: usize, v: i32) -> Result<()> {
        let row_idx = self.row_count;
        let col = self
            .columns
            .get_mut(col_idx)
            .ok_or_else(|| Error::Other("column index out of bounds".to_string()))?;
        col.set_validity(row_idx, true)?;
        match col {
            ColumnData::FixedI32 { values, .. } => {
                values.push(v);
                Ok(())
            }
            _ => Err(Error::Other("expected I32 column".to_string())),
        }
    }

    pub fn push_f32_bits(&mut self, col_idx: usize, bits: u32) -> Result<()> {
        let row_idx = self.row_count;
        let col = self
            .columns
            .get_mut(col_idx)
            .ok_or_else(|| Error::Other("column index out of bounds".to_string()))?;
        col.set_validity(row_idx, true)?;
        match col {
            ColumnData::FixedF32Bits { values, .. } => {
                values.push(bits);
                Ok(())
            }
            _ => Err(Error::Other("expected F32 column".to_string())),
        }
    }

    pub fn push_f64_bits(&mut self, col_idx: usize, bits: u64) -> Result<()> {
        let row_idx = self.row_count;
        let col = self
            .columns
            .get_mut(col_idx)
            .ok_or_else(|| Error::Other("column index out of bounds".to_string()))?;
        col.set_validity(row_idx, true)?;
        match col {
            ColumnData::FixedF64Bits { values, .. } => {
                values.push(bits);
                Ok(())
            }
            _ => Err(Error::Other("expected F64 column".to_string())),
        }
    }

    pub fn push_uuid_bytes(&mut self, col_idx: usize, bytes16: [u8; 16]) -> Result<()> {
        let row_idx = self.row_count;
        let col = self
            .columns
            .get_mut(col_idx)
            .ok_or_else(|| Error::Other("column index out of bounds".to_string()))?;
        col.set_validity(row_idx, true)?;
        match col {
            ColumnData::FixedUuid { values, .. } => {
                values.push(bytes16);
                Ok(())
            }
            _ => Err(Error::Other("expected UUID column".to_string())),
        }
    }

    pub fn push_timestamp_micros(&mut self, col_idx: usize, micros: i64) -> Result<()> {
        let row_idx = self.row_count;
        let col = self
            .columns
            .get_mut(col_idx)
            .ok_or_else(|| Error::Other("column index out of bounds".to_string()))?;
        col.set_validity(row_idx, true)?;
        match col {
            ColumnData::FixedTimestampMicros { values, .. } => {
                values.push(micros);
                Ok(())
            }
            _ => Err(Error::Other("expected TIMESTAMPTZ micros column".to_string())),
        }
    }

    pub fn push_utf8(&mut self, col_idx: usize, s: &str) -> Result<()> {
        self.push_var_bytes(col_idx, s.as_bytes())
    }

    pub fn push_var_bytes(&mut self, col_idx: usize, bytes: &[u8]) -> Result<()> {
        let row_idx = self.row_count;
        if row_idx >= self.row_capacity {
            return Err(Error::Other("columnar batch is full".to_string()));
        }
        let col = self
            .columns
            .get_mut(col_idx)
            .ok_or_else(|| Error::Other("column index out of bounds".to_string()))?;

        col.set_validity(row_idx, true)?;
        match col {
            ColumnData::Var {
                data,
                chunks,
                offsets,
                total_len,
                ..
            } => {
                let add: u32 = bytes
                    .len()
                    .try_into()
                    .map_err(|_| Error::Other("var chunk too large".to_string()))?;
                let new_total = total_len
                    .checked_add(add)
                    .ok_or_else(|| Error::Other("var column too large".to_string()))?;
                // Inline small varlen payloads to avoid `Bytes` churn for tiny strings/bytea.
                const INLINE_LIMIT: u32 = 64 * 1024;
                if chunks.is_empty() && new_total <= INLINE_LIMIT {
                    data.extend_from_slice(bytes);
                } else {
                    if chunks.is_empty() && !data.is_empty() {
                        chunks.push(Bytes::copy_from_slice(data.as_slice()));
                        data.clear();
                    }
                    chunks.push(Bytes::copy_from_slice(bytes));
                }
                *total_len = new_total;
                offsets.push(new_total);
            }
            _ => return Err(Error::Other("expected varlen column".to_string())),
        }
        Ok(())
    }

    pub fn end_row(&mut self) -> Result<()> {
        if self.row_count >= self.row_capacity {
            return Err(Error::Other("columnar batch is full".to_string()));
        }
        self.finish_row()
    }

    fn finish_row(&mut self) -> Result<()> {
        self.row_count = self
            .row_count
            .checked_add(1)
            .ok_or_else(|| Error::Other("row count overflow".to_string()))?;
        Ok(())
    }
}

pub struct CopyBinaryBatchReader<'c> {
    copy_out: BoxStream<'c, std::result::Result<Bytes, sqlx::Error>>,
    schema: ColumnarSchema,
    buf: Vec<u8>,
    pos: usize,
    done: bool,
    header_parsed: bool,
    cfg: CopyBinaryBatchReaderConfig,
    read_cfg: ColumnarReadConfig,
}

impl<'c> CopyBinaryBatchReader<'c> {
    pub async fn new_select_unchecked(
        conn: &'c mut PgConnection,
        select_sql: &str,
        schema: ColumnarSchema,
    ) -> Result<Self> {
        let copy_sql = format!("COPY ({select_sql}) TO STDOUT (FORMAT BINARY)");
        Self::new_copy_sql_unchecked(conn, &copy_sql, schema).await
    }

    pub async fn new_copy_sql_unchecked(
        conn: &'c mut PgConnection,
        copy_sql: &str,
        schema: ColumnarSchema,
    ) -> Result<Self> {
        if schema.is_empty() {
            return Err(Error::Other("schema must not be empty".to_string()));
        }
        let copy_out = conn.copy_out_raw(copy_sql).await?;
        Ok(Self {
            copy_out,
            schema,
            buf: Vec::new(),
            pos: 0,
            done: false,
            header_parsed: false,
            cfg: CopyBinaryBatchReaderConfig::default(),
            read_cfg: ColumnarReadConfig::default(),
        })
    }

    pub fn with_config(mut self, cfg: CopyBinaryBatchReaderConfig) -> Self {
        self.cfg = cfg;
        self
    }

    pub fn with_read_config(mut self, cfg: ColumnarReadConfig) -> Self {
        self.read_cfg = cfg;
        self
    }

    pub fn schema(&self) -> &ColumnarSchema {
        &self.schema
    }

    pub async fn next_batch_into(&mut self, out: &mut ColumnarBatch) -> Result<usize> {
        if &self.schema != out.schema() {
            return Err(Error::Other(
                "output batch schema does not match reader schema".to_string(),
            ));
        }
        if self.done {
            out.prepare(out.row_capacity())?;
            return Ok(0);
        }

        out.prepare(out.row_capacity())?;

        if !self.header_parsed {
            self.parse_header().await?;
            self.header_parsed = true;
        }

        while out.row_count() < out.row_capacity() {
            let field_count = self.read_i16_be().await?;
            if field_count == -1 {
                self.done = true;
                break;
            }
            let field_count_usize: usize = field_count
                .try_into()
                .map_err(|_| Error::Other("negative field count".to_string()))?;
            if field_count_usize != self.schema.len() {
                return Err(Error::Other(format!(
                    "COPY row has {field_count_usize} fields but schema expects {}",
                    self.schema.len()
                )));
            }

            let row_idx = out.row_count();
            for col_idx in 0..field_count_usize {
                self.read_field_into(out, row_idx, col_idx).await?;
            }
            out.finish_row()?;
        }

        Ok(out.row_count())
    }

    async fn parse_header(&mut self) -> Result<()> {
        const SIG: &[u8; 11] = b"PGCOPY\n\xff\r\n\0";
        let mut sig = [0u8; 11];
        self.read_exact_into(&mut sig).await?;
        if sig.as_slice() != SIG {
            return Err(Error::Other("invalid COPY BINARY signature".to_string()));
        }
        let flags = self.read_i32_be().await?;
        if flags != 0 {
            return Err(Error::Other(format!(
                "unsupported COPY BINARY flags: {flags}"
            )));
        }
        let ext_len = self.read_i32_be().await?;
        if ext_len < 0 {
            return Err(Error::Other("invalid COPY header extension length".to_string()));
        }
        let ext_len_usize: usize = ext_len
            .try_into()
            .map_err(|_| Error::Other("invalid COPY header extension length".to_string()))?;
        if ext_len_usize > self.cfg.max_header_extension_bytes {
            return Err(Error::Other("COPY header extension too large".to_string()));
        }
        if ext_len_usize > 0 {
            self.discard(ext_len_usize).await?;
        }
        Ok(())
    }

    async fn read_u8(&mut self) -> Result<u8> {
        self.ensure_available(1).await?;
        let b = self
            .buf
            .get(self.pos)
            .copied()
            .ok_or_else(|| Error::Other("unexpected end of COPY stream".to_string()))?;
        self.pos = self
            .pos
            .checked_add(1)
            .ok_or_else(|| Error::Other("buffer index overflow".to_string()))?;
        Ok(b)
    }

    async fn read_field_into(
        &mut self,
        out: &mut ColumnarBatch,
        row_idx: usize,
        col_idx: usize,
    ) -> Result<()> {
        let len = self.read_i32_be().await?;
        let is_null = len == -1;
        if len < -1 {
            return Err(Error::Other("invalid COPY field length".to_string()));
        }

        let col = out
            .columns
            .get_mut(col_idx)
            .ok_or_else(|| Error::Other("column index out of bounds".to_string()))?;

        if is_null {
            col.set_validity(row_idx, false)?;
            match col {
                ColumnData::FixedBool { values, .. } => values.push(0),
                ColumnData::FixedI16 { values, .. } => values.push(0),
                ColumnData::FixedI32 { values, .. } => values.push(0),
                ColumnData::FixedI64 { values, .. } => values.push(0),
                ColumnData::FixedF32Bits { values, .. } => values.push(0),
                ColumnData::FixedF64Bits { values, .. } => values.push(0),
                ColumnData::FixedUuid { values, .. } => values.push([0u8; 16]),
                ColumnData::FixedTimestampMicros { values, .. } => values.push(0),
                ColumnData::Var {
                    offsets, total_len, ..
                } => {
                    offsets.push(*total_len);
                }
            }
            return Ok(());
        }

        let len_usize: usize = len
            .try_into()
            .map_err(|_| Error::Other("invalid COPY field length".to_string()))?;
        if len_usize > self.cfg.max_field_bytes {
            return Err(Error::Other("COPY field too large".to_string()));
        }

        col.set_validity(row_idx, true)?;
        match col {
            ColumnData::FixedBool { values, .. } => {
                if len_usize != 1 {
                    return Err(Error::Other("COPY field length mismatch for Bool".to_string()));
                }
                let b = self.read_u8().await?;
                values.push(b);
            }
            ColumnData::FixedI16 { values, .. } => {
                if len_usize != 2 {
                    return Err(Error::Other("COPY field length mismatch for I16".to_string()));
                }
                values.push(self.read_i16_be().await?);
            }
            ColumnData::FixedI32 { values, .. } => {
                if len_usize != 4 {
                    return Err(Error::Other("COPY field length mismatch for I32".to_string()));
                }
                values.push(self.read_i32_be().await?);
            }
            ColumnData::FixedI64 { values, .. } => {
                if len_usize != 8 {
                    return Err(Error::Other("COPY field length mismatch for I64".to_string()));
                }
                values.push(self.read_i64_be().await?);
            }
            ColumnData::FixedF32Bits { values, .. } => {
                if len_usize != 4 {
                    return Err(Error::Other("COPY field length mismatch for F32".to_string()));
                }
                values.push(self.read_u32_be().await?);
            }
            ColumnData::FixedF64Bits { values, .. } => {
                if len_usize != 8 {
                    return Err(Error::Other("COPY field length mismatch for F64".to_string()));
                }
                values.push(self.read_u64_be().await?);
            }
            ColumnData::FixedUuid { values, .. } => {
                if len_usize != 16 {
                    return Err(Error::Other("COPY field length mismatch for UUID".to_string()));
                }
                let mut v = [0u8; 16];
                self.read_exact_into(&mut v).await?;
                values.push(v);
            }
            ColumnData::FixedTimestampMicros { values, .. } => {
                if len_usize != 8 {
                    return Err(Error::Other(
                        "COPY field length mismatch for TimestampTz".to_string(),
                    ));
                }
                let pg_micros = self.read_i64_be().await?;
                if pg_micros == i64::MIN || pg_micros == i64::MAX {
                    return Err(Error::Other(
                        "timestamptz infinity is not supported".to_string(),
                    ));
                }
                const UNIX_TO_PG_EPOCH_MICROS: i64 = 946_684_800_000_000;
                let unix_micros = pg_micros
                    .checked_add(UNIX_TO_PG_EPOCH_MICROS)
                    .ok_or_else(|| Error::Other("timestamp overflow".to_string()))?;
                values.push(unix_micros);
            }
            ColumnData::Var {
                ty,
                data,
                offsets,
                total_len,
                ..
            } => {
                let validate_utf8 = self.read_cfg.validate_utf8;
                self.ensure_available(len_usize).await?;
                let start = self.pos;
                let end = start
                    .checked_add(len_usize)
                    .ok_or_else(|| Error::Other("buffer index overflow".to_string()))?;
                let bytes = self
                    .buf
                    .get(start..end)
                    .ok_or_else(|| Error::Other("unexpected end of COPY stream".to_string()))?;
                self.pos = end;
                match ty {
                    ColumnarType::Utf8 => {
                        if validate_utf8 && std::str::from_utf8(bytes).is_err() {
                            return Err(Error::Other("invalid UTF-8 in TEXT column".to_string()));
                        }
                    }
                    ColumnarType::Bytes => {
                    }
                    _ => return Err(Error::Other("internal type mismatch".to_string())),
                }
                let add: u32 = bytes
                    .len()
                    .try_into()
                    .map_err(|_| Error::Other("var chunk too large".to_string()))?;
                let new_total = total_len
                    .checked_add(add)
                    .ok_or_else(|| Error::Other("var column too large".to_string()))?;
                data.extend_from_slice(bytes);
                *total_len = new_total;
                offsets.push(new_total);
            }
        }

        Ok(())
    }

    async fn ensure_available(&mut self, needed: usize) -> Result<()> {
        if needed == 0 {
            return Ok(());
        }

        self.compact_if_needed();

        while self.buf.len().saturating_sub(self.pos) < needed {
            match self.copy_out.try_next().await {
                Ok(Some(chunk)) => self.buf.extend_from_slice(chunk.as_ref()),
                Ok(None) => break,
                Err(e) => return Err(Error::Database(e)),
            }
        }

        if self.buf.len().saturating_sub(self.pos) < needed {
            return Err(Error::Other("unexpected end of COPY stream".to_string()));
        }

        Ok(())
    }

    async fn discard(&mut self, n: usize) -> Result<()> {
        if n == 0 {
            return Ok(());
        }
        self.ensure_available(n).await?;
        self.pos = self
            .pos
            .checked_add(n)
            .ok_or_else(|| Error::Other("buffer index overflow".to_string()))?;
        Ok(())
    }

    async fn read_i16_be(&mut self) -> Result<i16> {
        self.ensure_available(2).await?;
        let start = self.pos;
        let end = start
            .checked_add(2)
            .ok_or_else(|| Error::Other("buffer index overflow".to_string()))?;
        let s = self
            .buf
            .get(start..end)
            .ok_or_else(|| Error::Other("unexpected end of COPY stream".to_string()))?;
        self.pos = end;
        Ok(i16::from_be_bytes([s[0], s[1]]))
    }

    async fn read_i32_be(&mut self) -> Result<i32> {
        self.ensure_available(4).await?;
        let start = self.pos;
        let end = start
            .checked_add(4)
            .ok_or_else(|| Error::Other("buffer index overflow".to_string()))?;
        let s = self
            .buf
            .get(start..end)
            .ok_or_else(|| Error::Other("unexpected end of COPY stream".to_string()))?;
        self.pos = end;
        Ok(i32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }

    async fn read_i64_be(&mut self) -> Result<i64> {
        self.ensure_available(8).await?;
        let start = self.pos;
        let end = start
            .checked_add(8)
            .ok_or_else(|| Error::Other("buffer index overflow".to_string()))?;
        let s = self
            .buf
            .get(start..end)
            .ok_or_else(|| Error::Other("unexpected end of COPY stream".to_string()))?;
        self.pos = end;
        Ok(i64::from_be_bytes([
            s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
        ]))
    }

    async fn read_u32_be(&mut self) -> Result<u32> {
        self.ensure_available(4).await?;
        let start = self.pos;
        let end = start
            .checked_add(4)
            .ok_or_else(|| Error::Other("buffer index overflow".to_string()))?;
        let s = self
            .buf
            .get(start..end)
            .ok_or_else(|| Error::Other("unexpected end of COPY stream".to_string()))?;
        self.pos = end;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }

    async fn read_u64_be(&mut self) -> Result<u64> {
        self.ensure_available(8).await?;
        let start = self.pos;
        let end = start
            .checked_add(8)
            .ok_or_else(|| Error::Other("buffer index overflow".to_string()))?;
        let s = self
            .buf
            .get(start..end)
            .ok_or_else(|| Error::Other("unexpected end of COPY stream".to_string()))?;
        self.pos = end;
        Ok(u64::from_be_bytes([
            s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
        ]))
    }

    async fn read_exact_into(&mut self, out: &mut [u8]) -> Result<()> {
        let n = out.len();
        if n == 0 {
            return Ok(());
        }
        self.ensure_available(n).await?;
        let start = self.pos;
        let end = start
            .checked_add(n)
            .ok_or_else(|| Error::Other("buffer index overflow".to_string()))?;
        let s = self
            .buf
            .get(start..end)
            .ok_or_else(|| Error::Other("unexpected end of COPY stream".to_string()))?;
        out.copy_from_slice(s);
        self.pos = end;
        Ok(())
    }

    fn compact_if_needed(&mut self) {
        if self.pos == 0 {
            return;
        }
        const MIN_COMPACT: usize = 1024 * 1024;
        if self.pos < MIN_COMPACT && self.pos * 2 < self.buf.len() {
            return;
        }
        let remaining = self.buf.len().saturating_sub(self.pos);
        self.buf.copy_within(self.pos.., 0);
        self.buf.truncate(remaining);
        self.pos = 0;
    }
}

pub struct RowWiseBatchReader<'c> {
    rows: BoxStream<'c, std::result::Result<PgRow, sqlx::Error>>,
    schema: ColumnarSchema,
    cfg: RowWiseBatchReaderConfig,
    preflight_done: bool,
    done: bool,
}

#[derive(Debug, Clone)]
pub struct RowWiseBatchReaderConfig {
    pub validate_column_count: bool,
    pub validate_column_names: bool,
}

impl Default for RowWiseBatchReaderConfig {
    fn default() -> Self {
        Self {
            validate_column_count: false,
            validate_column_names: false,
        }
    }
}

impl<'c> RowWiseBatchReader<'c> {
    pub async fn new_select_unchecked(
        conn: &'c mut PgConnection,
        select_sql: &'c str,
        schema: ColumnarSchema,
    ) -> Result<Self> {
        if schema.is_empty() {
            return Err(Error::Other("schema must not be empty".to_string()));
        }
        let rows = sqlx::query(select_sql).fetch(conn).boxed();
        Ok(Self {
            rows,
            schema,
            cfg: RowWiseBatchReaderConfig::default(),
            preflight_done: false,
            done: false,
        })
    }

    pub fn with_config(mut self, cfg: RowWiseBatchReaderConfig) -> Self {
        self.cfg = cfg;
        self
    }

    pub fn schema(&self) -> &ColumnarSchema {
        &self.schema
    }

    pub async fn next_batch_into(&mut self, out: &mut ColumnarBatch) -> Result<usize> {
        if &self.schema != out.schema() {
            return Err(Error::Other(
                "output batch schema does not match reader schema".to_string(),
            ));
        }
        if self.done {
            out.prepare(out.row_capacity())?;
            return Ok(0);
        }

        out.prepare(out.row_capacity())?;

        while out.row_count() < out.row_capacity() {
            let row = match self.rows.try_next().await {
                Ok(Some(r)) => r,
                Ok(None) => {
                    self.done = true;
                    break;
                }
                Err(e) => return Err(Error::Database(e)),
            };

            if !self.preflight_done {
                self.run_preflight(&row)?;
                self.preflight_done = true;
            }

            for (col_idx, f) in self.schema.fields().iter().enumerate() {
                match f.ty {
                    ColumnarType::Bool => {
                        let v: Option<bool> = row.try_get(col_idx).map_err(Error::Database)?;
                        match v {
                            Some(v) => out.push_bool(col_idx, v)?,
                            None => out.push_null(col_idx)?,
                        }
                    }
                    ColumnarType::I16 => {
                        let v: Option<i16> = row.try_get(col_idx).map_err(Error::Database)?;
                        match v {
                            Some(v) => out.push_i16(col_idx, v)?,
                            None => out.push_null(col_idx)?,
                        }
                    }
                    ColumnarType::I32 => {
                        let v: Option<i32> = row.try_get(col_idx).map_err(Error::Database)?;
                        match v {
                            Some(v) => out.push_i32(col_idx, v)?,
                            None => out.push_null(col_idx)?,
                        }
                    }
                    ColumnarType::I64 => {
                        let v: Option<i64> = row.try_get(col_idx).map_err(Error::Database)?;
                        match v {
                            Some(v) => out.push_i64(col_idx, v)?,
                            None => out.push_null(col_idx)?,
                        }
                    }
                    ColumnarType::F32 => {
                        let v: Option<f32> = row.try_get(col_idx).map_err(Error::Database)?;
                        match v {
                            Some(v) => {
                                out.push_f32_bits(col_idx, v.to_bits())?;
                            }
                            None => out.push_null(col_idx)?,
                        }
                    }
                    ColumnarType::F64 => {
                        let v: Option<f64> = row.try_get(col_idx).map_err(Error::Database)?;
                        match v {
                            Some(v) => out.push_f64_bits(col_idx, v.to_bits())?,
                            None => out.push_null(col_idx)?,
                        }
                    }
                    ColumnarType::Uuid => {
                        let v: Option<sqlx::types::Uuid> =
                            row.try_get(col_idx).map_err(Error::Database)?;
                        match v {
                            Some(v) => out.push_uuid_bytes(col_idx, *v.as_bytes())?,
                            None => out.push_null(col_idx)?,
                        }
                    }
                    ColumnarType::TimestampTzMicros => {
                        let v: Option<crate::SqlxTimestamp> =
                            row.try_get(col_idx).map_err(Error::Database)?;
                        match v {
                            Some(v) => out.push_timestamp_micros(
                                col_idx,
                                v.to_jiff().as_microsecond(),
                            )?,
                            None => out.push_null(col_idx)?,
                        }
                    }
                    ColumnarType::Utf8 => {
                        let v: Option<&str> = row.try_get(col_idx).map_err(Error::Database)?;
                        match v {
                            Some(v) => out.push_utf8(col_idx, v)?,
                            None => out.push_null(col_idx)?,
                        }
                    }
                    ColumnarType::Bytes => {
                        let v: Option<&[u8]> = row.try_get(col_idx).map_err(Error::Database)?;
                        match v {
                            Some(v) => out.push_var_bytes(col_idx, v)?,
                            None => out.push_null(col_idx)?,
                        }
                    }
                }
            }

            out.end_row()?;
        }

        Ok(out.row_count())
    }

    fn run_preflight(&self, row: &PgRow) -> Result<()> {
        if !self.cfg.validate_column_count && !self.cfg.validate_column_names {
            return Ok(());
        }

        let got_cols = row.columns().len();
        let exp_cols = self.schema.len();

        if self.cfg.validate_column_count && got_cols != exp_cols {
            return Err(Error::Other(format!(
                "row-wise preflight failed: column count mismatch (expected {exp_cols}, got {got_cols})"
            )));
        }

        if self.cfg.validate_column_names {
            let cols = row.columns();
            let n = exp_cols.min(cols.len());
            for i in 0..n {
                let expected = self.schema.fields()[i].name.as_deref();
                if let Some(expected) = expected {
                    let got = cols[i].name();
                    if got != expected {
                        return Err(Error::Other(format!(
                            "row-wise preflight failed: column name mismatch at index {i} (expected `{expected}`, got `{got}`)"
                        )));
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ColumnarAutoConfig {
    pub expected_rows: Option<usize>,
    pub cols_force_copy_min: usize,
    pub cols_force_row_wise_max: usize,
    pub rows_force_row_wise_min: usize,
}

impl Default for ColumnarAutoConfig {
    fn default() -> Self {
        Self {
            expected_rows: None,
            cols_force_copy_min: 128,
            cols_force_row_wise_max: 64,
            rows_force_row_wise_min: 500_000,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ColumnarReaderMode {
    CopyBinary,
    RowWise,
    Auto(ColumnarAutoConfig),
}

impl Default for ColumnarReaderMode {
    fn default() -> Self {
        Self::Auto(ColumnarAutoConfig::default())
    }
}

pub enum ColumnarBatchReader<'c> {
    Copy(CopyBinaryBatchReader<'c>),
    RowWise(RowWiseBatchReader<'c>),
}

impl<'c> ColumnarBatchReader<'c> {
    pub async fn new_select_unchecked(
        conn: &'c mut PgConnection,
        select_sql: &'c str,
        schema: ColumnarSchema,
        mode: ColumnarReaderMode,
    ) -> Result<Self> {
        enum Chosen {
            CopyBinary,
            RowWise,
        }
        let chosen = match mode {
            ColumnarReaderMode::CopyBinary => Chosen::CopyBinary,
            ColumnarReaderMode::RowWise => Chosen::RowWise,
            ColumnarReaderMode::Auto(cfg) => {
                let cols = schema.len();
                let expected_rows = cfg.expected_rows.unwrap_or(0);
                if cols >= cfg.cols_force_copy_min {
                    Chosen::CopyBinary
                } else if expected_rows >= cfg.rows_force_row_wise_min
                    && cols <= cfg.cols_force_row_wise_max
                {
                    Chosen::RowWise
                } else {
                    Chosen::CopyBinary
                }
            }
        };

        match chosen {
            Chosen::CopyBinary => Ok(Self::Copy(
                CopyBinaryBatchReader::new_select_unchecked(conn, select_sql, schema).await?,
            )),
            Chosen::RowWise => Ok(Self::RowWise(
                RowWiseBatchReader::new_select_unchecked(conn, select_sql, schema).await?,
            )),
        }
    }

    pub fn schema(&self) -> &ColumnarSchema {
        match self {
            ColumnarBatchReader::Copy(r) => r.schema(),
            ColumnarBatchReader::RowWise(r) => r.schema(),
        }
    }

    pub async fn next_batch_into(&mut self, out: &mut ColumnarBatch) -> Result<usize> {
        match self {
            ColumnarBatchReader::Copy(r) => r.next_batch_into(out).await,
            ColumnarBatchReader::RowWise(r) => r.next_batch_into(out).await,
        }
    }
}
