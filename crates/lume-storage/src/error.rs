use thiserror::Error;

/// Persistence, configuration, and artwork cache errors.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("sqlite error: {0}")]
    Sql(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("playlist '{0}' is read-only")]
    PlaylistReadOnly(String),

    #[error("not found: {0}")]
    NotFound(String),
}
