use crate::{Error, Result};
use bytes::Bytes;
use futures_util::stream::BoxStream;
use futures_util::TryStreamExt;
use sqlx::postgres::PgConnection;

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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FixedEncoding {
    Le,
    PgBe,
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

#[derive(Debug, Default, Clone)]
pub struct ColumnarReadConfig {
    pub validate_utf8: bool,
}

#[derive(Debug, Clone)]
pub struct CopyBinaryBatchReaderConfig {
    pub compact_after_bytes: usize,
    pub max_header_extension_bytes: usize,
    pub max_field_bytes: usize,
}

impl Default for CopyBinaryBatchReaderConfig {
    fn default() -> Self {
        Self {
            compact_after_bytes: 64 * 1024,
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
    Fixed {
        ty: ColumnarType,
        encoding: FixedEncoding,
        width: usize,
        validity: ValidityBitmap,
        values: Vec<u8>,
    },
    Var {
        ty: ColumnarType,
        validity: ValidityBitmap,
        offsets: Vec<u32>,
        data: Vec<u8>,
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
            }),
            ColumnarType::Bool => Ok(ColumnData::Fixed {
                ty,
                encoding: FixedEncoding::Le,
                width: 1,
                validity: ValidityBitmap::new(),
                values: Vec::new(),
            }),
            ColumnarType::I16 => Ok(ColumnData::Fixed {
                ty,
                encoding: FixedEncoding::Le,
                width: 2,
                validity: ValidityBitmap::new(),
                values: Vec::new(),
            }),
            ColumnarType::I32 | ColumnarType::F32 => Ok(ColumnData::Fixed {
                ty,
                encoding: FixedEncoding::Le,
                width: 4,
                validity: ValidityBitmap::new(),
                values: Vec::new(),
            }),
            ColumnarType::I64 | ColumnarType::F64 | ColumnarType::TimestampTzMicros => Ok(ColumnData::Fixed {
                ty,
                encoding: FixedEncoding::Le,
                width: 8,
                validity: ValidityBitmap::new(),
                values: Vec::new(),
            }),
            ColumnarType::Uuid => Ok(ColumnData::Fixed {
                ty,
                encoding: FixedEncoding::Le,
                width: 16,
                validity: ValidityBitmap::new(),
                values: Vec::new(),
            }),
        }
    }

    fn ty(&self) -> ColumnarType {
        match self {
            ColumnData::Fixed { ty, .. } => *ty,
            ColumnData::Var { ty, .. } => *ty,
        }
    }

    fn fixed_encoding(&self) -> Option<FixedEncoding> {
        match self {
            ColumnData::Fixed { encoding, .. } => Some(*encoding),
            ColumnData::Var { .. } => None,
        }
    }

    fn prepare(&mut self, row_capacity: usize) -> Result<()> {
        match self {
            ColumnData::Fixed {
                width,
                validity,
                values,
                ..
            } => {
                validity.prepare(row_capacity)?;
                values.clear();
                let cap = row_capacity
                    .checked_mul(*width)
                    .ok_or_else(|| Error::Other("fixed column capacity overflow".to_string()))?;
                values.reserve(cap);
            }
            ColumnData::Var {
                validity,
                offsets,
                data,
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
            }
        }
        Ok(())
    }

    fn set_validity(&mut self, row_idx: usize, is_valid: bool) -> Result<()> {
        match self {
            ColumnData::Fixed { validity, .. } => validity.set(row_idx, is_valid),
            ColumnData::Var { validity, .. } => validity.set(row_idx, is_valid),
        }
    }

    fn validity_bytes(&self) -> &[u8] {
        match self {
            ColumnData::Fixed { validity, .. } => validity.as_bytes(),
            ColumnData::Var { validity, .. } => validity.as_bytes(),
        }
    }

    fn fixed_values_bytes(&self) -> Option<&[u8]> {
        match self {
            ColumnData::Fixed { values, .. } => Some(values.as_slice()),
            ColumnData::Var { .. } => None,
        }
    }

    fn var_slices(&self) -> Option<(&[u32], &[u8])> {
        match self {
            ColumnData::Var { offsets, data, .. } => Some((offsets.as_slice(), data.as_slice())),
            ColumnData::Fixed { .. } => None,
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
        self.columns.get(idx).and_then(|c| c.fixed_values_bytes())
    }

    pub fn fixed_encoding(&self, idx: usize) -> Option<FixedEncoding> {
        self.columns.get(idx).and_then(|c| c.fixed_encoding())
    }

    pub fn var_slices(&self, idx: usize) -> Option<(&[u32], &[u8])> {
        self.columns.get(idx).and_then(|c| c.var_slices())
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
            ColumnData::Fixed { width, values, .. } => {
                let start = values.len();
                let new_len = start
                    .checked_add(*width)
                    .ok_or_else(|| Error::Other("fixed column size overflow".to_string()))?;
                values.resize(new_len, 0);
            }
            ColumnData::Var { offsets, data, .. } => {
                let len_u32: u32 = data
                    .len()
                    .try_into()
                    .map_err(|_| Error::Other("var column too large".to_string()))?;
                offsets.push(len_u32);
            }
        }
        Ok(())
    }

    pub fn push_fixed_bytes(&mut self, col_idx: usize, bytes: &[u8], encoding: FixedEncoding) -> Result<()> {
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
            ColumnData::Fixed {
                width,
                encoding: enc,
                values,
                ..
            } => {
                if bytes.len() != *width {
                    return Err(Error::Other("fixed-width byte length mismatch".to_string()));
                }
                *enc = encoding;
                values.extend_from_slice(bytes);
            }
            _ => return Err(Error::Other("expected fixed-width column".to_string())),
        }
        Ok(())
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
            ColumnData::Var { data, offsets, .. } => {
                data.extend_from_slice(bytes);
                let len_u32: u32 = data
                    .len()
                    .try_into()
                    .map_err(|_| Error::Other("var column too large".to_string()))?;
                offsets.push(len_u32);
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
        // Set fixed-width encoding once per column to avoid per-cell overhead.
        for (i, col) in out.columns.iter_mut().enumerate() {
            let Some(field) = out.schema.fields().get(i) else {
                return Err(Error::Other("schema/column mismatch".to_string()));
            };
            if let ColumnData::Fixed { encoding, .. } = col {
                *encoding = match field.ty {
                    ColumnarType::I16
                    | ColumnarType::I32
                    | ColumnarType::I64
                    | ColumnarType::F32
                    | ColumnarType::F64 => FixedEncoding::PgBe,
                    _ => FixedEncoding::Le,
                };
            }
        }

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
        let sig = self.read_slice(11).await?;
        if sig != SIG {
            return Err(Error::Other("invalid COPY BINARY signature".to_string()));
        }
        self.maybe_compact();
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
            let _ = self.read_slice(ext_len_usize).await?;
            self.maybe_compact();
        }
        Ok(())
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
                ColumnData::Fixed { width, values, .. } => {
                    let start = values.len();
                    let new_len = start
                        .checked_add(*width)
                        .ok_or_else(|| Error::Other("fixed column size overflow".to_string()))?;
                    values.resize(new_len, 0);
                }
                ColumnData::Var { offsets, data, .. } => {
                    let len_u32: u32 = data
                        .len()
                        .try_into()
                        .map_err(|_| Error::Other("var column too large".to_string()))?;
                    offsets.push(len_u32);
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
            ColumnData::Fixed {
                ty,
                width,
                values,
                ..
            } => {
                if len_usize != *width {
                    return Err(Error::Other(format!(
                        "COPY field length mismatch for {ty:?}: got {len_usize}, expected {width}"
                    )));
                }
                let bytes = self.read_slice(len_usize).await?;
                match ty {
                    ColumnarType::Bool => {
                        values.push(bytes[0]);
                    }
                    ColumnarType::I16 => {
                        values.extend_from_slice(bytes);
                    }
                    ColumnarType::I32 => {
                        values.extend_from_slice(bytes);
                    }
                    ColumnarType::I64 => {
                        values.extend_from_slice(bytes);
                    }
                    ColumnarType::F32 => {
                        values.extend_from_slice(bytes);
                    }
                    ColumnarType::F64 => {
                        values.extend_from_slice(bytes);
                    }
                    ColumnarType::Uuid => {
                        values.extend_from_slice(bytes);
                    }
                    ColumnarType::TimestampTzMicros => {
                        let pg_micros = i64::from_be_bytes([
                            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6],
                            bytes[7],
                        ]);
                        if pg_micros == i64::MIN || pg_micros == i64::MAX {
                            return Err(Error::Other(
                                "timestamptz infinity is not supported".to_string(),
                            ));
                        }
                        const UNIX_TO_PG_EPOCH_MICROS: i64 = 946_684_800_000_000;
                        let unix_micros = pg_micros
                            .checked_add(UNIX_TO_PG_EPOCH_MICROS)
                            .ok_or_else(|| Error::Other("timestamp overflow".to_string()))?;
                        values.extend_from_slice(&unix_micros.to_le_bytes());
                    }
                    ColumnarType::Utf8 | ColumnarType::Bytes => {
                        return Err(Error::Other("internal type mismatch".to_string()));
                    }
                }
                self.maybe_compact();
            }
            ColumnData::Var { ty, data, .. } => {
                let validate_utf8 = self.read_cfg.validate_utf8;
                let bytes = self.read_slice(len_usize).await?;
                match ty {
                    ColumnarType::Utf8 => {
                        if validate_utf8 && std::str::from_utf8(bytes).is_err() {
                            return Err(Error::Other("invalid UTF-8 in TEXT column".to_string()));
                        }
                        data.extend_from_slice(bytes);
                    }
                    ColumnarType::Bytes => {
                        data.extend_from_slice(bytes);
                    }
                    _ => return Err(Error::Other("internal type mismatch".to_string())),
                }
                let len_u32: u32 = data
                    .len()
                    .try_into()
                    .map_err(|_| Error::Other("var column too large".to_string()))?;
                if let ColumnData::Var { offsets, .. } = col {
                    offsets.push(len_u32);
                }
                self.maybe_compact();
            }
        }

        Ok(())
    }

    async fn ensure_available(&mut self, needed: usize) -> Result<()> {
        while self.buf.len().saturating_sub(self.pos) < needed {
            match self.copy_out.try_next().await {
                Ok(Some(chunk)) => self.buf.extend_from_slice(&chunk),
                Ok(None) => break,
                Err(e) => return Err(Error::Database(e)),
            }
        }
        if self.buf.len().saturating_sub(self.pos) < needed {
            return Err(Error::Other("unexpected end of COPY stream".to_string()));
        }
        Ok(())
    }

    fn maybe_compact(&mut self) {
        if self.pos < self.cfg.compact_after_bytes {
            return;
        }
        let remaining = self.buf.len().saturating_sub(self.pos);
        if remaining == 0 {
            self.buf.clear();
            self.pos = 0;
            return;
        }
        // Avoid repeatedly moving large tails; compact only when the remaining tail is small.
        if remaining <= self.cfg.compact_after_bytes {
            self.buf.copy_within(self.pos.., 0);
            self.buf.truncate(remaining);
            self.pos = 0;
        }
    }

    async fn read_slice(&mut self, n: usize) -> Result<&[u8]> {
        if n == 0 {
            return Ok(&[]);
        }
        self.ensure_available(n).await?;
        let start = self.pos;
        let end = start
            .checked_add(n)
            .ok_or_else(|| Error::Other("buffer index overflow".to_string()))?;
        let slice = &self.buf[start..end];
        self.pos = end;
        Ok(slice)
    }

    async fn read_i16_be(&mut self) -> Result<i16> {
        let b = self.read_slice(2).await?;
        let arr = [b[0], b[1]];
        self.maybe_compact();
        Ok(i16::from_be_bytes(arr))
    }

    async fn read_i32_be(&mut self) -> Result<i32> {
        let b = self.read_slice(4).await?;
        let arr = [b[0], b[1], b[2], b[3]];
        self.maybe_compact();
        Ok(i32::from_be_bytes(arr))
    }
}
