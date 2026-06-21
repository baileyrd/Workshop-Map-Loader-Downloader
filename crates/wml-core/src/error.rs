//! Crate-wide error type.

/// Errors produced anywhere in `wml-core`.
#[derive(Debug, thiserror::Error)]
pub enum WmlError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("config error: {0}")]
    Config(String),

    #[error("bakkesmod bridge error: {0}")]
    Bridge(String),

    #[error("toml deserialize error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("toml serialize error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    /// A code path that is scaffolded but not yet implemented.
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, WmlError>;
