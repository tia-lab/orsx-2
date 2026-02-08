use crate::{Error, Result};
use sqlx::encode::Encode;
use sqlx::postgres::{PgArguments, Postgres};
use sqlx::types::{Json, JsonValue, Type};
use std::marker::PhantomData;

pub trait OrsxValueVisitor<'q> {
    fn visit_i16(&mut self, col: &'static str, value: Option<i16>) -> Result<()>;
    fn visit_i32(&mut self, col: &'static str, value: Option<i32>) -> Result<()>;
    fn visit_i64(&mut self, col: &'static str, value: Option<i64>) -> Result<()>;
    fn visit_f32(&mut self, col: &'static str, value: Option<f32>) -> Result<()>;
    fn visit_f64(&mut self, col: &'static str, value: Option<f64>) -> Result<()>;
    fn visit_bool(&mut self, col: &'static str, value: Option<bool>) -> Result<()>;

    fn visit_text(&mut self, col: &'static str, value: Option<&'q str>) -> Result<()>;
    fn visit_bytes(&mut self, col: &'static str, value: Option<&'q [u8]>) -> Result<()>;

    fn visit_uuid(&mut self, col: &'static str, value: Option<&'q sqlx::types::Uuid>) -> Result<()>;
    fn visit_sqlx_timestamp(
        &mut self,
        col: &'static str,
        value: Option<&'q crate::SqlxTimestamp>,
    ) -> Result<()>;

    fn visit_json_value(&mut self, col: &'static str, value: Option<&'q JsonValue>) -> Result<()>;

    fn visit_json<T>(&mut self, col: &'static str, value: Option<&'q Json<T>>) -> Result<()>
    where
        T: serde::Serialize + Send + Sync;
}

#[derive(Debug, Default)]
pub struct PgArgumentsVisitor<'q> {
    args: PgArguments,
    _pd: PhantomData<&'q ()>,
}

impl<'q> PgArgumentsVisitor<'q> {
    pub fn new() -> Self {
        Self {
            args: PgArguments::default(),
            _pd: PhantomData,
        }
    }

    pub fn reserve(&mut self, additional: usize, size: usize) {
        sqlx::Arguments::reserve(&mut self.args, additional, size);
    }

    pub fn into_arguments(self) -> PgArguments {
        self.args
    }

    fn add<T>(&mut self, col: &'static str, value: T) -> Result<()>
    where
        T: 'q + Encode<'q, Postgres> + Type<Postgres>,
    {
        sqlx::Arguments::add(&mut self.args, value)
            .map_err(|e| Error::Other(format!("PgArgumentsVisitor add failed for `{col}`: {e}")))?;
        Ok(())
    }
}

impl<'q> OrsxValueVisitor<'q> for PgArgumentsVisitor<'q> {
    fn visit_i16(&mut self, col: &'static str, value: Option<i16>) -> Result<()> {
        self.add(col, value)
    }

    fn visit_i32(&mut self, col: &'static str, value: Option<i32>) -> Result<()> {
        self.add(col, value)
    }

    fn visit_i64(&mut self, col: &'static str, value: Option<i64>) -> Result<()> {
        self.add(col, value)
    }

    fn visit_f32(&mut self, col: &'static str, value: Option<f32>) -> Result<()> {
        self.add(col, value)
    }

    fn visit_f64(&mut self, col: &'static str, value: Option<f64>) -> Result<()> {
        self.add(col, value)
    }

    fn visit_bool(&mut self, col: &'static str, value: Option<bool>) -> Result<()> {
        self.add(col, value)
    }

    fn visit_text(&mut self, col: &'static str, value: Option<&'q str>) -> Result<()> {
        self.add(col, value)
    }

    fn visit_bytes(&mut self, col: &'static str, value: Option<&'q [u8]>) -> Result<()> {
        self.add(col, value)
    }

    fn visit_uuid(&mut self, col: &'static str, value: Option<&'q sqlx::types::Uuid>) -> Result<()> {
        self.add(col, value)
    }

    fn visit_sqlx_timestamp(
        &mut self,
        col: &'static str,
        value: Option<&'q crate::SqlxTimestamp>,
    ) -> Result<()> {
        self.add(col, value)
    }

    fn visit_json_value(&mut self, col: &'static str, value: Option<&'q JsonValue>) -> Result<()> {
        self.add(col, value)
    }

    fn visit_json<T>(&mut self, col: &'static str, value: Option<&'q Json<T>>) -> Result<()>
    where
        T: serde::Serialize + Send + Sync,
    {
        self.add(col, value)
    }
}
