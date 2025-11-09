use thiserror::Error;

#[derive(Debug, Error)]
pub enum FontError {
    #[error("parse error: {0}")]
    Parse(String),
    #[error("unsupported font type")]
    UnsupportedType,
    #[error("invalid glyph data")]
    InvalidGlyph,
    #[error("unknown character: {0}")]
    UnknownChar(char),
}

pub type Result<T> = std::result::Result<T, FontError>;
