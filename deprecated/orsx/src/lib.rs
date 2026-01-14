// Module declarations
pub mod error;
pub mod indexes;
pub mod migrations;
pub mod traits;
pub mod types;

// Re-exports
pub use error::{Error, Result};
pub use migrations::Migrations;
pub use traits::{BatchTableQuery, OrsxMigrate, TableQuery};
pub use types::{Compressed, FieldType};

// PostgreSQL identifier quoting for SQL injection prevention
// Escapes double quotes and wraps identifier in quotes
pub fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

// Re-export sqlx for convenience
pub use jiff;
pub use pgvector;
pub use sqlx;

// Re-export derive macro
pub use orsx_macros::OrsxMigrate;

// Prelude for convenient imports
pub mod prelude {
    pub use crate::error::{Error, Result};
    pub use crate::migrations::Migrations;
    pub use crate::traits::{BatchTableQuery, OrsxMigrate as OrsxMigrateTrait, TableQuery};
    pub use crate::types::{Compressed, FieldType};
    pub use crate::OrsxMigrate; // derive macro
    pub use jiff;
    pub use pgvector;
    pub use sqlx;
}
