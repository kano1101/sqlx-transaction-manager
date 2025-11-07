/// Error types for transaction management
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Database error from SQLx
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Transaction has already been consumed (committed or rolled back)
    #[error("Transaction has already been consumed")]
    AlreadyConsumed,
}

/// Result type alias for transaction operations
pub type Result<T> = std::result::Result<T, Error>;
