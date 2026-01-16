use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Database error: {message}")]
    Database {
        #[source]
        source: sqlx::Error,
        message: String,
    },

    #[error("Migration error: {message}")]
    Migration {
        message: String,
        sql: Option<String>,
        context: Option<String>,
    },

    #[error("Schema error: {0}")]
    Schema(String),

    #[error("Compression error: {0}")]
    Compression(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        Error::Database {
            source: err,
            message: "Database operation failed".to_string(),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
