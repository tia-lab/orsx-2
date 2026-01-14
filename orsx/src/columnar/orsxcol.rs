use crate::{Error, Result};

use super::types::{ColumnarBatch, ColumnarField, ColumnarSchema, ColumnarType, ColumnData, FixedEncoding};

const MAGIC: &[u8; 8] = b"ORSXCOL1";
const VERSION: u16 = 1;

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

fn write_bytes_len_u32(out: &mut Vec<u8>, bytes: &[u8]) -> Result<()> {
    let len: u32 = bytes
        .len()
        .try_into()
        .map_err(|_| Error::Other("payload too large".to_string()))?;
    write_u32_le(out, len);
    out.extend_from_slice(bytes);
    Ok(())
}

pub fn encode_orsxcol_v1_into(batch: &ColumnarBatch, out: &mut Vec<u8>) -> Result<()> {
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

    let expected_validity = ceil_div_8(batch.row_count)?;

    for (field, col) in batch.schema.fields().iter().zip(batch.columns.iter()) {
        let tid = type_id(field.ty);
        write_u16_le(out, tid);
        let encoding_id: u16 = match col {
            ColumnData::Fixed { encoding, ty, .. } => match encoding {
                FixedEncoding::Le => 0,
                FixedEncoding::PgBe => match ty {
                    ColumnarType::I16
                    | ColumnarType::I32
                    | ColumnarType::I64
                    | ColumnarType::F32
                    | ColumnarType::F64 => 1,
                    _ => {
                        return Err(Error::Other(
                            "PgBe encoding only supported for numeric fixed-width columns".to_string(),
                        ))
                    }
                },
            },
            ColumnData::Var { .. } => 0,
        };
        write_u16_le(out, encoding_id);

        let name_bytes = field.name.as_deref().unwrap_or("").as_bytes();
        let name_len: u16 = name_bytes
            .len()
            .try_into()
            .map_err(|_| Error::Other("column name too long".to_string()))?;
        write_u16_le(out, name_len);
        out.extend_from_slice(name_bytes);

        let validity: &[u8] = match col {
            ColumnData::Fixed { validity, .. } => validity.bytes.as_slice(),
            ColumnData::Var { validity, .. } => validity.bytes.as_slice(),
        };
        if validity.len() != expected_validity {
            return Err(Error::Other("validity length mismatch".to_string()));
        }
        write_bytes_len_u32(out, validity)?;

        match col {
            ColumnData::Fixed { values, .. } => {
                write_bytes_len_u32(out, values)?;
                write_u32_le(out, 0);
            }
            ColumnData::Var { offsets, data, .. } => {
                let mut offsets_bytes = Vec::new();
                offsets_bytes.reserve(
                    offsets
                        .len()
                        .checked_mul(4)
                        .ok_or_else(|| Error::Other("offsets overflow".to_string()))?,
                );
                for &o in offsets {
                    offsets_bytes.extend_from_slice(&o.to_le_bytes());
                }
                write_bytes_len_u32(out, &offsets_bytes)?;
                write_bytes_len_u32(out, data)?;
            }
        }
    }

    Ok(())
}

