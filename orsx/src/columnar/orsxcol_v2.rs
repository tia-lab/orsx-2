use crate::{Error, Result};
use super::types::{ColumnarBatch, ColumnarField, ColumnarSchema, ColumnarType, ColumnData};
use std::mem::size_of;
use std::collections::HashMap;
use bytes::Bytes;

// ORSXCOL2 is a performance-focused envelope version. v2 MVP supports:
// - fixed-width: PlainLE (encoding_id=0) and PgBeFixed (encoding_id=1)
// - varlen: PlainVar (encoding_id=0)
//
// Experimental (opt-in):
// - varlen: DictUtf8 (encoding_id=2)
// - I64: DeltaVarintI64 (encoding_id=3)
//
// All integer fields in the envelope header/descriptor are little-endian.

const MAGIC: &[u8; 8] = b"ORSXCOL2";
const VERSION: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedEncodingId {
    PlainLe = 0,
    PgBeFixed = 1,
}

impl FixedEncodingId {
    fn from_u16(v: u16) -> Option<Self> {
        match v {
            0 => Some(FixedEncodingId::PlainLe),
            1 => Some(FixedEncodingId::PgBeFixed),
            _ => None,
        }
    }
}

const ENC_PLAIN: u16 = 0;
const ENC_DICT_UTF8: u16 = 2;
const ENC_DELTA_VARINT_I64: u16 = 3;

fn type_id(ty: ColumnarType) -> u16 {
    match ty {
        ColumnarType::Bool => 1,
        ColumnarType::I16 => 2,
        ColumnarType::I32 => 3,
        ColumnarType::I64 => 4,
        ColumnarType::F32 => 5,
        ColumnarType::F64 => 6,
        ColumnarType::Uuid => 7,
        ColumnarType::TimestampTzMicros => 8,
        ColumnarType::Utf8 => 9,
        ColumnarType::Bytes => 10,
        ColumnarType::JsonbText => 11,
    }
}

fn type_from_id(id: u16) -> Result<ColumnarType> {
    match id {
        1 => Ok(ColumnarType::Bool),
        2 => Ok(ColumnarType::I16),
        3 => Ok(ColumnarType::I32),
        4 => Ok(ColumnarType::I64),
        5 => Ok(ColumnarType::F32),
        6 => Ok(ColumnarType::F64),
        7 => Ok(ColumnarType::Uuid),
        8 => Ok(ColumnarType::TimestampTzMicros),
        9 => Ok(ColumnarType::Utf8),
        10 => Ok(ColumnarType::Bytes),
        11 => Ok(ColumnarType::JsonbText),
        _ => Err(Error::Other(format!("unknown column type id: {id}"))),
    }
}

fn ceil_div_8(n: usize) -> Result<usize> {
    n.checked_add(7)
        .ok_or_else(|| Error::Other("size overflow".to_string()))
        .map(|v| v / 8)
}

fn write_u16_le(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_u32_le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_u16_len_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> Result<()> {
    let len: u16 = bytes
        .len()
        .try_into()
        .map_err(|_| Error::Other("name too long".to_string()))?;
    write_u16_le(out, len);
    out.extend_from_slice(bytes);
    Ok(())
}

fn write_u32_len_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> Result<()> {
    let len: u32 = bytes
        .len()
        .try_into()
        .map_err(|_| Error::Other("payload too large".to_string()))?;
    write_u32_le(out, len);
    out.extend_from_slice(bytes);
    Ok(())
}

#[inline]
fn checked_byte_len(count: usize, elem_size: usize, err: &'static str) -> Result<usize> {
    count
        .checked_mul(elem_size)
        .ok_or_else(|| Error::Other(err.to_string()))
}

#[derive(Debug, Default, Clone)]
pub struct OrsxcolV2EncodeWorkspace {
    enable_dict_utf8: bool,
    enable_delta_varint_i64: bool,

    var_coalesce: Vec<u8>,

    dict_values: Vec<Vec<u8>>,
    dict_map: HashMap<Vec<u8>, u32>,
    dict_indices: Vec<u32>,
    dict_indices_bytes: Vec<u8>,
    dict_offsets: Vec<u32>,
    dict_blob: Vec<u8>,

    delta_buf: Vec<u8>,
}

#[derive(Debug, Default, Clone)]
pub struct OrsxcolV2DecodeWorkspace {
    dict_offsets: Vec<u32>,
}

impl OrsxcolV2EncodeWorkspace {
    pub fn set_enable_dict_utf8(&mut self, enabled: bool) -> &mut Self {
        self.enable_dict_utf8 = enabled;
        self
    }

    pub fn set_enable_delta_varint_i64(&mut self, enabled: bool) -> &mut Self {
        self.enable_delta_varint_i64 = enabled;
        self
    }
}

#[inline]
fn zigzag_i64_to_u64(x: i64) -> u64 {
    ((x as u64) << 1) ^ (((x >> 63) as u64) & 1)
}

#[inline]
fn zigzag_u64_to_i64(x: u64) -> i64 {
    let v = (x >> 1) as i64;
    let neg = (x & 1) as i64;
    v ^ -neg
}

#[inline]
fn write_u64_varint(out: &mut Vec<u8>, mut x: u64) {
    while x >= 0x80 {
        out.push((x as u8) | 0x80);
        x >>= 7;
    }
    out.push(x as u8);
}

fn read_u64_varint(bytes: &[u8], pos: &mut usize) -> Result<u64> {
    let mut x: u64 = 0;
    let mut shift: u32 = 0;
    loop {
        if *pos >= bytes.len() {
            return Err(Error::Other("truncated varint".to_string()));
        }
        let b = bytes[*pos];
        *pos += 1;
        let lo = (b & 0x7F) as u64;
        if shift >= 64 {
            return Err(Error::Other("varint overflow".to_string()));
        }
        x |= lo << shift;
        if (b & 0x80) == 0 {
            return Ok(x);
        }
        shift = shift
            .checked_add(7)
            .ok_or_else(|| Error::Other("varint overflow".to_string()))?;
    }
}

#[inline]
fn validity_all_valid(validity: &[u8], row_count: usize) -> bool {
    if row_count == 0 {
        return true;
    }
    let needed = match row_count.checked_add(7) {
        Some(v) => v / 8,
        None => return false,
    };
    if validity.len() < needed {
        return false;
    }
    if needed == 0 {
        return true;
    }
    let rem = row_count % 8;
    let full = if rem == 0 { needed } else { needed - 1 };
    for &b in &validity[..full] {
        if b != 0xFF {
            return false;
        }
    }
    if rem == 0 {
        return true;
    }
    let mask = (1u8 << rem) - 1;
    (validity[needed - 1] & mask) == mask
}

fn build_delta_varint_i64_payload<'a>(
    ws: &'a mut OrsxcolV2EncodeWorkspace,
    values: &[i64],
) -> Result<Option<&'a [u8]>> {
    if values.is_empty() {
        return Ok(None);
    }
    ws.delta_buf.clear();
    ws.delta_buf.reserve(8 + values.len().saturating_mul(2));
    ws.delta_buf.extend_from_slice(&values[0].to_le_bytes());
    let mut prev = values[0];
    for &v in &values[1..] {
        let delta = v.wrapping_sub(prev);
        write_u64_varint(&mut ws.delta_buf, zigzag_i64_to_u64(delta));
        prev = v;
    }
    if ws.delta_buf.len() >= values.len() * 8 {
        return Ok(None);
    }
    Ok(Some(ws.delta_buf.as_slice()))
}

