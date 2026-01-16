// Compressed<T> wrapper for automatic compression using cydec
use cydec::{FloatingCodec, IntegerCodec};
use serde::{Deserialize, Serialize};
use sqlx::{
    encode::IsNull,
    postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef},
    Decode, Encode, Postgres, Type,
};
use std::error::Error as StdError;

// Compressed wrapper for Vec<T> types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Compressed<T>(pub Vec<T>);

impl<T> Compressed<T> {
    pub fn new(data: Vec<T>) -> Self {
        Self(data)
    }

    pub fn into_inner(self) -> Vec<T> {
        self.0
    }

    pub fn as_slice(&self) -> &[T] {
        &self.0
    }
}

impl<T> From<Vec<T>> for Compressed<T> {
    fn from(data: Vec<T>) -> Self {
        Self(data)
    }
}

// Macro to implement Type for all Compressed<Vec<T>> types
macro_rules! impl_compressed_type {
    ($t:ty) => {
        impl Type<Postgres> for Compressed<$t> {
            fn type_info() -> PgTypeInfo {
                PgTypeInfo::with_name("BYTEA")
            }
        }
    };
}

// Implement for all numeric types
impl_compressed_type!(i32);
impl_compressed_type!(i64);
impl_compressed_type!(u32);
impl_compressed_type!(u64);
impl_compressed_type!(f32);
impl_compressed_type!(f64);

// Encode implementations (compress on write)

impl Encode<'_, Postgres> for Compressed<i32> {
    fn encode_by_ref(
        &self,
        buf: &mut PgArgumentBuffer,
    ) -> Result<IsNull, Box<dyn StdError + Send + Sync>> {
        let codec = IntegerCodec::default();
        match codec.compress_i32(&self.0) {
            Ok(compressed) => {
                buf.extend_from_slice(&compressed);
                Ok(IsNull::No)
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl Encode<'_, Postgres> for Compressed<i64> {
    fn encode_by_ref(
        &self,
        buf: &mut PgArgumentBuffer,
    ) -> Result<IsNull, Box<dyn StdError + Send + Sync>> {
        let codec = IntegerCodec::default();
        match codec.compress_i64(&self.0) {
            Ok(compressed) => {
                buf.extend_from_slice(&compressed);
                Ok(IsNull::No)
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl Encode<'_, Postgres> for Compressed<u32> {
    fn encode_by_ref(
        &self,
        buf: &mut PgArgumentBuffer,
    ) -> Result<IsNull, Box<dyn StdError + Send + Sync>> {
        let codec = IntegerCodec::default();
        match codec.compress_u32(&self.0) {
            Ok(compressed) => {
                buf.extend_from_slice(&compressed);
                Ok(IsNull::No)
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl Encode<'_, Postgres> for Compressed<u64> {
    fn encode_by_ref(
        &self,
        buf: &mut PgArgumentBuffer,
    ) -> Result<IsNull, Box<dyn StdError + Send + Sync>> {
        let codec = IntegerCodec::default();
        match codec.compress_u64(&self.0) {
            Ok(compressed) => {
                buf.extend_from_slice(&compressed);
                Ok(IsNull::No)
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl Encode<'_, Postgres> for Compressed<f32> {
    fn encode_by_ref(
        &self,
        buf: &mut PgArgumentBuffer,
    ) -> Result<IsNull, Box<dyn StdError + Send + Sync>> {
        let codec = FloatingCodec::default();
        match codec.compress_f32(&self.0, None) {
            Ok(compressed) => {
                buf.extend_from_slice(&compressed);
                Ok(IsNull::No)
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl Encode<'_, Postgres> for Compressed<f64> {
    fn encode_by_ref(
        &self,
        buf: &mut PgArgumentBuffer,
    ) -> Result<IsNull, Box<dyn StdError + Send + Sync>> {
        let codec = FloatingCodec::default();
        match codec.compress_f64(&self.0, None) {
            Ok(compressed) => {
                buf.extend_from_slice(&compressed);
                Ok(IsNull::No)
            }
            Err(e) => Err(e.into()),
        }
    }
}

// Decode implementations (decompress on read)

impl Decode<'_, Postgres> for Compressed<i32> {
    fn decode(value: PgValueRef<'_>) -> Result<Self, Box<dyn StdError + Send + Sync>> {
        let bytes = <Vec<u8> as Decode<Postgres>>::decode(value)?;
        let codec = IntegerCodec::default();
        codec
            .decompress_i32(&bytes)
            .map(Compressed)
            .map_err(|e| format!("Decompression failed: {}", e).into())
    }
}

impl Decode<'_, Postgres> for Compressed<i64> {
    fn decode(value: PgValueRef<'_>) -> Result<Self, Box<dyn StdError + Send + Sync>> {
        let bytes = <Vec<u8> as Decode<Postgres>>::decode(value)?;
        let codec = IntegerCodec::default();
        codec
            .decompress_i64(&bytes)
            .map(Compressed)
            .map_err(|e| format!("Decompression failed: {}", e).into())
    }
}

impl Decode<'_, Postgres> for Compressed<u32> {
    fn decode(value: PgValueRef<'_>) -> Result<Self, Box<dyn StdError + Send + Sync>> {
        let bytes = <Vec<u8> as Decode<Postgres>>::decode(value)?;
        let codec = IntegerCodec::default();
        codec
            .decompress_u32(&bytes)
            .map(Compressed)
            .map_err(|e| format!("Decompression failed: {}", e).into())
    }
}

impl Decode<'_, Postgres> for Compressed<u64> {
    fn decode(value: PgValueRef<'_>) -> Result<Self, Box<dyn StdError + Send + Sync>> {
        let bytes = <Vec<u8> as Decode<Postgres>>::decode(value)?;
        let codec = IntegerCodec::default();
        codec
            .decompress_u64(&bytes)
            .map(Compressed)
            .map_err(|e| format!("Decompression failed: {}", e).into())
    }
}

impl Decode<'_, Postgres> for Compressed<f32> {
    fn decode(value: PgValueRef<'_>) -> Result<Self, Box<dyn StdError + Send + Sync>> {
        let bytes = <Vec<u8> as Decode<Postgres>>::decode(value)?;
        let codec = FloatingCodec::default();
        codec
            .decompress_f32(&bytes, None)
            .map(Compressed)
            .map_err(|e| format!("Decompression failed: {}", e).into())
    }
}

impl Decode<'_, Postgres> for Compressed<f64> {
    fn decode(value: PgValueRef<'_>) -> Result<Self, Box<dyn StdError + Send + Sync>> {
        let bytes = <Vec<u8> as Decode<Postgres>>::decode(value)?;
        let codec = FloatingCodec::default();
        codec
            .decompress_f64(&bytes, None)
            .map(Compressed)
            .map_err(|e| format!("Decompression failed: {}", e).into())
    }
}
