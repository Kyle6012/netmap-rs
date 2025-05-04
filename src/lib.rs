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

#[macro_use]
extern  crate bitflags;
#[macro_use]
extern crate thiserror;

pub mod error;
pub mod fallback;
pub mod frame;
pub mod netmap;
pub mod ring;

#[cfg(feature = "sys")]
pub use netmap_min_sys as ffi;

pub use crate::{
    error::Error,
    frame::Frame,
    netmap::{Netmap, NetmapBuilder},
    ring::{Ring, RxRing, TxRing},
};

/// prelude for convenient imports

pub mod prelude {
    pub use crate::{Frame, Netmap, NetmapBuilder, Ring, RxRing, TxRing};
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works(){
        assert_eq!(2 + 2, 4);
    }
}
