use crate::{Error, Result};
use crc32fast::Hasher;

pub const ORSX_MAGIC: &[u8; 4] = b"ORSX";
pub const ORSX_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecId {
    CydecInteger = 1,
    CydecFloat = 2,
}

impl CodecId {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(CodecId::CydecInteger),
            2 => Some(CodecId::CydecFloat),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElemTypeId {
    I32 = 1,
    I64 = 2,
    U32 = 3,
    U64 = 4,
    F32 = 5,
    F64 = 6,
}

impl ElemTypeId {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(ElemTypeId::I32),
            2 => Some(ElemTypeId::I64),
            3 => Some(ElemTypeId::U32),
            4 => Some(ElemTypeId::U64),
            5 => Some(ElemTypeId::F32),
            6 => Some(ElemTypeId::F64),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvelopeHeader {
    pub version: u8,
    pub codec: CodecId,
    pub elem_type: ElemTypeId,
    pub elem_count: u32,
    pub uncompressed_len: u32,
    pub checksum_crc32: u32,
}

// Layout (little-endian):
// magic[4] | version[1] | codec[1] | elem_type[1] | reserved[1] |
// elem_count[4] | uncompressed_len[4] | checksum_crc32[4] | payload...
pub const HEADER_LEN: usize = 4 + 1 + 1 + 1 + 1 + 4 + 4 + 4;

pub fn build_envelope(
    codec: CodecId,
    elem_type: ElemTypeId,
    elem_count: u32,
    uncompressed_len: u32,
    payload: &[u8],
    out: &mut Vec<u8>,
) {
    out.clear();
    out.reserve(HEADER_LEN + payload.len());

    out.extend_from_slice(ORSX_MAGIC);
    out.push(ORSX_VERSION);
    out.push(codec as u8);
    out.push(elem_type as u8);
    out.push(0); // reserved
    out.extend_from_slice(&elem_count.to_le_bytes());
    out.extend_from_slice(&uncompressed_len.to_le_bytes());

    let checksum = crc32_payload(payload);
    out.extend_from_slice(&checksum.to_le_bytes());
    out.extend_from_slice(payload);
}

pub fn parse_envelope(bytes: &[u8]) -> Result<(EnvelopeHeader, &[u8])> {
    if bytes.len() < HEADER_LEN {
        return Err(Error::Other("compressed payload too short".to_string()));
    }
    if &bytes[0..4] != ORSX_MAGIC {
        return Err(Error::Other("compressed payload has invalid magic".to_string()));
    }
    let version = bytes[4];
    if version != ORSX_VERSION {
        return Err(Error::Other(format!(
            "unsupported compression envelope version: {version}"
        )));
    }
    let codec = CodecId::from_u8(bytes[5]).ok_or_else(|| {
        Error::Other(format!("unsupported compression codec id: {}", bytes[5]))
    })?;
    let elem_type = ElemTypeId::from_u8(bytes[6]).ok_or_else(|| {
        Error::Other(format!("unsupported compression element type id: {}", bytes[6]))
    })?;

    let elem_count = read_u32_le(&bytes[8..12])?;
    let uncompressed_len = read_u32_le(&bytes[12..16])?;
    let checksum_crc32 = read_u32_le(&bytes[16..20])?;
    let payload = &bytes[HEADER_LEN..];

    let actual = crc32_payload(payload);
    if actual != checksum_crc32 {
        return Err(Error::Other("compressed payload checksum mismatch".to_string()));
    }

    Ok((
        EnvelopeHeader {
            version,
            codec,
            elem_type,
            elem_count,
            uncompressed_len,
            checksum_crc32,
        },
        payload,
    ))
}

pub fn crc32_payload(payload: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(payload);
    hasher.finalize()
}

fn read_u32_le(b: &[u8]) -> Result<u32> {
    if b.len() != 4 {
        return Err(Error::Other("invalid u32 field length".to_string()));
    }
    let mut arr = [0u8; 4];
    arr.copy_from_slice(b);
    Ok(u32::from_le_bytes(arr))
}
