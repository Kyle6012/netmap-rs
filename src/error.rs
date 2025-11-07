use std::io;
use thiserror::Error;

/// Errors that can occur when working with Netmap

#[derive(Error, Debug)]
pub enum Error {
    /// I/O error from the underlying system
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// operation would block
    #[error("Operation would block")]
    WouldBlock,

    /// binding interface failed
    #[error("Failed to bind to interface: {0}")]
    BindFail(String),

    /// Invalid ring index
    #[error("Invalid ring index: {0}")]
    InvalidRingIndex(usize),

    /// Packet too large for ring buffer
    #[error("Packet too large for ring buffer: {0} bytes")]
    PacketTooLarge(usize),

    /// Not enough space in ring buffer
    #[error("Not enough space in ring buffer")]
    InsufficientSpace,

    /// Platform not yet supported
    #[error("Platform not yet supported: {0}")]
    UnsupportedPlatform(String),

    /// Feature not  supported in fallback mode
    #[error("Feature not supported in fallback mode: {0}")]
    FallbackUnsupported(String),
}

impl From<Error> for io::Error {
    fn from(err: Error) -> io::Error {
        match err {
            Error::Io(e) => e,
            e => io::Error::new(io::ErrorKind::Other, e.to_string()),
        }
    }
}