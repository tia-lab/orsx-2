use crate::compression::{build_envelope, parse_envelope, CodecId, ElemTypeId};
use crate::{Error, Result};
use cydec::IntegerCodec;
use serde::{Deserialize, Serialize};
use sqlx::{
    encode::IsNull,
    postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef},
    Decode, Encode, Postgres, Type,
};
use std::error::Error as StdError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Compressed<T>(pub Vec<T>);

impl<T> Compressed<T> {
    pub fn new(values: Vec<T>) -> Self {
        Self(values)
    }

    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    pub fn into_inner(self) -> Vec<T> {
        self.0
    }
}

#[derive(Debug, Default, Clone)]
pub struct CompressedWorkspace {
    bytes: Vec<u8>,
    payload: Vec<u8>,
    tmp_u64: Vec<u64>,
    tmp_u32: Vec<u32>,
}

impl CompressedWorkspace {
    pub fn with_capacity(bytes: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(bytes),
            payload: Vec::with_capacity(bytes),
            tmp_u64: Vec::new(),
            tmp_u32: Vec::new(),
        }
    }

    pub fn prepare(&mut self) {
        self.bytes.clear();
        self.payload.clear();
        self.tmp_u64.clear();
        self.tmp_u32.clear();
    }
}

pub trait CompressedElem: Sized {
    const ELEM_ID: ElemTypeId;
    const CODEC: CodecId;
    fn compress_payload(values: &Vec<Self>, ws: &mut CompressedWorkspace) -> Result<()>;
    fn decompress_payload(payload: &[u8]) -> Result<Vec<Self>>;
}

impl CompressedElem for i32 {
    const ELEM_ID: ElemTypeId = ElemTypeId::I32;
    const CODEC: CodecId = CodecId::CydecInteger;

    fn compress_payload(values: &Vec<Self>, ws: &mut CompressedWorkspace) -> Result<()> {
        let codec = IntegerCodec::default();
        let bytes = codec
            .compress_i32(values)
            .map_err(|e| Error::Other(format!("compression failed: {e}")))?;
        ws.payload.clear();
        ws.payload.extend_from_slice(&bytes);
        Ok(())
    }

    fn decompress_payload(payload: &[u8]) -> Result<Vec<Self>> {
        let codec = IntegerCodec::default();
        codec
            .decompress_i32(payload)
            .map_err(|e| Error::Other(format!("decompression failed: {e}")))
    }
}

impl CompressedElem for i64 {
    const ELEM_ID: ElemTypeId = ElemTypeId::I64;
    const CODEC: CodecId = CodecId::CydecInteger;

    fn compress_payload(values: &Vec<Self>, ws: &mut CompressedWorkspace) -> Result<()> {
        let codec = IntegerCodec::default();
        let bytes = codec
            .compress_i64(values)
            .map_err(|e| Error::Other(format!("compression failed: {e}")))?;
        ws.payload.clear();
        ws.payload.extend_from_slice(&bytes);
        Ok(())
    }

    fn decompress_payload(payload: &[u8]) -> Result<Vec<Self>> {
        let codec = IntegerCodec::default();
        codec
            .decompress_i64(payload)
            .map_err(|e| Error::Other(format!("decompression failed: {e}")))
    }
}

impl CompressedElem for u32 {
    const ELEM_ID: ElemTypeId = ElemTypeId::U32;
    const CODEC: CodecId = CodecId::CydecInteger;

    fn compress_payload(values: &Vec<Self>, ws: &mut CompressedWorkspace) -> Result<()> {
        let codec = IntegerCodec::default();
        let bytes = codec
            .compress_u32(values)
            .map_err(|e| Error::Other(format!("compression failed: {e}")))?;
        ws.payload.clear();
        ws.payload.extend_from_slice(&bytes);
        Ok(())
    }

    fn decompress_payload(payload: &[u8]) -> Result<Vec<Self>> {
        let codec = IntegerCodec::default();
        codec
            .decompress_u32(payload)
            .map_err(|e| Error::Other(format!("decompression failed: {e}")))
    }
}

impl CompressedElem for u64 {
    const ELEM_ID: ElemTypeId = ElemTypeId::U64;
    const CODEC: CodecId = CodecId::CydecInteger;

    fn compress_payload(values: &Vec<Self>, ws: &mut CompressedWorkspace) -> Result<()> {
        let codec = IntegerCodec::default();
        let bytes = codec
            .compress_u64(values)
            .map_err(|e| Error::Other(format!("compression failed: {e}")))?;
        ws.payload.clear();
        ws.payload.extend_from_slice(&bytes);
        Ok(())
    }

    fn decompress_payload(payload: &[u8]) -> Result<Vec<Self>> {
        let codec = IntegerCodec::default();
        codec
            .decompress_u64(payload)
            .map_err(|e| Error::Other(format!("decompression failed: {e}")))
    }
}

impl CompressedElem for f32 {
    const ELEM_ID: ElemTypeId = ElemTypeId::F32;
    // Lossless: compress IEEE754 bit patterns using integer codec.
    const CODEC: CodecId = CodecId::CydecInteger;

