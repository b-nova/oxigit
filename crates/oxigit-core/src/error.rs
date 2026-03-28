use thiserror::Error;

#[derive(Debug, Error)]
pub enum OxigitError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Authentication failed")]
    AuthFailed,

    #[error("Username already taken")]
    UsernameTaken,

    #[error("Email already taken")]
    EmailTaken,

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Git error: {0}")]
    Git(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, OxigitError>;