fn build_dict_utf8_payload<'a>(
    ws: &'a mut OrsxcolV2EncodeWorkspace,
    validity: &[u8],
    row_count: usize,
    offsets: &[u32],
    data: &[u8],
    chunks: &[Bytes],
    total_len: u32,
) -> Result<Option<(&'a [u8], &'a [u8])>> {
    if row_count == 0 {
        return Ok(None);
    }
    if offsets.len() != row_count + 1 {
        return Ok(None);
    }
    let total_usize: usize = total_len
        .try_into()
        .map_err(|_| Error::Other("var column too large".to_string()))?;

    ws.var_coalesce.clear();
    ws.var_coalesce.reserve(total_usize);
    ws.var_coalesce.extend_from_slice(data);
    for c in chunks {
        ws.var_coalesce.extend_from_slice(c.as_ref());
    }
    if ws.var_coalesce.len() != total_usize {
        return Err(Error::Other("var payload length mismatch".to_string()));
    }

    ws.dict_values.clear();
    ws.dict_map.clear();
    ws.dict_indices.clear();
    ws.dict_indices.reserve(row_count);

    for row in 0..row_count {
        let is_valid = (validity[row / 8] & (1u8 << (row % 8))) != 0;
        if !is_valid {
            ws.dict_indices.push(0);
            continue;
        }
        let start = offsets[row] as usize;
        let end = offsets[row + 1] as usize;
        if end < start || end > total_usize {
            return Err(Error::Other("offset out of bounds".to_string()));
        }
        let bytes = &ws.var_coalesce[start..end];
        if let Some(&idx) = ws.dict_map.get(bytes) {
            ws.dict_indices.push(idx);
            continue;
        }
        let idx: u32 = ws
            .dict_values
            .len()
            .try_into()
            .map_err(|_| Error::Other("dict too large".to_string()))?;
        let key = bytes.to_vec();
        ws.dict_values.push(key.clone());
        ws.dict_map.insert(key, idx);
        ws.dict_indices.push(idx);
    }

    let dict_count = ws.dict_values.len();
    if dict_count == 0 {
        return Ok(None);
    }

    let index_width: usize = if dict_count <= 0x100 {
        1
    } else if dict_count <= 0x1_0000 {
        2
    } else {
        4
    };

    let indices_len = row_count
        .checked_mul(index_width)
        .ok_or_else(|| Error::Other("indices overflow".to_string()))?;

    ws.dict_indices_bytes.clear();
    ws.dict_indices_bytes.reserve(indices_len);
    match index_width {
        1 => {
            for &idx in &ws.dict_indices {
                ws.dict_indices_bytes.push(idx as u8);
            }
        }
        2 => {
            for &idx in &ws.dict_indices {
                let v: u16 = idx
                    .try_into()
                    .map_err(|_| Error::Other("dict index overflow".to_string()))?;
                ws.dict_indices_bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        4 => {
            for &idx in &ws.dict_indices {
                ws.dict_indices_bytes.extend_from_slice(&idx.to_le_bytes());
            }
        }
        _ => return Err(Error::Other("invalid index width".to_string())),
    }
    if ws.dict_indices_bytes.len() != indices_len {
        return Err(Error::Other("indices length mismatch".to_string()));
    }

    ws.dict_offsets.clear();
    ws.dict_offsets.reserve(dict_count + 1);
    ws.dict_offsets.push(0u32);
    let mut total: u32 = 0;
    for v in &ws.dict_values {
        let len_u32: u32 = v
            .len()
            .try_into()
            .map_err(|_| Error::Other("dict entry too large".to_string()))?;
        total = total
            .checked_add(len_u32)
            .ok_or_else(|| Error::Other("dict overflow".to_string()))?;
        ws.dict_offsets.push(total);
    }

    ws.dict_blob.clear();
    ws.dict_blob.reserve(1 + 4 + (dict_count + 1) * 4 + total as usize);
    ws.dict_blob.push(index_width as u8);
    let dict_count_u32: u32 = dict_count
        .try_into()
        .map_err(|_| Error::Other("dict too large".to_string()))?;
    write_u32_le(&mut ws.dict_blob, dict_count_u32);
    for &o in &ws.dict_offsets {
        write_u32_le(&mut ws.dict_blob, o);
    }
    for v in &ws.dict_values {
        ws.dict_blob.extend_from_slice(v.as_slice());
    }

    let plain_offsets_len = (row_count + 1)
        .checked_mul(4)
        .ok_or_else(|| Error::Other("offsets overflow".to_string()))?;
    let plain_total = plain_offsets_len
        .checked_add(total_usize)
        .ok_or_else(|| Error::Other("payload overflow".to_string()))?;
    let dict_total = ws
        .dict_indices_bytes
        .len()
        .checked_add(ws.dict_blob.len())
        .ok_or_else(|| Error::Other("payload overflow".to_string()))?;
    if dict_total >= plain_total {
        return Ok(None);
    }

    Ok(Some((
        ws.dict_indices_bytes.as_slice(),
        ws.dict_blob.as_slice(),
    )))
}

fn decode_dict_utf8_to_var_col(
    ws: &mut OrsxcolV2DecodeWorkspace,
    row_count: usize,
    validity: &[u8],
    indices_bytes: &[u8],
    dict_blob: &[u8],
    out_offsets: &mut Vec<u32>,
    out_data: &mut Vec<u8>,
) -> Result<()> {
    if row_count == 0 {
        out_offsets.clear();
        out_offsets.push(0);
        out_data.clear();
        return Ok(());
    }

    if dict_blob.len() < 1 + 4 {
        return Err(Error::Other("dict blob truncated".to_string()));
    }
    let index_width = dict_blob[0] as usize;
    if index_width != 1 && index_width != 2 && index_width != 4 {
        return Err(Error::Other("invalid dict index width".to_string()));
    }
    let dict_count = u32::from_le_bytes([dict_blob[1], dict_blob[2], dict_blob[3], dict_blob[4]])
        as usize;
    let offsets_bytes_len = (dict_count + 1)
        .checked_mul(4)
        .ok_or_else(|| Error::Other("dict offsets overflow".to_string()))?;
    let header_len: usize = 1 + 4;
    let offsets_start: usize = header_len;
    let offsets_end = offsets_start
        .checked_add(offsets_bytes_len)
        .ok_or_else(|| Error::Other("dict offsets overflow".to_string()))?;
    if offsets_end > dict_blob.len() {
        return Err(Error::Other("dict offsets truncated".to_string()));
    }

    ws.dict_offsets.clear();
    ws.dict_offsets.resize(dict_count + 1, 0u32);
    #[cfg(target_endian = "little")]
    {
        let out_bytes = unsafe {
            std::slice::from_raw_parts_mut(
                ws.dict_offsets.as_mut_ptr() as *mut u8,
                offsets_bytes_len,
            )
        };
        out_bytes.copy_from_slice(&dict_blob[offsets_start..offsets_end]);
    }
    #[cfg(not(target_endian = "little"))]
    {
        for i in 0..(dict_count + 1) {
            let j = offsets_start + i * 4;
            ws.dict_offsets[i] = u32::from_le_bytes([
                dict_blob[j],
                dict_blob[j + 1],
                dict_blob[j + 2],
                dict_blob[j + 3],
            ]);
        }
    }

    let dict_bytes = &dict_blob[offsets_end..];
    let dict_total = *ws.dict_offsets.last().unwrap_or(&0);
    if dict_total as usize != dict_bytes.len() {
        return Err(Error::Other("dict final offset mismatch".to_string()));
    }
    let mut prev = 0u32;
    for &o in ws.dict_offsets.iter() {
        if o < prev {
            return Err(Error::Other("dict offsets must be non-decreasing".to_string()));
        }
        prev = o;
    }

    let expected_indices_len = row_count
        .checked_mul(index_width)
        .ok_or_else(|| Error::Other("indices overflow".to_string()))?;
    if indices_bytes.len() != expected_indices_len {
        return Err(Error::Other("indices length mismatch".to_string()));
    }

    out_offsets.clear();
    out_offsets.reserve(row_count + 1);
    out_offsets.push(0u32);
    out_data.clear();

    let mut total: u32 = 0;
    for row in 0..row_count {
        let is_valid = (validity[row / 8] & (1u8 << (row % 8))) != 0;
        if !is_valid {
            out_offsets.push(total);
            continue;
        }
        let idx: usize = match index_width {
            1 => indices_bytes[row] as usize,
            2 => {
                let j = row * 2;
                u16::from_le_bytes([indices_bytes[j], indices_bytes[j + 1]]) as usize
            }
            4 => {
                let j = row * 4;
                u32::from_le_bytes([
                    indices_bytes[j],
                    indices_bytes[j + 1],
                    indices_bytes[j + 2],
                    indices_bytes[j + 3],
                ]) as usize
            }
            _ => return Err(Error::Other("invalid index width".to_string())),
        };
        if idx >= dict_count {
            return Err(Error::Other("dict index out of bounds".to_string()));
        }
        let start = ws.dict_offsets[idx] as usize;
        let end = ws.dict_offsets[idx + 1] as usize;
        out_data.extend_from_slice(&dict_bytes[start..end]);
        let add: u32 = (end - start)
            .try_into()
            .map_err(|_| Error::Other("dict expansion too large".to_string()))?;
        total = total
            .checked_add(add)
            .ok_or_else(|| Error::Other("dict expansion overflow".to_string()))?;
        out_offsets.push(total);
    }

    Ok(())
}

fn decode_delta_varint_i64_from_payload(payload: &[u8], row_count: usize, out: &mut [i64]) -> Result<()> {
    if row_count == 0 {
        return Ok(());
    }
    if out.len() < row_count {
        return Err(Error::Other("output length mismatch".to_string()));
    }
    if payload.len() < 8 {
        return Err(Error::Other("delta payload truncated".to_string()));
    }
    let mut pos = 8usize;
    let base_bytes = &payload[..8];
    let mut prev = i64::from_le_bytes([
        base_bytes[0],
        base_bytes[1],
        base_bytes[2],
        base_bytes[3],
        base_bytes[4],
        base_bytes[5],
        base_bytes[6],
        base_bytes[7],
    ]);
    out[0] = prev;
    for i in 1..row_count {
        let zz = read_u64_varint(payload, &mut pos)?;
        let delta = zigzag_u64_to_i64(zz);
        prev = prev.wrapping_add(delta);
        out[i] = prev;
    }
    if pos != payload.len() {
        return Err(Error::Other("trailing bytes in delta payload".to_string()));
    }
    Ok(())
}

pub fn encode_orsxcol_v2_into(batch: &ColumnarBatch, out: &mut Vec<u8>) -> Result<()> {
    let mut ws = OrsxcolV2EncodeWorkspace::default();
    encode_orsxcol_v2_into_with_workspace(batch, out, &mut ws)
}

pub fn encode_orsxcol_v2_into_with_workspace(
    batch: &ColumnarBatch,
    out: &mut Vec<u8>,
    ws: &mut OrsxcolV2EncodeWorkspace,
) -> Result<()> {
    out.clear();

    out.extend_from_slice(MAGIC);
    write_u16_le(out, VERSION);
    write_u16_le(out, 0); // flags

    let row_count_u32: u32 = batch
        .row_count
        .try_into()
        .map_err(|_| Error::Other("row_count too large".to_string()))?;
    write_u32_le(out, row_count_u32);

    let col_count_u16: u16 = batch
        .columns
        .len()
        .try_into()
        .map_err(|_| Error::Other("col_count too large".to_string()))?;
    write_u16_le(out, col_count_u16);

    // v2 MVP: no schema_id.
    write_u16_le(out, 0); // schema_id_len

    let expected_validity = ceil_div_8(batch.row_count)?;

    for (field, col) in batch.schema.fields().iter().zip(batch.columns.iter()) {
        let tid = type_id(field.ty);
        write_u16_le(out, tid);

        let name_bytes = field.name.as_deref().unwrap_or("").as_bytes();

        let validity: &[u8] = match col {
            ColumnData::Var { validity, .. } => validity.bytes.as_slice(),
            ColumnData::FixedBool { validity, .. } => validity.bytes.as_slice(),
            ColumnData::FixedI16 { validity, .. } => validity.bytes.as_slice(),
            ColumnData::FixedI32 { validity, .. } => validity.bytes.as_slice(),
            ColumnData::FixedI64 { validity, .. } => validity.bytes.as_slice(),
            ColumnData::FixedF32Bits { validity, .. } => validity.bytes.as_slice(),
            ColumnData::FixedF64Bits { validity, .. } => validity.bytes.as_slice(),
            ColumnData::FixedUuid { validity, .. } => validity.bytes.as_slice(),
            ColumnData::FixedTimestampMicros { validity, .. } => validity.bytes.as_slice(),
        };
        // In-memory validity bitmaps are sized to the batch row_capacity, while the envelope is
        // sized to the batch row_count. Emit only the prefix required for row_count.
        if validity.len() < expected_validity {
            return Err(Error::Other("validity length mismatch".to_string()));
        }
        let validity_prefix = &validity[..expected_validity];

        let mut dict_payload: Option<(&[u8], &[u8])> = None;
        let mut delta_payload: Option<&[u8]> = None;

        let encoding_id: u16 = match col {
            ColumnData::Var {
                offsets,
                data,
                chunks,
                total_len,
                ..
            } if ws.enable_dict_utf8
                && matches!(field.ty, ColumnarType::Utf8 | ColumnarType::JsonbText) =>
            {
                if let Some((idx_bytes, dict_blob)) = build_dict_utf8_payload(
                    ws,
                    validity_prefix,
                    batch.row_count,
                    offsets.as_slice(),
                    data.as_slice(),
                    chunks.as_slice(),
                    *total_len,
                )? {
                    dict_payload = Some((idx_bytes, dict_blob));
                    ENC_DICT_UTF8
                } else {
                    ENC_PLAIN
                }
            }
            ColumnData::FixedI64 { values, .. }
                if ws.enable_delta_varint_i64
                    && field.ty == ColumnarType::I64
                    && validity_all_valid(validity_prefix, batch.row_count) =>
            {
                if let Some(payload) = build_delta_varint_i64_payload(ws, values.as_slice())? {
                    delta_payload = Some(payload);
                    ENC_DELTA_VARINT_I64
                } else {
                    ENC_PLAIN
                }
            }
            ColumnData::FixedTimestampMicros { values, .. }
                if ws.enable_delta_varint_i64
                    && field.ty == ColumnarType::TimestampTzMicros
                    && validity_all_valid(validity_prefix, batch.row_count) =>
            {
                if let Some(payload) = build_delta_varint_i64_payload(ws, values.as_slice())? {
                    delta_payload = Some(payload);
                    ENC_DELTA_VARINT_I64
                } else {
                    ENC_PLAIN
                }
            }
            ColumnData::Var { .. } => ENC_PLAIN,
            _ => FixedEncodingId::PlainLe as u16,
        };

        write_u16_le(out, encoding_id);
        // col_flags (reserved; v2 MVP: 0)
        write_u16_le(out, 0);
        write_u16_len_bytes(out, name_bytes)?;
        write_u32_len_bytes(out, validity_prefix)?;

        match col {
            ColumnData::FixedBool { values, .. } => {
                if values.len() != batch.row_count {
                    return Err(Error::Other("fixed bool length mismatch".to_string()));
                }
                write_u32_len_bytes(out, values.as_slice())?;
                write_u32_le(out, 0);
            }
            ColumnData::FixedI16 { values, .. } => {
                if values.len() != batch.row_count {
                    return Err(Error::Other("fixed i16 length mismatch".to_string()));
                }
                let byte_len = checked_byte_len(batch.row_count, size_of::<i16>(), "values overflow")?;
                write_u32_le(
                    out,
                    byte_len
                        .try_into()
                        .map_err(|_| Error::Other("payload too large".to_string()))?,
                );
                out.reserve(byte_len);
                #[cfg(target_endian = "little")]
                {
                    let bytes = unsafe {
                        std::slice::from_raw_parts(values.as_ptr() as *const u8, byte_len)
                    };
                    out.extend_from_slice(bytes);
                }
                #[cfg(not(target_endian = "little"))]
                {
                    for &v in values {
                        out.extend_from_slice(&v.to_le_bytes());
                    }
                }
                write_u32_le(out, 0);
            }
            ColumnData::FixedI32 { values, .. } => {
                if values.len() != batch.row_count {
                    return Err(Error::Other("fixed i32 length mismatch".to_string()));
                }
                let byte_len = checked_byte_len(batch.row_count, size_of::<i32>(), "values overflow")?;
                write_u32_le(
                    out,
                    byte_len
                        .try_into()
                        .map_err(|_| Error::Other("payload too large".to_string()))?,
                );
                out.reserve(byte_len);
                #[cfg(target_endian = "little")]
                {
                    let bytes = unsafe {
                        std::slice::from_raw_parts(values.as_ptr() as *const u8, byte_len)
                    };
                    out.extend_from_slice(bytes);
                }
                #[cfg(not(target_endian = "little"))]
                {
                    for &v in values {
                        out.extend_from_slice(&v.to_le_bytes());
                    }
                }
                write_u32_le(out, 0);
            }
            ColumnData::FixedI64 { values, .. } => {
                if values.len() != batch.row_count {
                    return Err(Error::Other("fixed i64 length mismatch".to_string()));
                }
                if encoding_id == ENC_DELTA_VARINT_I64 {
                    let payload = delta_payload.ok_or_else(|| {
                        Error::Other("missing delta payload".to_string())
                    })?;
                    write_u32_len_bytes(out, payload)?;
                    write_u32_le(out, 0);
                } else {
                    let byte_len =
                        checked_byte_len(batch.row_count, size_of::<i64>(), "values overflow")?;
                    write_u32_le(
                        out,
                        byte_len
                            .try_into()
                            .map_err(|_| Error::Other("payload too large".to_string()))?,
                    );
                    out.reserve(byte_len);
                    #[cfg(target_endian = "little")]
                    {
                        let bytes = unsafe {
                            std::slice::from_raw_parts(values.as_ptr() as *const u8, byte_len)
                        };
                        out.extend_from_slice(bytes);
                    }
                    #[cfg(not(target_endian = "little"))]
                    {
                        for &v in values {
                            out.extend_from_slice(&v.to_le_bytes());
                        }
                    }
                    write_u32_le(out, 0);
                }
            }
            ColumnData::FixedF32Bits { values, .. } => {
                if values.len() != batch.row_count {
                    return Err(Error::Other("fixed f32 length mismatch".to_string()));
                }
                let byte_len = checked_byte_len(batch.row_count, size_of::<u32>(), "values overflow")?;
                write_u32_le(
                    out,
                    byte_len
                        .try_into()
                        .map_err(|_| Error::Other("payload too large".to_string()))?,
                );
                out.reserve(byte_len);
                #[cfg(target_endian = "little")]
                {
                    let bytes = unsafe {
                        std::slice::from_raw_parts(values.as_ptr() as *const u8, byte_len)
                    };
                    out.extend_from_slice(bytes);
                }
                #[cfg(not(target_endian = "little"))]
                {
                    for &v in values {
                        out.extend_from_slice(&v.to_le_bytes());
                    }
                }
                write_u32_le(out, 0);
            }
            ColumnData::FixedF64Bits { values, .. } => {
                if values.len() != batch.row_count {
                    return Err(Error::Other("fixed f64 length mismatch".to_string()));
                }
                let byte_len = checked_byte_len(batch.row_count, size_of::<u64>(), "values overflow")?;
                write_u32_le(
                    out,
                    byte_len
                        .try_into()
                        .map_err(|_| Error::Other("payload too large".to_string()))?,
                );
                out.reserve(byte_len);
                #[cfg(target_endian = "little")]
                {
                    let bytes = unsafe {
                        std::slice::from_raw_parts(values.as_ptr() as *const u8, byte_len)
                    };
                    out.extend_from_slice(bytes);
                }
                #[cfg(not(target_endian = "little"))]
                {
                    for &v in values {
                        out.extend_from_slice(&v.to_le_bytes());
                    }
                }
                write_u32_le(out, 0);
            }
            ColumnData::FixedUuid { values, .. } => {
                if values.len() != batch.row_count {
                    return Err(Error::Other("fixed uuid length mismatch".to_string()));
                }
                let byte_len = batch
                    .row_count
                    .checked_mul(16)
                    .ok_or_else(|| Error::Other("values overflow".to_string()))?;
                write_u32_le(
                    out,
                    byte_len
                        .try_into()
                        .map_err(|_| Error::Other("payload too large".to_string()))?,
                );
                out.reserve(byte_len);
                for v in values {
                    out.extend_from_slice(v);
                }
                write_u32_le(out, 0);
            }
            ColumnData::FixedTimestampMicros { values, .. } => {
                if values.len() != batch.row_count {
                    return Err(Error::Other("fixed timestamp length mismatch".to_string()));
                }
                if encoding_id == ENC_DELTA_VARINT_I64 {
                    let payload = delta_payload.ok_or_else(|| {
                        Error::Other("missing delta payload".to_string())
                    })?;
                    write_u32_len_bytes(out, payload)?;
                    write_u32_le(out, 0);
                } else {
                    let byte_len =
                        checked_byte_len(batch.row_count, size_of::<i64>(), "values overflow")?;
                    write_u32_le(
                        out,
                        byte_len
                            .try_into()
                            .map_err(|_| Error::Other("payload too large".to_string()))?,
                    );
                    out.reserve(byte_len);
                    #[cfg(target_endian = "little")]
                    {
                        let bytes = unsafe {
                            std::slice::from_raw_parts(values.as_ptr() as *const u8, byte_len)
                        };
                        out.extend_from_slice(bytes);
                    }
                    #[cfg(not(target_endian = "little"))]
                    {
                        for &v in values {
                            out.extend_from_slice(&v.to_le_bytes());
                        }
                    }
                    write_u32_le(out, 0);
                }
            }
            ColumnData::Var {
                offsets,
                data,
                chunks,
                total_len,
                ..
            } => {
                if encoding_id == ENC_DICT_UTF8 {
                    let (idx_bytes, dict_blob) = dict_payload.ok_or_else(|| {
                        Error::Other("missing dict payload".to_string())
                    })?;
                    write_u32_len_bytes(out, idx_bytes)?;
                    write_u32_len_bytes(out, dict_blob)?;
                    continue;
                }

                // payload_1: offsets (u32 LE)
                let offsets_byte_len: usize = offsets
                    .len()
                    .checked_mul(4)
                    .ok_or_else(|| Error::Other("offsets overflow".to_string()))?;
                write_u32_le(
                    out,
                    offsets_byte_len
                        .try_into()
                        .map_err(|_| Error::Other("payload too large".to_string()))?,
                );
                out.reserve(offsets_byte_len);
                #[cfg(target_endian = "little")]
                {
                    let bytes = unsafe {
                        std::slice::from_raw_parts(offsets.as_ptr() as *const u8, offsets_byte_len)
                    };
                    out.extend_from_slice(bytes);
                }
                #[cfg(not(target_endian = "little"))]
                {
                    for &o in offsets {
                        out.extend_from_slice(&o.to_le_bytes());
                    }
                }

                let expected_total = *offsets.last().unwrap_or(&0);
                if expected_total != *total_len {
                    return Err(Error::Other("var offsets/total_len mismatch".to_string()));
                }
                let total_usize: usize = (*total_len)
                    .try_into()
                    .map_err(|_| Error::Other("var column too large".to_string()))?;
                let data_len = data.len();
                let chunks_len: usize = chunks.iter().map(|c| c.len()).sum();
                if data_len
                    .checked_add(chunks_len)
                    .ok_or_else(|| Error::Other("var payload length overflow".to_string()))?
                    != total_usize
                {
                    return Err(Error::Other("var payload length mismatch".to_string()));
                }

                // payload_2: data bytes
                write_u32_le(
                    out,
                    total_usize
                        .try_into()
                        .map_err(|_| Error::Other("payload too large".to_string()))?,
                );
                let payload_start = out.len();
                out.reserve(total_usize);
                out.extend_from_slice(data.as_slice());
                for c in chunks {
                    out.extend_from_slice(c.as_ref());
                }
                if out.len().saturating_sub(payload_start) != total_usize {
                    return Err(Error::Other("var payload length mismatch".to_string()));
                }
            }
        }
    }

    Ok(())
}

pub fn decode_orsxcol_v2(bytes: &[u8]) -> Result<ColumnarBatch> {
    let mut ws = OrsxcolV2DecodeWorkspace::default();
    decode_orsxcol_v2_with_workspace(bytes, &mut ws)
}

pub fn decode_orsxcol_v2_with_workspace(
    bytes: &[u8],
    ws: &mut OrsxcolV2DecodeWorkspace,
) -> Result<ColumnarBatch> {
    let mut pos = 0usize;

    fn take<'a>(bytes: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8]> {
        let end = pos
            .checked_add(n)
            .ok_or_else(|| Error::Other("decode overflow".to_string()))?;
        if end > bytes.len() {
            return Err(Error::Other("truncated orscol".to_string()));
        }
        let slice = &bytes[*pos..end];
        *pos = end;
        Ok(slice)
    }

    fn read_u16_le(bytes: &[u8], pos: &mut usize) -> Result<u16> {
        let b = take(bytes, pos, 2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn read_u32_le(bytes: &[u8], pos: &mut usize) -> Result<u32> {
        let b = take(bytes, pos, 4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    let magic = take(bytes, &mut pos, 8)?;
    if magic != MAGIC {
        return Err(Error::Other("invalid ORSXCOL magic".to_string()));
    }
    let version = read_u16_le(bytes, &mut pos)?;
    if version != VERSION {
        return Err(Error::Other(format!("unsupported ORSXCOL version: {version}")));
    }
    let _flags = read_u16_le(bytes, &mut pos)?;
    let row_count = read_u32_le(bytes, &mut pos)? as usize;
    let col_count = read_u16_le(bytes, &mut pos)? as usize;
    if col_count == 0 {
        return Err(Error::Other("ORSXCOL must have at least one column".to_string()));
    }

    let schema_id_len = read_u16_le(bytes, &mut pos)? as usize;
    if schema_id_len > 0 {
        let _ = take(bytes, &mut pos, schema_id_len)?;
    }

    let expected_validity = ceil_div_8(row_count)?;

    let mut fields: Vec<ColumnarField> = Vec::with_capacity(col_count);
    let mut columns: Vec<ColumnData> = Vec::with_capacity(col_count);

    for _ in 0..col_count {
        let tid = read_u16_le(bytes, &mut pos)?;
        let ty = type_from_id(tid)?;
        let encoding_id_u16 = read_u16_le(bytes, &mut pos)?;
        let _col_flags = read_u16_le(bytes, &mut pos)?;

        let name_len = read_u16_le(bytes, &mut pos)? as usize;
        let name_bytes = take(bytes, &mut pos, name_len)?;
        let name = if name_len == 0 {
            None
        } else {
            Some(
                std::str::from_utf8(name_bytes)
                    .map_err(|_| Error::Other("invalid UTF-8 column name".to_string()))?
                    .to_string(),
            )
        };

        let validity_len = read_u32_le(bytes, &mut pos)? as usize;
        if validity_len != expected_validity {
            return Err(Error::Other("validity length mismatch".to_string()));
        }
        let validity = take(bytes, &mut pos, validity_len)?.to_vec();

        let payload1_len = read_u32_le(bytes, &mut pos)? as usize;
        let payload1 = take(bytes, &mut pos, payload1_len)?;
        let payload2_len = read_u32_le(bytes, &mut pos)? as usize;
        let payload2 = take(bytes, &mut pos, payload2_len)?;

        fields.push(ColumnarField { name, ty });

        match ty {
            ColumnarType::Utf8 | ColumnarType::Bytes | ColumnarType::JsonbText => match encoding_id_u16 {
                ENC_PLAIN => {
                    let expected_offsets_len = (row_count + 1)
                        .checked_mul(4)
                        .ok_or_else(|| Error::Other("offsets overflow".to_string()))?;
                    if payload1.len() != expected_offsets_len {
                        return Err(Error::Other("offsets length mismatch".to_string()));
                    }
                    let mut offsets: Vec<u32> = Vec::new();
                    offsets.resize(row_count + 1, 0u32);
                    #[cfg(target_endian = "little")]
                    {
                        let out_bytes = unsafe {
                            std::slice::from_raw_parts_mut(
                                offsets.as_mut_ptr() as *mut u8,
                                expected_offsets_len,
                            )
                        };
                        out_bytes.copy_from_slice(payload1);
                    }
                    #[cfg(not(target_endian = "little"))]
                    {
                        for i in 0..(row_count + 1) {
                            let j = i * 4;
                            offsets[i] = u32::from_le_bytes([
                                payload1[j],
                                payload1[j + 1],
                                payload1[j + 2],
                                payload1[j + 3],
                            ]);
                        }
                    }
                    // Validate monotonic offsets and final == data.len
                    let mut prev = 0u32;
                    for &o in &offsets {
                        if o < prev {
                            return Err(Error::Other("offsets must be non-decreasing".to_string()));
                        }
                        prev = o;
                    }
                    let final_off = *offsets.last().unwrap_or(&0);
                    let data_len_u32: u32 = payload2
                        .len()
                        .try_into()
                        .map_err(|_| Error::Other("data too large".to_string()))?;
                    if final_off != data_len_u32 {
                        return Err(Error::Other("final offset mismatch".to_string()));
                    }

                    let mut col = ColumnData::new(ty)?;
                    if let ColumnData::Var {
                        validity: v,
                        offsets: o,
                        data,
                        chunks,
                        total_len,
                        ..
                    } = &mut col
                    {
                        v.bytes = validity;
                        *o = offsets;
                        *total_len = data_len_u32;
                        data.clear();
                        data.extend_from_slice(payload2);
                        chunks.clear();
                    } else {
                        return Err(Error::Other("internal type mismatch".to_string()));
                    }
                    columns.push(col);
                }
                ENC_DICT_UTF8 => {
                    if ty == ColumnarType::Bytes {
                        return Err(Error::Other("DictUtf8 is not supported for Bytes".to_string()));
                    }
                    let mut col = ColumnData::new(ty)?;
                    if let ColumnData::Var {
                        validity: v,
                        offsets: o,
                        data,
                        chunks,
                        total_len,
                        ..
                    } = &mut col
                    {
                        v.bytes = validity;
                        decode_dict_utf8_to_var_col(ws, row_count, v.bytes.as_slice(), payload1, payload2, o, data)?;
                        *total_len = data
                            .len()
                            .try_into()
                            .map_err(|_| Error::Other("data too large".to_string()))?;
                        chunks.clear();
                    } else {
                        return Err(Error::Other("internal type mismatch".to_string()));
                    }
                    columns.push(col);
                }
                _ => return Err(Error::Other("invalid encoding for varlen column".to_string())),
            },
            _ => {
                // Fixed-width
                if encoding_id_u16 == ENC_DELTA_VARINT_I64
                    && !(ty == ColumnarType::I64 || ty == ColumnarType::TimestampTzMicros)
                {
                    return Err(Error::Other("invalid encoding for fixed column".to_string()));
                }
                let enc = if encoding_id_u16 == ENC_DELTA_VARINT_I64 {
                    FixedEncodingId::PlainLe
                } else {
                    FixedEncodingId::from_u16(encoding_id_u16).ok_or_else(|| {
                        Error::Other("invalid encoding for fixed column".to_string())
                    })?
                };
                if payload2_len != 0 {
                    return Err(Error::Other("fixed-width payload_2 must be empty".to_string()));
                }

                let mut col = ColumnData::new(ty)?;
                match &mut col {
                    ColumnData::FixedBool { validity: v, values } => {
                        v.bytes = validity;
                        if payload1.len() != row_count {
                            return Err(Error::Other("values length mismatch".to_string()));
                        }
                        values.clear();
                        values.extend_from_slice(payload1);
                    }
                    ColumnData::FixedI16 { validity: v, values } => {
                        v.bytes = validity;
                        if payload1.len() != row_count * 2 {
                            return Err(Error::Other("values length mismatch".to_string()));
                        }
                        values.clear();
                        values.resize(row_count, 0i16);
                        let byte_len = checked_byte_len(row_count, size_of::<i16>(), "values overflow")?;
                        #[cfg(target_endian = "little")]
                        {
                            let out_bytes = unsafe {
                                std::slice::from_raw_parts_mut(
                                    values.as_mut_ptr() as *mut u8,
                                    byte_len,
                                )
                            };
                            out_bytes.copy_from_slice(payload1);
                            if enc == FixedEncodingId::PgBeFixed {
                                for v in values.iter_mut() {
                                    *v = v.swap_bytes();
                                }
                            }
                        }
                        #[cfg(not(target_endian = "little"))]
                        {
                            for i in 0..row_count {
                                let j = i * 2;
                                let b = [payload1[j], payload1[j + 1]];
                                values[i] = match enc {
                                    FixedEncodingId::PlainLe => i16::from_le_bytes(b),
                                    FixedEncodingId::PgBeFixed => i16::from_be_bytes(b),
                                };
                            }
                        }
                    }
                    ColumnData::FixedI32 { validity: v, values } => {
                        v.bytes = validity;
                        if payload1.len() != row_count * 4 {
                            return Err(Error::Other("values length mismatch".to_string()));
                        }
                        values.clear();
                        values.resize(row_count, 0i32);
                        let byte_len = checked_byte_len(row_count, size_of::<i32>(), "values overflow")?;
                        #[cfg(target_endian = "little")]
                        {
                            let out_bytes = unsafe {
                                std::slice::from_raw_parts_mut(
                                    values.as_mut_ptr() as *mut u8,
                                    byte_len,
                                )
                            };
                            out_bytes.copy_from_slice(payload1);
                            if enc == FixedEncodingId::PgBeFixed {
                                for v in values.iter_mut() {
                                    *v = v.swap_bytes();
                                }
                            }
                        }
                        #[cfg(not(target_endian = "little"))]
                        {
                            for i in 0..row_count {
                                let j = i * 4;
                                let b = [payload1[j], payload1[j + 1], payload1[j + 2], payload1[j + 3]];
                                values[i] = match enc {
                                    FixedEncodingId::PlainLe => i32::from_le_bytes(b),
                                    FixedEncodingId::PgBeFixed => i32::from_be_bytes(b),
                                };
                            }
                        }
                    }
                    ColumnData::FixedI64 { validity: v, values } => {
                        v.bytes = validity;
                        values.clear();
                        values.resize(row_count, 0i64);
                        if encoding_id_u16 == ENC_DELTA_VARINT_I64 {
                            decode_delta_varint_i64_from_payload(
                                payload1,
                                row_count,
                                values.as_mut_slice(),
                            )?;
                        } else {
                            if payload1.len() != row_count * 8 {
                                return Err(Error::Other("values length mismatch".to_string()));
                            }
                            let byte_len =
                                checked_byte_len(row_count, size_of::<i64>(), "values overflow")?;
                            #[cfg(target_endian = "little")]
                            {
                                let out_bytes = unsafe {
                                    std::slice::from_raw_parts_mut(
                                        values.as_mut_ptr() as *mut u8,
                                        byte_len,
                                    )
                                };
                                out_bytes.copy_from_slice(payload1);
                                if enc == FixedEncodingId::PgBeFixed {
                                    for v in values.iter_mut() {
                                        *v = v.swap_bytes();
                                    }
                                }
                            }
                            #[cfg(not(target_endian = "little"))]
                            {
                                for i in 0..row_count {
                                    let j = i * 8;
                                    let b = [
                                        payload1[j],
                                        payload1[j + 1],
                                        payload1[j + 2],
                                        payload1[j + 3],
                                        payload1[j + 4],
                                        payload1[j + 5],
                                        payload1[j + 6],
                                        payload1[j + 7],
                                    ];
                                    values[i] = match enc {
                                        FixedEncodingId::PlainLe => i64::from_le_bytes(b),
                                        FixedEncodingId::PgBeFixed => i64::from_be_bytes(b),
                                    };
                                }
                            }
                        }
                    }
                    ColumnData::FixedF32Bits { validity: v, values } => {
                        v.bytes = validity;
                        if payload1.len() != row_count * 4 {
                            return Err(Error::Other("values length mismatch".to_string()));
                        }
                        values.clear();
                        values.resize(row_count, 0u32);
                        let byte_len = checked_byte_len(row_count, size_of::<u32>(), "values overflow")?;
                        #[cfg(target_endian = "little")]
                        {
                            let out_bytes = unsafe {
                                std::slice::from_raw_parts_mut(
                                    values.as_mut_ptr() as *mut u8,
                                    byte_len,
                                )
                            };
                            out_bytes.copy_from_slice(payload1);
                            if enc == FixedEncodingId::PgBeFixed {
                                for v in values.iter_mut() {
                                    *v = v.swap_bytes();
                                }
                            }
                        }
                        #[cfg(not(target_endian = "little"))]
                        {
                            for i in 0..row_count {
                                let j = i * 4;
                                let b = [payload1[j], payload1[j + 1], payload1[j + 2], payload1[j + 3]];
                                values[i] = match enc {
                                    FixedEncodingId::PlainLe => u32::from_le_bytes(b),
                                    FixedEncodingId::PgBeFixed => u32::from_be_bytes(b),
                                };
                            }
                        }
                    }
                    ColumnData::FixedF64Bits { validity: v, values } => {
                        v.bytes = validity;
                        if payload1.len() != row_count * 8 {
                            return Err(Error::Other("values length mismatch".to_string()));
                        }
                        values.clear();
                        values.resize(row_count, 0u64);
                        let byte_len = checked_byte_len(row_count, size_of::<u64>(), "values overflow")?;
                        #[cfg(target_endian = "little")]
                        {
                            let out_bytes = unsafe {
                                std::slice::from_raw_parts_mut(
                                    values.as_mut_ptr() as *mut u8,
                                    byte_len,
                                )
                            };
                            out_bytes.copy_from_slice(payload1);
                            if enc == FixedEncodingId::PgBeFixed {
                                for v in values.iter_mut() {
                                    *v = v.swap_bytes();
                                }
                            }
                        }
                        #[cfg(not(target_endian = "little"))]
                        {
                            for i in 0..row_count {
                                let j = i * 8;
                                let b = [
                                    payload1[j],
                                    payload1[j + 1],
                                    payload1[j + 2],
                                    payload1[j + 3],
                                    payload1[j + 4],
                                    payload1[j + 5],
                                    payload1[j + 6],
                                    payload1[j + 7],
                                ];
                                values[i] = match enc {
                                    FixedEncodingId::PlainLe => u64::from_le_bytes(b),
                                    FixedEncodingId::PgBeFixed => u64::from_be_bytes(b),
                                };
                            }
                        }
                    }
                    ColumnData::FixedUuid { validity: v, values } => {
                        v.bytes = validity;
                        if payload1.len() != row_count * 16 {
                            return Err(Error::Other("values length mismatch".to_string()));
                        }
                        values.clear();
                        values.resize(row_count, [0u8; 16]);
                        let byte_len = checked_byte_len(row_count, 16, "values overflow")?;
                        let out_bytes = unsafe {
                            std::slice::from_raw_parts_mut(values.as_mut_ptr() as *mut u8, byte_len)
                        };
                        out_bytes.copy_from_slice(payload1);
                    }
                    ColumnData::FixedTimestampMicros { validity: v, values } => {
                        v.bytes = validity;
                        values.clear();
                        values.resize(row_count, 0i64);
                        if encoding_id_u16 == ENC_DELTA_VARINT_I64 {
                            decode_delta_varint_i64_from_payload(
                                payload1,
                                row_count,
                                values.as_mut_slice(),
                            )?;
                        } else {
                            if payload1.len() != row_count * 8 {
                                return Err(Error::Other("values length mismatch".to_string()));
                            }
                            let byte_len =
                                checked_byte_len(row_count, size_of::<i64>(), "values overflow")?;
                            #[cfg(target_endian = "little")]
                            {
                                let out_bytes = unsafe {
                                    std::slice::from_raw_parts_mut(
                                        values.as_mut_ptr() as *mut u8,
                                        byte_len,
                                    )
                                };
                                out_bytes.copy_from_slice(payload1);
                                if enc == FixedEncodingId::PgBeFixed {
                                    for v in values.iter_mut() {
                                        *v = v.swap_bytes();
                                    }
                                }
                            }
                            #[cfg(not(target_endian = "little"))]
                            {
                                for i in 0..row_count {
                                    let j = i * 8;
                                    let b = [
                                        payload1[j],
                                        payload1[j + 1],
                                        payload1[j + 2],
                                        payload1[j + 3],
                                        payload1[j + 4],
                                        payload1[j + 5],
                                        payload1[j + 6],
                                        payload1[j + 7],
                                    ];
                                    values[i] = match enc {
                                        FixedEncodingId::PlainLe => i64::from_le_bytes(b),
                                        FixedEncodingId::PgBeFixed => i64::from_be_bytes(b),
                                    };
                                }
                            }
                        }
                    }
                    ColumnData::Var { .. } => {
                        return Err(Error::Other("internal type mismatch".to_string()));
                    }
                }
                columns.push(col);
            }
        }
    }

    if pos != bytes.len() {
        return Err(Error::Other("trailing bytes in ORSXCOL".to_string()));
    }

    let schema = ColumnarSchema::new(fields)?;
    Ok(ColumnarBatch {
        schema,
        row_capacity: row_count.max(1),
        row_count,
        columns,
    })
}

pub fn decode_orsxcol_v2_into(bytes: &[u8], out: &mut ColumnarBatch) -> Result<()> {
    let mut ws = OrsxcolV2DecodeWorkspace::default();
    decode_orsxcol_v2_into_with_workspace(bytes, out, &mut ws)
}

pub fn decode_orsxcol_v2_into_with_workspace(
    bytes: &[u8],
    out: &mut ColumnarBatch,
    ws: &mut OrsxcolV2DecodeWorkspace,
) -> Result<()> {
    let mut pos = 0usize;

    fn take<'a>(bytes: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8]> {
        let end = pos
            .checked_add(n)
            .ok_or_else(|| Error::Other("decode overflow".to_string()))?;
        if end > bytes.len() {
            return Err(Error::Other("truncated orscol".to_string()));
        }
        let slice = &bytes[*pos..end];
        *pos = end;
        Ok(slice)
    }

    fn read_u16_le(bytes: &[u8], pos: &mut usize) -> Result<u16> {
        let b = take(bytes, pos, 2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn read_u32_le(bytes: &[u8], pos: &mut usize) -> Result<u32> {
        let b = take(bytes, pos, 4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    // Header
    let magic = take(bytes, &mut pos, 8)?;
    if magic != MAGIC {
        return Err(Error::Other("bad ORSXCOL magic".to_string()));
    }
    let version = read_u16_le(bytes, &mut pos)?;
    if version != VERSION {
        return Err(Error::Other("unsupported ORSXCOL version".to_string()));
    }
    let _flags = read_u16_le(bytes, &mut pos)?;
    let row_count = read_u32_le(bytes, &mut pos)? as usize;
    let col_count = read_u16_le(bytes, &mut pos)? as usize;

    // v2 MVP: schema id not used; skip if present.
    let schema_id_len = read_u16_le(bytes, &mut pos)? as usize;
    if schema_id_len > 0 {
        let _ = take(bytes, &mut pos, schema_id_len)?;
    }

    if out.schema.len() != col_count {
        return Err(Error::Other(
            "decode_orsxcol_v2_into requires matching schema".to_string(),
        ));
    }

    let row_capacity = row_count.max(1);
    out.prepare(row_capacity)?;
    let expected_validity = ceil_div_8(row_count)?;

    for col_idx in 0..col_count {
        let tid = read_u16_le(bytes, &mut pos)?;
        let ty = type_from_id(tid)?;
        let encoding_id_u16 = read_u16_le(bytes, &mut pos)?;
        let _col_flags = read_u16_le(bytes, &mut pos)?;

        // Schema check: type + name.
        let expected_field = &out.schema.fields()[col_idx];
        if expected_field.ty != ty {
            return Err(Error::Other(
                "decode_orsxcol_v2_into requires matching schema".to_string(),
            ));
        }

        let name_len = read_u16_le(bytes, &mut pos)? as usize;
        let name_bytes = take(bytes, &mut pos, name_len)?;
        match expected_field.name.as_deref() {
            None => {
                if name_len != 0 {
                    return Err(Error::Other(
                        "decode_orsxcol_v2_into requires matching schema".to_string(),
                    ));
                }
            }
            Some(expected) => {
                if name_bytes != expected.as_bytes() {
                    return Err(Error::Other(
                        "decode_orsxcol_v2_into requires matching schema".to_string(),
                    ));
                }
            }
        }

        let validity_len = read_u32_le(bytes, &mut pos)? as usize;
        if validity_len != expected_validity {
            return Err(Error::Other("validity length mismatch".to_string()));
        }
        let validity = take(bytes, &mut pos, validity_len)?;

        let payload1_len = read_u32_le(bytes, &mut pos)? as usize;
        let payload1 = take(bytes, &mut pos, payload1_len)?;
        let payload2_len = read_u32_le(bytes, &mut pos)? as usize;
        let payload2 = take(bytes, &mut pos, payload2_len)?;

        // Fill column in-place.
        let col = out
            .columns
            .get_mut(col_idx)
            .ok_or_else(|| Error::Other("column index out of bounds".to_string()))?;

        match ty {
            ColumnarType::Utf8 | ColumnarType::Bytes | ColumnarType::JsonbText => match encoding_id_u16 {
                ENC_PLAIN => {
                    let expected_offsets_len = (row_count + 1)
                        .checked_mul(4)
                        .ok_or_else(|| Error::Other("offsets overflow".to_string()))?;
                    if payload1.len() != expected_offsets_len {
                        return Err(Error::Other("offsets length mismatch".to_string()));
                    }

                    if let ColumnData::Var {
                        validity: v,
                        offsets: o,
                        data,
                        chunks,
                        total_len,
                        ..
                    } = col
                    {
                        // validity
                        if validity_len > 0 {
                            v.bytes[..validity_len].copy_from_slice(validity);
                        }

                        // offsets
                        o.clear();
                        o.resize(row_count + 1, 0u32);
                        #[cfg(target_endian = "little")]
                        {
                            let out_bytes = unsafe {
                                std::slice::from_raw_parts_mut(
                                    o.as_mut_ptr() as *mut u8,
                                    expected_offsets_len,
                                )
                            };
                            out_bytes.copy_from_slice(payload1);
                        }
                        #[cfg(not(target_endian = "little"))]
                        {
                            for i in 0..(row_count + 1) {
                                let j = i * 4;
                                o[i] = u32::from_le_bytes([
                                    payload1[j],
                                    payload1[j + 1],
                                    payload1[j + 2],
                                    payload1[j + 3],
                                ]);
                            }
                        }

                        // validate offsets
                        let mut prev = 0u32;
                        for &off in o.iter() {
                            if off < prev {
                                return Err(Error::Other(
                                    "offsets must be non-decreasing".to_string(),
                                ));
                            }
                            prev = off;
                        }
                        let final_off = *o.last().unwrap_or(&0);
                        let data_len_u32: u32 = payload2
                            .len()
                            .try_into()
                            .map_err(|_| Error::Other("data too large".to_string()))?;
                        if final_off != data_len_u32 {
                            return Err(Error::Other("final offset mismatch".to_string()));
                        }

                        // data
                        data.clear();
                        data.extend_from_slice(payload2);
                        chunks.clear();
                        *total_len = data_len_u32;
                    } else {
                        return Err(Error::Other("internal type mismatch".to_string()));
                    }
                }
                ENC_DICT_UTF8 => {
                    if ty == ColumnarType::Bytes {
                        return Err(Error::Other("DictUtf8 is not supported for Bytes".to_string()));
                    }
                    if let ColumnData::Var {
                        validity: v,
                        offsets: o,
                        data,
                        chunks,
                        total_len,
                        ..
                    } = col
                    {
                        if validity_len > 0 {
                            v.bytes[..validity_len].copy_from_slice(validity);
                        }
                        decode_dict_utf8_to_var_col(ws, row_count, &v.bytes[..validity_len], payload1, payload2, o, data)?;
                        chunks.clear();
                        *total_len = data
                            .len()
                            .try_into()
                            .map_err(|_| Error::Other("data too large".to_string()))?;
                    } else {
                        return Err(Error::Other("internal type mismatch".to_string()));
                    }
                }
                _ => return Err(Error::Other("invalid encoding for varlen column".to_string())),
            },
            _ => {
                if encoding_id_u16 == ENC_DELTA_VARINT_I64
                    && !(ty == ColumnarType::I64 || ty == ColumnarType::TimestampTzMicros)
                {
                    return Err(Error::Other("invalid encoding for fixed column".to_string()));
                }
                let enc = if encoding_id_u16 == ENC_DELTA_VARINT_I64 {
                    FixedEncodingId::PlainLe
                } else {
                    FixedEncodingId::from_u16(encoding_id_u16).ok_or_else(|| {
                        Error::Other("invalid encoding for fixed column".to_string())
                    })?
                };
                if payload2_len != 0 {
                    return Err(Error::Other("fixed-width payload_2 must be empty".to_string()));
                }

                match col {
                    ColumnData::FixedBool { validity: v, values } => {
                        if validity_len > 0 {
                            v.bytes[..validity_len].copy_from_slice(validity);
                        }
                        if payload1.len() != row_count {
                            return Err(Error::Other("values length mismatch".to_string()));
                        }
                        values.clear();
                        values.extend_from_slice(payload1);
                        let _ = enc;
                    }
                    ColumnData::FixedI16 { validity: v, values } => {
                        if validity_len > 0 {
                            v.bytes[..validity_len].copy_from_slice(validity);
                        }
                        if payload1.len() != checked_byte_len(row_count, size_of::<i16>(), "values overflow")? {
                            return Err(Error::Other("values length mismatch".to_string()));
                        }
                        values.clear();
                        values.resize(row_count, 0i16);
                        let byte_len = checked_byte_len(row_count, size_of::<i16>(), "values overflow")?;
                        #[cfg(target_endian = "little")]
                        {
                            let out_bytes = unsafe {
                                std::slice::from_raw_parts_mut(
                                    values.as_mut_ptr() as *mut u8,
                                    byte_len,
                                )
                            };
                            out_bytes.copy_from_slice(payload1);
                            if enc == FixedEncodingId::PgBeFixed {
                                for v in values.iter_mut() {
                                    *v = v.swap_bytes();
                                }
                            }
                        }
                        #[cfg(not(target_endian = "little"))]
                        {
                            for i in 0..row_count {
                                let j = i * 2;
                                let b = [payload1[j], payload1[j + 1]];
                                values[i] = match enc {
                                    FixedEncodingId::PlainLe => i16::from_le_bytes(b),
                                    FixedEncodingId::PgBeFixed => i16::from_be_bytes(b),
                                };
                            }
                        }
                    }
                    ColumnData::FixedI32 { validity: v, values } => {
                        if validity_len > 0 {
                            v.bytes[..validity_len].copy_from_slice(validity);
                        }
                        if payload1.len() != checked_byte_len(row_count, size_of::<i32>(), "values overflow")? {
                            return Err(Error::Other("values length mismatch".to_string()));
                        }
                        values.clear();
                        values.resize(row_count, 0i32);
                        let byte_len = checked_byte_len(row_count, size_of::<i32>(), "values overflow")?;
                        #[cfg(target_endian = "little")]
                        {
                            let out_bytes = unsafe {
                                std::slice::from_raw_parts_mut(
                                    values.as_mut_ptr() as *mut u8,
                                    byte_len,
                                )
                            };
                            out_bytes.copy_from_slice(payload1);
                            if enc == FixedEncodingId::PgBeFixed {
                                for v in values.iter_mut() {
                                    *v = v.swap_bytes();
                                }
                            }
                        }
                        #[cfg(not(target_endian = "little"))]
                        {
                            for i in 0..row_count {
                                let j = i * 4;
                                let b = [payload1[j], payload1[j + 1], payload1[j + 2], payload1[j + 3]];
                                values[i] = match enc {
                                    FixedEncodingId::PlainLe => i32::from_le_bytes(b),
                                    FixedEncodingId::PgBeFixed => i32::from_be_bytes(b),
                                };
                            }
                        }
                    }
                    ColumnData::FixedI64 { validity: v, values } => {
                        if validity_len > 0 {
                            v.bytes[..validity_len].copy_from_slice(validity);
                        }
                        values.clear();
                        values.resize(row_count, 0i64);
                        if encoding_id_u16 == ENC_DELTA_VARINT_I64 {
                            decode_delta_varint_i64_from_payload(
                                payload1,
                                row_count,
                                values.as_mut_slice(),
                            )?;
                        } else {
                            if payload1.len()
                                != checked_byte_len(row_count, size_of::<i64>(), "values overflow")?
                            {
                                return Err(Error::Other("values length mismatch".to_string()));
                            }
                            let byte_len =
                                checked_byte_len(row_count, size_of::<i64>(), "values overflow")?;
                            #[cfg(target_endian = "little")]
                            {
                                let out_bytes = unsafe {
                                    std::slice::from_raw_parts_mut(
                                        values.as_mut_ptr() as *mut u8,
                                        byte_len,
                                    )
                                };
                                out_bytes.copy_from_slice(payload1);
                                if enc == FixedEncodingId::PgBeFixed {
                                    for v in values.iter_mut() {
                                        *v = v.swap_bytes();
                                    }
                                }
                            }
                            #[cfg(not(target_endian = "little"))]
                            {
                                for i in 0..row_count {
                                    let j = i * 8;
                                    let b = [
                                        payload1[j],
                                        payload1[j + 1],
                                        payload1[j + 2],
                                        payload1[j + 3],
                                        payload1[j + 4],
                                        payload1[j + 5],
                                        payload1[j + 6],
                                        payload1[j + 7],
                                    ];
                                    values[i] = match enc {
                                        FixedEncodingId::PlainLe => i64::from_le_bytes(b),
                                        FixedEncodingId::PgBeFixed => i64::from_be_bytes(b),
                                    };
                                }
                            }
                        }
                    }
                    ColumnData::FixedF32Bits { validity: v, values } => {
                        if validity_len > 0 {
                            v.bytes[..validity_len].copy_from_slice(validity);
                        }
                        if payload1.len() != checked_byte_len(row_count, size_of::<u32>(), "values overflow")? {
                            return Err(Error::Other("values length mismatch".to_string()));
                        }
                        values.clear();
                        values.resize(row_count, 0u32);
                        let byte_len = checked_byte_len(row_count, size_of::<u32>(), "values overflow")?;
                        #[cfg(target_endian = "little")]
                        {
                            let out_bytes = unsafe {
                                std::slice::from_raw_parts_mut(
                                    values.as_mut_ptr() as *mut u8,
                                    byte_len,
                                )
                            };
                            out_bytes.copy_from_slice(payload1);
                            if enc == FixedEncodingId::PgBeFixed {
                                for v in values.iter_mut() {
                                    *v = v.swap_bytes();
                                }
                            }
                        }
                        #[cfg(not(target_endian = "little"))]
                        {
                            for i in 0..row_count {
                                let j = i * 4;
                                let b = [payload1[j], payload1[j + 1], payload1[j + 2], payload1[j + 3]];
                                values[i] = match enc {
                                    FixedEncodingId::PlainLe => u32::from_le_bytes(b),
                                    FixedEncodingId::PgBeFixed => u32::from_be_bytes(b),
                                };
                            }
                        }
                    }
                    ColumnData::FixedF64Bits { validity: v, values } => {
                        if validity_len > 0 {
                            v.bytes[..validity_len].copy_from_slice(validity);
                        }
                        if payload1.len() != checked_byte_len(row_count, size_of::<u64>(), "values overflow")? {
                            return Err(Error::Other("values length mismatch".to_string()));
                        }
                        values.clear();
                        values.resize(row_count, 0u64);
                        let byte_len = checked_byte_len(row_count, size_of::<u64>(), "values overflow")?;
                        #[cfg(target_endian = "little")]
                        {
                            let out_bytes = unsafe {
                                std::slice::from_raw_parts_mut(
                                    values.as_mut_ptr() as *mut u8,
                                    byte_len,
                                )
                            };
                            out_bytes.copy_from_slice(payload1);
                            if enc == FixedEncodingId::PgBeFixed {
                                for v in values.iter_mut() {
                                    *v = v.swap_bytes();
                                }
                            }
                        }
                        #[cfg(not(target_endian = "little"))]
                        {
                            for i in 0..row_count {
                                let j = i * 8;
                                let b = [
                                    payload1[j],
                                    payload1[j + 1],
                                    payload1[j + 2],
                                    payload1[j + 3],
                                    payload1[j + 4],
                                    payload1[j + 5],
                                    payload1[j + 6],
                                    payload1[j + 7],
                                ];
                                values[i] = match enc {
                                    FixedEncodingId::PlainLe => u64::from_le_bytes(b),
                                    FixedEncodingId::PgBeFixed => u64::from_be_bytes(b),
                                };
                            }
                        }
                    }
                    ColumnData::FixedUuid { validity: v, values } => {
                        if validity_len > 0 {
                            v.bytes[..validity_len].copy_from_slice(validity);
                        }
                        if payload1.len() != checked_byte_len(row_count, 16, "values overflow")? {
                            return Err(Error::Other("values length mismatch".to_string()));
                        }
                        values.clear();
                        values.resize(row_count, [0u8; 16]);
                        let byte_len = checked_byte_len(row_count, 16, "values overflow")?;
                        let out_bytes = unsafe {
                            std::slice::from_raw_parts_mut(values.as_mut_ptr() as *mut u8, byte_len)
                        };
                        out_bytes.copy_from_slice(payload1);
                        let _ = enc;
                    }
                    ColumnData::FixedTimestampMicros { validity: v, values } => {
                        if validity_len > 0 {
                            v.bytes[..validity_len].copy_from_slice(validity);
                        }
                        values.clear();
                        values.resize(row_count, 0i64);
                        if encoding_id_u16 == ENC_DELTA_VARINT_I64 {
                            decode_delta_varint_i64_from_payload(
                                payload1,
                                row_count,
                                values.as_mut_slice(),
                            )?;
                        } else {
                            if payload1.len()
                                != checked_byte_len(row_count, size_of::<i64>(), "values overflow")?
                            {
                                return Err(Error::Other("values length mismatch".to_string()));
                            }
                            let byte_len =
                                checked_byte_len(row_count, size_of::<i64>(), "values overflow")?;
                            #[cfg(target_endian = "little")]
                            {
                                let out_bytes = unsafe {
                                    std::slice::from_raw_parts_mut(
                                        values.as_mut_ptr() as *mut u8,
                                        byte_len,
                                    )
                                };
                                out_bytes.copy_from_slice(payload1);
                                if enc == FixedEncodingId::PgBeFixed {
                                    for v in values.iter_mut() {
                                        *v = v.swap_bytes();
                                    }
                                }
                            }
                            #[cfg(not(target_endian = "little"))]
                            {
                                for i in 0..row_count {
                                    let j = i * 8;
                                    let b = [
                                        payload1[j],
                                        payload1[j + 1],
                                        payload1[j + 2],
                                        payload1[j + 3],
                                        payload1[j + 4],
                                        payload1[j + 5],
                                        payload1[j + 6],
                                        payload1[j + 7],
                                    ];
                                    values[i] = match enc {
                                        FixedEncodingId::PlainLe => i64::from_le_bytes(b),
                                        FixedEncodingId::PgBeFixed => i64::from_be_bytes(b),
                                    };
                                }
                            }
                        }
                    }
                    ColumnData::Var { .. } => {
                        return Err(Error::Other("internal type mismatch".to_string()));
                    }
                }
            }
        }
    }

    if pos != bytes.len() {
        return Err(Error::Other("trailing bytes in ORSXCOL".to_string()));
    }

    out.row_count = row_count;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flip_last_byte(mut v: Vec<u8>) -> Vec<u8> {
        if let Some(last) = v.last_mut() {
            *last ^= 0xAA;
        }
        v
    }

    #[test]
    fn orscol2_roundtrip_mixed_types_with_nulls() {
        let row_count = 3usize;
        let schema = ColumnarSchema::new(vec![
            ColumnarField {
                name: Some("a".to_string()),
                ty: ColumnarType::I64,
            },
            ColumnarField {
                name: Some("b".to_string()),
                ty: ColumnarType::Utf8,
            },
            ColumnarField {
                name: Some("c".to_string()),
                ty: ColumnarType::Bytes,
            },
        ])
        .unwrap();

        let mut c0 = ColumnData::new(ColumnarType::I64).unwrap();
        let mut c1 = ColumnData::new(ColumnarType::Utf8).unwrap();
        let mut c2 = ColumnData::new(ColumnarType::Bytes).unwrap();

        if let ColumnData::FixedI64 { validity, values } = &mut c0 {
            validity.bytes = vec![0b0000_0101]; // rows 0 and 2 valid
            values.push(10);
            values.push(0);
            values.push(30);
        } else {
            panic!("expected fixed");
        }

        if let ColumnData::Var {
            validity,
            offsets,
            data,
            chunks,
            total_len,
            ..
        } = &mut c1
        {
            validity.bytes = vec![0b0000_0111]; // all valid
            offsets.clear();
            offsets.extend_from_slice(&[0, 1, 3, 3]);
            data.clear();
            chunks.clear();
            chunks.push(bytes::Bytes::copy_from_slice(b"x"));
            chunks.push(bytes::Bytes::copy_from_slice(b"yz"));
            *total_len = 3;
        } else {
            panic!("expected var");
        }

        if let ColumnData::Var {
            validity,
            offsets,
            data,
            chunks,
            total_len,
            ..
        } = &mut c2
        {
            validity.bytes = vec![0b0000_0010]; // only row 1 valid
            offsets.clear();
            offsets.extend_from_slice(&[0, 0, 2, 2]);
            data.clear();
            chunks.clear();
            chunks.push(bytes::Bytes::copy_from_slice(&[0xAA, 0xBB]));
            *total_len = 2;
        } else {
            panic!("expected var");
        }

        let batch = ColumnarBatch {
            schema,
            row_capacity: row_count,
            row_count,
            columns: vec![c0, c1, c2],
        };

        let mut encoded = Vec::new();
        encode_orsxcol_v2_into(&batch, &mut encoded).unwrap();
        let decoded = decode_orsxcol_v2(&encoded).unwrap();

        assert_eq!(decoded.row_count, 3);
        assert_eq!(decoded.schema, batch.schema);
        assert_eq!(decoded.columns.len(), batch.columns.len());

        for (a, b) in batch.columns.iter().zip(decoded.columns.iter()) {
            match (a, b) {
                (
                    ColumnData::FixedI64 {
                        validity: v_a,
                        values: vals_a,
                    },
                    ColumnData::FixedI64 {
                        validity: v_b,
                        values: vals_b,
                    },
                ) => {
                    assert_eq!(v_a.bytes, v_b.bytes);
                    assert_eq!(vals_a, vals_b);
                }
                (
                    ColumnData::Var {
                        ty: ty_a,
                        validity: v_a,
                        offsets: o_a,
                        data: d_a,
                        chunks: c_a,
                        total_len: t_a,
                    },
                    ColumnData::Var {
                        ty: ty_b,
                        validity: v_b,
                        offsets: o_b,
                        data: d_b,
                        chunks: c_b,
                        total_len: t_b,
                    },
                ) => {
                    assert_eq!(ty_a, ty_b);
                    assert_eq!(v_a.bytes, v_b.bytes);
                    assert_eq!(o_a, o_b);
                    assert_eq!(t_a, t_b);
                    let mut a_bytes: Vec<u8> = Vec::new();
                    a_bytes.extend_from_slice(d_a.as_slice());
                    a_bytes.extend(c_a.iter().flat_map(|x| x.as_ref().iter().copied()));
                    let mut b_bytes: Vec<u8> = Vec::new();
                    b_bytes.extend_from_slice(d_b.as_slice());
                    b_bytes.extend(c_b.iter().flat_map(|x| x.as_ref().iter().copied()));
                    assert_eq!(a_bytes, b_bytes);
                }
                _ => panic!("type mismatch"),
            }
        }
    }

    #[test]
    fn orscol2_rejects_bad_magic() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"NOTMAGIC");
        bytes.extend_from_slice(&[0u8; 64]);
        let err = decode_orsxcol_v2(&bytes).unwrap_err();
        assert!(err.to_string().contains("magic"));
    }

    #[test]
    fn orscol2_rejects_truncated_header() {
        let bytes = b"ORSXCOL2";
        let err = decode_orsxcol_v2(bytes).unwrap_err();
        assert!(err.to_string().contains("truncated"));
    }

    #[test]
    fn orscol2_rejects_trailing_bytes() {
        let row_count = 1usize;
        let schema = ColumnarSchema::new(vec![ColumnarField {
            name: Some("a".to_string()),
            ty: ColumnarType::I64,
        }])
        .unwrap();
        let mut c0 = ColumnData::new(ColumnarType::I64).unwrap();
        if let ColumnData::FixedI64 { validity, values } = &mut c0 {
            validity.bytes = vec![0b0000_0001];
            values.push(7);
        }
        let batch = ColumnarBatch {
            schema,
            row_capacity: row_count,
            row_count,
            columns: vec![c0],
        };
        let mut encoded = Vec::new();
        encode_orsxcol_v2_into(&batch, &mut encoded).unwrap();
        encoded.push(0x00);
        let err = decode_orsxcol_v2(&encoded).unwrap_err();
        assert!(err.to_string().contains("trailing"));
    }

    #[test]
    fn orscol2_rejects_bad_offsets() {
        let row_count = 3usize;
        let schema = ColumnarSchema::new(vec![ColumnarField {
            name: Some("a".to_string()),
            ty: ColumnarType::Utf8,
        }])
        .unwrap();

        let mut c0 = ColumnData::new(ColumnarType::Utf8).unwrap();
        if let ColumnData::Var {
            validity,
            offsets,
            data,
            chunks,
            total_len,
            ..
        } = &mut c0
        {
            validity.bytes = vec![0b0000_0111];
            offsets.clear();
            offsets.extend_from_slice(&[0, 2, 1, 2]); // decreasing
            data.clear();
            chunks.clear();
            chunks.push(bytes::Bytes::copy_from_slice(b"ab"));
            *total_len = 2;
        }

        let batch = ColumnarBatch {
            schema,
            row_capacity: row_count,
            row_count,
            columns: vec![c0],
        };

        let mut encoded = Vec::new();
        encode_orsxcol_v2_into(&batch, &mut encoded).unwrap();
        let err = decode_orsxcol_v2(&encoded).unwrap_err();
        assert!(err.to_string().contains("offsets"), "unexpected: {err}");
    }

    #[test]
    fn orscol2_rejects_final_offset_mismatch() {
        // Manually construct a varlen column where offsets claim more data than present.
        let row_count = 2u32;
        let col_count = 1u16;

        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        write_u16_le(&mut out, VERSION);
        write_u16_le(&mut out, 0);
        write_u32_le(&mut out, row_count);
        write_u16_le(&mut out, col_count);
        write_u16_le(&mut out, 0); // schema_id_len

        write_u16_le(&mut out, type_id(ColumnarType::Bytes));
        write_u16_le(&mut out, 0); // PlainVar
        write_u16_le(&mut out, 0); // col_flags
        write_u16_len_bytes(&mut out, b"a").unwrap();
        write_u32_len_bytes(&mut out, &[0b0000_0011]).unwrap(); // validity

        // payload_1 offsets: [0, 1, 9] (final offset=9)
        write_u32_le(&mut out, 12);
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&9u32.to_le_bytes());

        // payload_2 data length is 2 bytes -> final offset mismatch
        write_u32_le(&mut out, 2);
        out.extend_from_slice(&[0xAB, 0xCD]);

        let err = decode_orsxcol_v2(&out).unwrap_err();
        assert!(
            err.to_string().contains("final offset"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn orscol2_rejects_invalid_fixed_payload2() {
        let row_count = 1usize;
        let schema = ColumnarSchema::new(vec![ColumnarField {
            name: Some("a".to_string()),
            ty: ColumnarType::I32,
        }])
        .unwrap();
        let mut c0 = ColumnData::new(ColumnarType::I32).unwrap();
        if let ColumnData::FixedI32 { validity, values } = &mut c0 {
            validity.bytes = vec![0b0000_0001];
            values.push(1);
        }
        let batch = ColumnarBatch {
            schema,
            row_capacity: row_count,
            row_count,
            columns: vec![c0],
        };
        let mut encoded = Vec::new();
        encode_orsxcol_v2_into(&batch, &mut encoded).unwrap();

        // Corrupt by flipping the last byte: almost certainly changes some length or payload.
        let corrupted = flip_last_byte(encoded);
        let _ = decode_orsxcol_v2(&corrupted).unwrap_err();
    }

    #[test]
    fn orscol2_decodes_pg_be_fixed_for_i64() {
        // Construct a minimal ORSXCOL2 buffer manually with:
        // - one i64 column
        // - encoding_id=PgBeFixed
        // - value encoded big-endian
        let row_count = 1u32;
        let col_count = 1u16;

        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        write_u16_le(&mut out, VERSION);
        write_u16_le(&mut out, 0);
        write_u32_le(&mut out, row_count);
        write_u16_le(&mut out, col_count);
        write_u16_le(&mut out, 0); // schema_id_len

        write_u16_le(&mut out, type_id(ColumnarType::I64));
        write_u16_le(&mut out, FixedEncodingId::PgBeFixed as u16);
        write_u16_le(&mut out, 0); // col_flags
        write_u16_len_bytes(&mut out, b"a").unwrap();
        // validity (1 row -> 1 byte)
        write_u32_len_bytes(&mut out, &[0b0000_0001]).unwrap();
        // payload_1: 8 bytes
        write_u32_le(&mut out, 8);
        out.extend_from_slice(&7_i64.to_be_bytes());
        // payload_2: empty
        write_u32_le(&mut out, 0);

        let decoded = decode_orsxcol_v2(&out).unwrap();
        assert_eq!(decoded.row_count, 1);
        assert_eq!(decoded.schema.fields()[0].name.as_deref(), Some("a"));
        let vals = decoded.fixed_i64(0).unwrap();
        assert_eq!(vals[0], 7);
    }
}
