//! Safe, zero-cost abstractions for Netmap kernel-bypass networking.
//!
//! # Fetures
//! - Zero-copy packet I/O
//! - Thread-per-ring with core pinning
//! - Batch Operations for high throughput
//! - Cross-platform surpport (with fallback implementation)

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

#[cfg(feature = "sys")]
#[cfg(feature = "sys")]
#[macro_use]
extern crate bitflags;
#[allow(unused_imports)] // Clippy seems to have a false positive with specific feature flags
#[macro_use]
extern crate thiserror;

/// Error types for the netmap library.
pub mod error;
/// Fallback implementations for non-Netmap platforms.
pub mod fallback;
/// Frame structures for representing network packets.
pub mod frame;
/// Netmap interface and builder types.
pub mod netmap;
/// Netmap ring manipulation.
pub mod ring;

#[cfg(feature = "sys")]
pub use netmap_min_sys as ffi;

pub use crate::{error::Error, frame::Frame};

/// Commonly used types for convenience.
pub mod prelude {
    pub use crate::error::Error;
    pub use crate::frame::Frame;

    #[cfg(feature = "sys")]
    pub use crate::{
        netmap::{Netmap, NetmapBuilder},
        ring::{Ring, RxRing, TxRing},
    };
}

// Re-export sys-specific types only when sys feature is enabled
#[cfg(feature = "sys")]
pub use crate::{
    netmap::{Netmap, NetmapBuilder},
    ring::{Ring, RxRing, TxRing},
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