    fn compress_payload(values: &Vec<Self>, ws: &mut CompressedWorkspace) -> Result<()> {
        ws.tmp_u32.resize(values.len(), 0);
        for (dst, src) in ws.tmp_u32.iter_mut().zip(values.iter()) {
            *dst = src.to_bits();
        }
        let codec = IntegerCodec::default();
        let bytes = codec
            .compress_u32(&ws.tmp_u32)
            .map_err(|e| Error::Other(format!("compression failed: {e}")))?;
        ws.payload.clear();
        ws.payload.extend_from_slice(&bytes);
        Ok(())
    }

    fn decompress_payload(payload: &[u8]) -> Result<Vec<Self>> {
        let codec = IntegerCodec::default();
        let bits = codec
            .decompress_u32(payload)
            .map_err(|e| Error::Other(format!("decompression failed: {e}")))?;
        Ok(bits.into_iter().map(f32::from_bits).collect())
    }
}

impl CompressedElem for f64 {
    const ELEM_ID: ElemTypeId = ElemTypeId::F64;
    // Lossless: compress IEEE754 bit patterns using integer codec.
    const CODEC: CodecId = CodecId::CydecInteger;

    fn compress_payload(values: &Vec<Self>, ws: &mut CompressedWorkspace) -> Result<()> {
        ws.tmp_u64.resize(values.len(), 0);
        for (dst, src) in ws.tmp_u64.iter_mut().zip(values.iter()) {
            *dst = src.to_bits();
        }
        let codec = IntegerCodec::default();
        let bytes = codec
            .compress_u64(&ws.tmp_u64)
            .map_err(|e| Error::Other(format!("compression failed: {e}")))?;
        ws.payload.clear();
        ws.payload.extend_from_slice(&bytes);
        Ok(())
    }

    fn decompress_payload(payload: &[u8]) -> Result<Vec<Self>> {
        let codec = IntegerCodec::default();
        let bits = codec
            .decompress_u64(payload)
            .map_err(|e| Error::Other(format!("decompression failed: {e}")))?;
        Ok(bits.into_iter().map(f64::from_bits).collect())
    }
}

impl<T: CompressedElem> Compressed<T> {
    pub fn encode_envelope_into(&self, out: &mut Vec<u8>, ws: &mut CompressedWorkspace) -> Result<()> {
        ws.prepare();
        T::compress_payload(&self.0, ws)?;

        let elem_count: u32 = self
            .0
            .len()
            .try_into()
            .map_err(|_| Error::Other("compressed vector too large".to_string()))?;
        let uncompressed_len: u32 = (self.0.len() * std::mem::size_of::<T>())
            .try_into()
            .map_err(|_| Error::Other("uncompressed size overflow".to_string()))?;

        build_envelope(
            T::CODEC,
            T::ELEM_ID,
            elem_count,
            uncompressed_len,
            &ws.payload,
            &mut ws.bytes,
        );

        out.clear();
        out.extend_from_slice(&ws.bytes);
        Ok(())
    }

    pub fn decode_envelope(bytes: &[u8]) -> Result<Self> {
        let (hdr, payload) = parse_envelope(bytes)?;
        if hdr.codec != T::CODEC || hdr.elem_type != T::ELEM_ID {
            return Err(Error::Other(
                "compressed payload type/codec mismatch".to_string(),
            ));
        }
        let values = T::decompress_payload(payload)?;
        if values.len() as u32 != hdr.elem_count {
            return Err(Error::Other("compressed payload length mismatch".to_string()));
        }
        Ok(Compressed(values))
    }
}

macro_rules! impl_sqlx_compressed {
    ($t:ty) => {
        impl Type<Postgres> for Compressed<$t> {
            fn type_info() -> PgTypeInfo {
                PgTypeInfo::with_name("BYTEA")
            }
        }

        impl Encode<'_, Postgres> for Compressed<$t> {
            fn encode_by_ref(
                &self,
                buf: &mut PgArgumentBuffer,
            ) -> std::result::Result<IsNull, Box<dyn StdError + Send + Sync>> {
                thread_local! {
                    static WS: std::cell::RefCell<crate::CompressedWorkspace> =
                        std::cell::RefCell::new(crate::CompressedWorkspace::default());
                    static OUT: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new(Vec::new());
                }

                let r = WS.with(|ws| {
                    OUT.with(|out| {
                        let mut ws = ws.borrow_mut();
                        let mut out = out.borrow_mut();
                        self.encode_envelope_into(&mut out, &mut ws)
                    })
                });

                match r {
                    Ok(()) => {
                        OUT.with(|out| {
                            let out = out.borrow();
                            buf.extend_from_slice(&out);
                        });
                        Ok(IsNull::No)
                    }
                    Err(e) => Err(e.to_string().into()),
                }
            }
        }

        impl Decode<'_, Postgres> for Compressed<$t> {
            fn decode(value: PgValueRef<'_>) -> std::result::Result<Self, Box<dyn StdError + Send + Sync>> {
                let bytes = <Vec<u8> as Decode<Postgres>>::decode(value)?;
                crate::Compressed::<$t>::decode_envelope(&bytes)
                    .map_err(|e| e.to_string().into())
            }
        }
    };
}

impl_sqlx_compressed!(i32);
impl_sqlx_compressed!(i64);
impl_sqlx_compressed!(u32);
impl_sqlx_compressed!(u64);
impl_sqlx_compressed!(f32);
impl_sqlx_compressed!(f64);
