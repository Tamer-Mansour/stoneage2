#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error at line {line}: {msg}")]
    Parse { line: usize, msg: String },
    #[error("unsupported file format: {0}")]
    Format(String),
    #[error("image error: {0}")]
    Image(String),
}

pub type Result<T> = std::result::Result<T, Error>;
