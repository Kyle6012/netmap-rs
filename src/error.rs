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
            e => io::Error::other(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages() {
        assert_eq!(Error::WouldBlock.to_string(), "Operation would block");
        assert_eq!(
            Error::BindFail("eth0".into()).to_string(),
            "Failed to bind to interface: eth0"
        );
        assert_eq!(
            Error::InvalidRingIndex(3).to_string(),
            "Invalid ring index: 3"
        );
        assert_eq!(
            Error::PacketTooLarge(9001).to_string(),
            "Packet too large for ring buffer: 9001 bytes"
        );
        assert_eq!(
            Error::InsufficientSpace.to_string(),
            "Not enough space in ring buffer"
        );
        assert_eq!(
            Error::UnsupportedPlatform("plan9".into()).to_string(),
            "Platform not yet supported: plan9"
        );
        assert_eq!(
            Error::FallbackUnsupported("fec".into()).to_string(),
            "Feature not supported in fallback mode: fec"
        );
    }

    #[test]
    fn from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file missing");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn from_error_to_io_error() {
        let netmap_err = Error::WouldBlock;
        let io_err: io::Error = netmap_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn io_error_unwrapped() {
        let source = io::Error::new(io::ErrorKind::ConnectionRefused, "refused");
        let netmap_err = Error::Io(source);
        let io_err: io::Error = netmap_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::ConnectionRefused);
        assert_eq!(io_err.to_string(), "refused");
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Error>();
    }
}
