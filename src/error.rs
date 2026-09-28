use std::fmt;

#[derive(Debug)]
pub enum PuppetError {
    PermissionDenied(String),
    CommandFailed(String),
    NetworkFailure(String),
    TemporaryFailure(String),
    ConfigurationError(String),
    Unsupported(String),
    Unknown(String),
}

impl PuppetError {
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::NetworkFailure(_) | Self::TemporaryFailure(_)
        )
    }
}

impl fmt::Display for PuppetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionDenied(msg) => write!(f, "permission denied: {msg}"),
            Self::CommandFailed(msg) => write!(f, "command failed: {msg}"),
            Self::NetworkFailure(msg) => write!(f, "network failure: {msg}"),
            Self::TemporaryFailure(msg) => write!(f, "temporary failure: {msg}"),
            Self::ConfigurationError(msg) => write!(f, "configuration error: {msg}"),
            Self::Unsupported(msg) => write!(f, "unsupported: {msg}"),
            Self::Unknown(msg) => write!(f, "unknown error: {msg}"),
        }
    }
}

impl std::error::Error for PuppetError {}
