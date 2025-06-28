//! Safe, zero-cost abstractions for Netmap kernel-bypass networking.
//!
//! # Features
//! - Zero-copy packet I/O
//! - Thread-per-ring with core pinning
//! - Batch Operations for high throughput
//! - Cross-platform support (with fallback implementation)
//!
//! # Usage
//!
//! Add `netmap-rs` to your `Cargo.toml`:
//! ```toml
//! [dependencies]
//! netmap-rs = "0.1" # Replace with the latest version
//! ```
//!
//! Basic example:
//! ```no_run
//! use netmap_rs::prelude::*;
//! use std::thread::sleep;
//! use std::time::Duration;
//!
//! fn main() -> Result<(), Error> {
//!     // Attempt to open a netmap interface.
//!     // Replace "netmap:eth0" with your desired interface.
//!     // The `.build()?` method finalizes the configuration and opens the interface.
//!     let nm = NetmapBuilder::new("netmap:eth0")
//!         .num_tx_rings(1) // Configure one transmission ring
//!         .num_rx_rings(1) // Configure one reception ring
//!         .build()?;
//!
//!     // Get handles to the first transmission and reception rings.
//!     let mut tx_ring = nm.tx_ring(0)?;
//!     let mut rx_ring = nm.rx_ring(0)?;
//!
//!     // Prepare a packet to send.
//!     let packet_data = b"hello netmap!";
//!
//!     // Send the packet.
//!     // The `send` method might not transmit immediately; it queues the packet.
//!     tx_ring.send(packet_data)?;
//!     // `sync` ensures that queued packets are made available to the hardware.
//!     tx_ring.sync();
//!     println!("Sent packet: {:?}", packet_data);
//!
//!     // Attempt to receive the packet.
//!     let mut received = false;
//!     for _ in 0..5 { // Try a few times with a delay
//!         // `sync` on the rx_ring tells the kernel we are done with previously received packets
//!         // and updates the ring's state to see new packets.
//!         rx_ring.sync();
//!         while let Some(frame) = rx_ring.recv() {
//!             println!("Received packet: {:?}", frame.payload());
//!             assert_eq!(frame.payload(), packet_data);
//!             received = true;
//!             break;
//!         }
//!         if received {
//!             break;
//!         }
//!         sleep(Duration::from_millis(100)); // Wait a bit for the packet to arrive
//!     }
//!
//!     if !received {
//!         eprintln!("Failed to receive the packet back.");
//!         // Depending on the setup (e.g. loopback interface), this might indicate an issue.
//!     }
//!
//!     Ok(())
//! }
//! ```
//! For more advanced examples, such as Forward Error Correction (FEC),
//! reliable delivery with ARQ, or dedicating threads per ring, please see the
//! files in the `examples` directory of the crate.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
// #![warn(rustdoc::missing_crate_level_docs)] // Already covered by the extensive example above

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

// Tokio async support (optional feature)
#[cfg(feature = "tokio-async")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio-async")))]
pub mod tokio_async;
// Re-export async types at the crate root when feature is enabled.
#[cfg(feature = "tokio-async")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio-async")))]
pub use tokio_async::{AsyncNetmapRxRing, AsyncNetmapTxRing, TokioNetmap};


pub use crate::{error::Error, frame::Frame};

/// The `prelude` module re-exports commonly used types from this crate
/// for easier access.
///
/// It is recommended to import all items from the prelude:
/// ```
/// use netmap_rs::prelude::*;
/// ```
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
