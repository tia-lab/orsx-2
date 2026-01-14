pub mod error;

pub use error::{Error, Result};

pub use sqlx;

pub fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

pub trait OrsxMigrate: Send + Sync {
    fn table_name() -> &'static str;
}

pub struct Migrations;

impl Migrations {
    pub async fn init<T: OrsxMigrate>(
        _pool: &sqlx::PgPool,
        _migrations: &[(T, Option<&str>)],
    ) -> Result<()> {
        Err(Error::Other(
            "orsx2 rewrite in progress: migrations not implemented yet".to_string(),
        ))
    }
}

pub use orsx_macros::OrsxMigrate;

pub mod prelude {
    pub use crate::{Error, Migrations, OrsxMigrate, Result};
    pub use crate::quote_identifier;
    pub use sqlx;
}