pub fn decode_orsxcol_v1(bytes: &[u8]) -> Result<ColumnarBatch> {
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

    let expected_validity = ceil_div_8(row_count)?;

    let mut fields = Vec::with_capacity(col_count);
    let mut columns = Vec::with_capacity(col_count);

    for _ in 0..col_count {
        let tid = read_u16_le(bytes, &mut pos)?;
        let ty = type_from_id(tid)?;
        let encoding_id = read_u16_le(bytes, &mut pos)?;

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
        let payload1 = take(bytes, &mut pos, payload1_len)?.to_vec();
        let payload2_len = read_u32_le(bytes, &mut pos)? as usize;
        let payload2 = take(bytes, &mut pos, payload2_len)?.to_vec();

        fields.push(ColumnarField { name, ty });

        match ty {
            ColumnarType::Utf8 | ColumnarType::Bytes => {
                if encoding_id != 0 {
                    return Err(Error::Other("invalid encoding for varlen column".to_string()));
                }
                let expected_offsets_len = (row_count + 1)
                    .checked_mul(4)
                    .ok_or_else(|| Error::Other("offsets overflow".to_string()))?;
                if payload1.len() != expected_offsets_len {
                    return Err(Error::Other("offsets length mismatch".to_string()));
                }
                let mut offsets = Vec::with_capacity(row_count + 1);
                for i in 0..(row_count + 1) {
                    let j = i * 4;
                    let v = u32::from_le_bytes([
                        payload1[j],
                        payload1[j + 1],
                        payload1[j + 2],
                        payload1[j + 3],
                    ]);
                    offsets.push(v);
                }
                // Validate non-decreasing and final == data.len()
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
                    data: d,
                    ..
                } = &mut col
                {
                    v.bytes = validity;
                    *o = offsets;
                    *d = payload2;
                }
                columns.push(col);
            }
            _ => {
                if payload2_len != 0 {
                    return Err(Error::Other("fixed-width payload_2 must be empty".to_string()));
                }
                let mut col = ColumnData::new(ty)?;
                if let ColumnData::Fixed {
                    width,
                    encoding,
                    validity: v,
                    values,
                    ..
                } = &mut col
                {
                    v.bytes = validity;
                    *encoding = match encoding_id {
                        0 => FixedEncoding::Le,
                        1 => match ty {
                            ColumnarType::I16
                            | ColumnarType::I32
                            | ColumnarType::I64
                            | ColumnarType::F32
                            | ColumnarType::F64 => FixedEncoding::PgBe,
                            _ => {
                                return Err(Error::Other(
                                    "PgBe encoding only supported for numeric fixed-width columns"
                                        .to_string(),
                                ))
                            }
                        },
                        _ => return Err(Error::Other("unknown fixed encoding id".to_string())),
                    };
                    let expected_values_len = row_count
                        .checked_mul(*width)
                        .ok_or_else(|| Error::Other("values overflow".to_string()))?;
                    if payload1.len() != expected_values_len {
                        return Err(Error::Other("values length mismatch".to_string()));
                    }
                    *values = payload1;
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

pub fn decode_orsxcol_v1_into(bytes: &[u8], out: &mut ColumnarBatch) -> Result<()> {
    let decoded = decode_orsxcol_v1(bytes)?;
    if decoded.schema != out.schema {
        return Err(Error::Other(
            "decode_orsxcol_v1_into requires matching schema".to_string(),
        ));
    }
    *out = decoded;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le_i64(v: i64) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    #[test]
    fn orscol_roundtrip_mixed_types_with_nulls() {
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

        if let ColumnData::Fixed {
            encoding,
            validity,
            values,
            ..
        } = &mut c0
        {
            *encoding = FixedEncoding::Le;
            validity.bytes = vec![0b0000_0101]; // rows 0 and 2 valid
            values.extend_from_slice(&le_i64(10));
            values.extend_from_slice(&le_i64(0)); // null placeholder
            values.extend_from_slice(&le_i64(30));
        } else {
            panic!("expected fixed");
        }

        if let ColumnData::Var {
            validity,
            offsets,
            data,
            ..
        } = &mut c1
        {
            validity.bytes = vec![0b0000_0111]; // all valid
            offsets.clear();
            offsets.extend_from_slice(&[0, 1, 3, 3]);
            data.extend_from_slice(b"x");
            data.extend_from_slice(b"yz");
        } else {
            panic!("expected var");
        }

        if let ColumnData::Var {
            validity,
            offsets,
            data,
            ..
        } = &mut c2
        {
            validity.bytes = vec![0b0000_0010]; // only row 1 valid
            offsets.clear();
            offsets.extend_from_slice(&[0, 0, 2, 2]);
            data.extend_from_slice(&[0xAA, 0xBB]);
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
        encode_orsxcol_v1_into(&batch, &mut encoded).unwrap();
        let decoded = decode_orsxcol_v1(&encoded).unwrap();

        assert_eq!(decoded.row_count, 3);
        assert_eq!(decoded.schema, batch.schema);
        assert_eq!(decoded.columns.len(), batch.columns.len());
        for (a, b) in batch.columns.iter().zip(decoded.columns.iter()) {
            match (a, b) {
                (
                    ColumnData::Fixed {
                        ty: ty_a,
                        encoding: e_a,
                        width: w_a,
                        validity: v_a,
                        values: vals_a,
                    },
                    ColumnData::Fixed {
                        ty: ty_b,
                        encoding: e_b,
                        width: w_b,
                        validity: v_b,
                        values: vals_b,
                    },
                ) => {
                    assert_eq!(ty_a, ty_b);
                    assert_eq!(e_a, e_b);
                    assert_eq!(w_a, w_b);
                    assert_eq!(v_a.bytes, v_b.bytes);
                    assert_eq!(vals_a, vals_b);
                }
                (
                    ColumnData::Var {
                        ty: ty_a,
                        validity: v_a,
                        offsets: o_a,
                        data: d_a,
                    },
                    ColumnData::Var {
                        ty: ty_b,
                        validity: v_b,
                        offsets: o_b,
                        data: d_b,
                    },
                ) => {
                    assert_eq!(ty_a, ty_b);
                    assert_eq!(v_a.bytes, v_b.bytes);
                    assert_eq!(o_a, o_b);
                    assert_eq!(d_a, d_b);
                }
                _ => panic!("type mismatch"),
            }
        }
    }

    #[test]
    fn orscol_rejects_bad_offsets() {
        let row_count = 2usize;
        let schema = ColumnarSchema::new(vec![ColumnarField {
            name: None,
            ty: ColumnarType::Utf8,
        }])
        .unwrap();

        let mut c0 = ColumnData::new(ColumnarType::Utf8).unwrap();
        if let ColumnData::Var {
            validity,
            offsets,
            data,
            ..
        } = &mut c0
        {
            validity.bytes = vec![0b0000_0011];
            offsets.clear();
            offsets.extend_from_slice(&[0, 5, 4]); // decreasing
            data.extend_from_slice(b"hello");
        } else {
            panic!("expected var");
        }

        let batch = ColumnarBatch {
            schema,
            row_capacity: row_count,
            row_count,
            columns: vec![c0],
        };

        let mut encoded = Vec::new();
        encode_orsxcol_v1_into(&batch, &mut encoded).unwrap();
        let err = decode_orsxcol_v1(&encoded).unwrap_err();
        assert!(err.to_string().contains("offsets"), "unexpected: {err}");
    }
}
