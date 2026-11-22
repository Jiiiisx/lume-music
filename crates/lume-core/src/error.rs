use thiserror::Error;

/// Core domain and provider error types shared across Lume Music crates.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("item not found: {0}")]
    NotFound(String),

    #[error("provider unavailable: {0}")]
    ProviderUnavailable(String),

    #[error("authentication required: {0}")]
    AuthRequired(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
