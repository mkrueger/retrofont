use thiserror::Error;

#[derive(Debug, Error)]
pub enum FontError {
    // IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // FIGlet-specific errors
    #[error(
        "FIGlet: gzip compressed .flf not supported without flate2; provide .flf or zipped archive"
    )]
    FigletGzipNotSupported,
    #[error("FIGlet: missing or invalid header")]
    FigletMissingHeader,
    #[error("FIGlet: not a flf2a header")]
    FigletInvalidSignature,
    #[error("FIGlet: incomplete header")]
    FigletIncompleteHeader,
    #[error("FIGlet: missing height in header")]
    FigletMissingHeight,
    #[error("FIGlet: incomplete character definition")]
    FigletIncompleteChar,
    #[error("FIGlet: character line missing @ marker")]
    FigletMissingMarker,

    // ZIP archive errors
    #[error("ZIP: {0}")]
    Zip(String),
    #[error("ZIP: archive contains no .flf file")]
    ZipNoFlf,

    // TDF-specific errors
    #[error("TDF: file too short")]
    TdfFileTooShort,
    #[error("TDF: invalid header length (expected {expected}, got {got})")]
    TdfIdLengthMismatch { expected: usize, got: usize },
    #[error("TDF: header ID mismatch")]
    TdfIdMismatch,
    #[error("TDF: missing CTRL-Z marker")]
    TdfMissingCtrlZ,
    #[error("TDF: font indicator mismatch")]
    TdfFontIndicatorMismatch,
    #[error("TDF: unsupported font type: {0}")]
    TdfUnsupportedType(u8),
    #[error("TDF: truncated data at {field}")]
    TdfTruncated { field: &'static str },
    #[error("TDF: glyph offset {offset} exceeds block size {size}")]
    TdfGlyphOutOfBounds { offset: usize, size: usize },
    #[error("TDF: bundle contains no fonts")]
    TdfEmptyBundle,
    #[error("TDF: name too long ({len} bytes, max {max})")]
    TdfNameTooLong { len: usize, max: usize },

    // Format detection
    #[error("unrecognized font format")]
    UnrecognizedFormat,

    // Conversion errors
    #[error("FIGlet font is not compatible with TDF conversion")]
    ConversionIncompatible,

    // Rendering errors
    #[error("unsupported font type")]
    UnsupportedType,
    #[error("invalid glyph data")]
    InvalidGlyph,
    #[error("unknown character: {0}")]
    UnknownChar(char),

    // UTF-8 errors
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
}

pub type Result<T> = std::result::Result<T, FontError>;
