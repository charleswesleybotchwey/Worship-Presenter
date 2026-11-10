use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("file: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0} is not a supported {1} file: {2}")]
    Format(String, &'static str, String),
    #[error("reference not understood: {0}")]
    Reference(String),
}

pub type Result<T> = std::result::Result<T, Error>;
