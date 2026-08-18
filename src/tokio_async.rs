//! Provides Tokio-based asynchronous wrappers for Netmap interfaces and rings.
//!
//! This module is only available when the `tokio-async` feature is enabled.
//!
//! It allows integrating Netmap I/O operations into Tokio's asynchronous runtime,
//! enabling non-blocking packet processing.
//!
//! # Key Components:
//! - [`TokioNetmap`]: Wraps a `netmap_rs::Netmap` instance with `tokio::io::unix::AsyncFd`
//!   to make it usable in an async context. It's the entry point for creating
//!   asynchronous ring wrappers.
//! - [`AsyncNetmapRxRing`]: Implements `tokio::io::AsyncRead` for a Netmap RX ring,
//!   allowing asynchronous packet reception.
//! - [`AsyncNetmapTxRing`]: Implements `tokio::io::AsyncWrite` for a Netmap TX ring,
//!   allowing asynchronous packet transmission.
//!
//! # Important Considerations for Correctness:
//! Each `poll_*` method performs the required Netmap kernel synchronization via
//! the `NIOCRXSYNC` (RX) and `NIOCTXSYNC` (TX) ioctls before checking the ring:
//! `AsyncRead::poll_read` calls `NIOCRXSYNC` so newly arrived packets become
//! visible, and `AsyncWrite::poll_flush` (and `poll_shutdown`) call
//! `NIOCTXSYNC` so queued packets are handed to the NIC. Without these sync
//! points the wrappers would never observe or deliver traffic.
//!
//! # Example Usage (Conceptual)
//! ```no_run
//! # #[cfg(feature = "tokio-async")]
//! # async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! use netmap_rs::NetmapBuilder;
//! use netmap_rs::tokio_async::TokioNetmap;
//! use tokio::io::{AsyncReadExt, AsyncWriteExt};
//!
//! // 1. Open a Netmap interface (e.g., a pipe for local testing)
//! let netmap_a = NetmapBuilder::new("netmap:pipe{myasync}").build()?;
//! let netmap_b = NetmapBuilder::new("netmap:pipe{myasync}").build()?;
//!
//! // 2. Wrap with TokioNetmap
//! let tokio_nm_a = TokioNetmap::new(netmap_a)?;
//! let tokio_nm_b = TokioNetmap::new(netmap_b)?;
//!
//! // 3. Get async ring handles
//! let mut tx_a = tokio_nm_a.tx_ring(0)?;
//! let mut rx_b = tokio_nm_b.rx_ring(0)?;
//!
//! // 4. Use in Tokio tasks
//! tokio::spawn(async move {
//!     let data_to_send = b"hello async netmap";
//!     tx_a.write_all(data_to_send).await.expect("Send failed");
//!     tx_a.flush().await.expect("Flush failed");
//! });
//!
//! let mut buffer = [0u8; 128];
//! let bytes_read = rx_b.read(&mut buffer).await.expect("Receive failed");
//! println!("Received: {:?}", &buffer[..bytes_read]);
//! # Ok(())
//! # }
//! ```

#![cfg(feature = "tokio-async")]

use crate::error::Error as NetmapError;
use crate::ffi;
use crate::netmap::Netmap;
use std::io;
use std::os::unix::io::AsRawFd;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// A Tokio-friendly wrapper around a [`Netmap`] interface.
///
/// It wraps the underlying Netmap file descriptor in a `tokio::io::unix::AsyncFd`
/// so ring operations can be polled from within a Tokio runtime. Use
/// [`rx_ring`](Self::rx_ring) and [`tx_ring`](Self::tx_ring) to obtain
/// asynchronous ring wrappers.
#[derive(Debug)]
pub struct TokioNetmap {
    async_fd_netmap: Arc<AsyncFd<Netmap>>,
}

impl TokioNetmap {
    /// Creates a new `TokioNetmap` by taking ownership of a `Netmap` instance
    /// and wrapping its file descriptor for asynchronous I/O with Tokio.
    ///
    /// # Arguments
    /// * `netmap`: The `Netmap` instance to wrap.
    ///
    /// # Errors
    /// Returns an `io::Error` if the `Netmap` file descriptor cannot be registered
    /// with Tokio's reactor (e.g., if it's not a valid fd).
    pub fn new(netmap: Netmap) -> io::Result<Self> {
        Ok(Self {
            async_fd_netmap: Arc::new(AsyncFd::new(netmap)?),
        })
    }

    /// Creates an asynchronous wrapper for a specific Netmap RX ring.
    ///
    /// This allows the RX ring to be used with Tokio's `AsyncRead` trait.
    ///
    /// # Arguments
    /// * `ring_idx`: The index of the RX ring to wrap. This index should be valid
    ///   for the underlying `Netmap` instance (i.e., less than `num_rx_rings()`).
    ///
    /// # Errors
    /// Returns `NetmapError::InvalidRingIndex` if the `ring_idx` is out of bounds.
    pub fn rx_ring(&self, ring_idx: usize) -> Result<AsyncNetmapRxRing, NetmapError> {
        let netmap_instance = self.async_fd_netmap.get_ref();
        if ring_idx >= netmap_instance.num_rx_rings() {
            return Err(NetmapError::InvalidRingIndex(ring_idx));
        }
        // Safety: Netmap guarantees nifp and rings are valid if open succeeded.
        // The lifetime of ring_ptr is tied to Netmap within AsyncFd, managed by Arc.
        let ring_ptr = unsafe { ffi::NETMAP_RXRING(netmap_instance.nifp(), ring_idx as u32) };

        Ok(AsyncNetmapRxRing {
            shared_fd_netmap: Arc::clone(&self.async_fd_netmap),
            ring_ptr,
        })
    }

    /// Creates an asynchronous wrapper for a specific Netmap TX ring.
    ///
    /// This allows the TX ring to be used with Tokio's `AsyncWrite` trait.
    /// # Arguments
    /// * `ring_idx`: The index of the TX ring to wrap. This index should be valid
    ///   for the underlying `Netmap` instance (i.e., less than `num_tx_rings()`).
    ///
    /// # Errors
    /// Returns `NetmapError::InvalidRingIndex` if the `ring_idx` is out of bounds.
    pub fn tx_ring(&self, ring_idx: usize) -> Result<AsyncNetmapTxRing, NetmapError> {
        let netmap_instance = self.async_fd_netmap.get_ref();
        if ring_idx >= netmap_instance.num_tx_rings() {
            return Err(NetmapError::InvalidRingIndex(ring_idx));
        }
        // Safety: See rx_ring.
        let ring_ptr = unsafe { ffi::NETMAP_TXRING(netmap_instance.nifp(), ring_idx as u32) };

        Ok(AsyncNetmapTxRing {
            shared_fd_netmap: Arc::clone(&self.async_fd_netmap),
            ring_ptr,
        })
    }
}

/// An asynchronous wrapper for a Netmap RX ring, implementing `tokio::io::AsyncRead`.
///
/// This struct allows receiving packets from a Netmap RX ring in an asynchronous
/// manner when used within a Tokio runtime. It shares an `AsyncFd<Netmap>` with
/// other ring wrappers from the same `TokioNetmap` instance.
///
/// **Note:** `poll_read` performs a `NIOCRXSYNC` ioctl on the underlying
/// descriptor before checking the ring, so packets received by the kernel are
/// made visible to the async task on each poll.
#[derive(Debug)]
pub struct AsyncNetmapRxRing {
    shared_fd_netmap: Arc<AsyncFd<Netmap>>,
    ring_ptr: *mut ffi::netmap_ring, // Raw pointer to the specific netmap_ring
}
unsafe impl Send for AsyncNetmapRxRing {}
// unsafe impl Sync for AsyncNetmapRxRing {} // Sync is tricky with raw ptr mutation if methods were &self

impl AsyncRead for AsyncNetmapRxRing {
    /// Attempts to read data from the Netmap RX ring into `buf`.
    ///
    /// This method integrates with Tokio's event loop. It will:
    /// 1. Synchronize the ring with the kernel via the `NIOCRXSYNC` ioctl.
    /// 2. Check for available packets in the ring.
    /// 3. If packets are available, copy one packet's data into `buf` and advance the ring.
    /// 4. If no packets are available, it registers the current task for wakeup
    ///    when the underlying Netmap file descriptor becomes readable and returns `Poll::Pending`.
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let self_mut = self.get_mut();
        loop {
            // 1. Synchronize the ring with the kernel. This is crucial for Netmap.
            // We call NIOCRXSYNC on the main Netmap file descriptor. This updates
            // the userspace view of all RX rings managed by this descriptor.
            // NIOCRXSYNC and NIOCTXSYNC are _IO ioctls that take no argument.
            unsafe {
                let fd = self_mut.shared_fd_netmap.get_ref().as_raw_fd();
                let ret = libc::ioctl(fd, ffi::NIOCRXSYNC);
                if ret == -1 {
                    // If ioctl fails, it's an OS error. Return it.
                    return Poll::Ready(Err(io::Error::last_os_error()));
                }
            }

            let ring = unsafe { &*self_mut.ring_ptr };
            // Ring pointers (head, tail, cur) should now be updated by the kernel side
            // due to NIOCRXSYNC. On RX rings, received packets occupy slots in
            // `[head, tail)`, so we read from `head` and advance `head`.
            let mut head = ring.head;
            let tail = ring.tail;
            let num_slots = ring.num_slots;

            if head == tail {
                match self_mut.shared_fd_netmap.poll_read_ready(cx) {
                    Poll::Ready(Ok(mut ready_guard)) => {
                        ready_guard.clear_ready();
                        // Re-check after poll indicated readiness. The kernel
                        // updates the ring on NIOCRXSYNC, performed above.
                        let updated_ring = unsafe { &*self_mut.ring_ptr };
                        head = updated_ring.head;
                        if head == tail {
                            return Poll::Pending;
                        }
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                }
            }
            // Process packet if head != tail
            let current_slot_idx = head % num_slots;
            let slot = unsafe { &*ring.slot.as_ptr().add(current_slot_idx as usize) };
            let packet_len = slot.len as usize;

            if packet_len == 0 {
                unsafe {
                    let mutable_ring = &mut *self_mut.ring_ptr;
                    let new_head = (head + 1) % num_slots;
                    mutable_ring.cur = new_head;
                    mutable_ring.head = new_head;
                }
                continue;
            }
            if buf.remaining() == 0 {
                return Poll::Ready(Ok(()));
            }

            let len_to_copy = std::cmp::min(packet_len, buf.remaining());
            let buf_ptr = unsafe { ffi::NETMAP_BUF(self_mut.ring_ptr, slot.buf_idx) };
            let packet_data =
                unsafe { std::slice::from_raw_parts(buf_ptr as *const u8, len_to_copy) };
            buf.put_slice(packet_data);

            unsafe {
                let mutable_ring = &mut *self_mut.ring_ptr;
                let new_head = (head + 1) % num_slots;
                mutable_ring.cur = new_head;
                mutable_ring.head = new_head;
            }
            return Poll::Ready(Ok(()));
        }
    }
}

/// An asynchronous wrapper for a Netmap TX ring, implementing
/// `tokio::io::AsyncWrite`.
///
/// It shares an `AsyncFd<Netmap>` with other ring wrappers from the same
/// [`TokioNetmap`] instance.
#[derive(Debug)]
pub struct AsyncNetmapTxRing {
    shared_fd_netmap: Arc<AsyncFd<Netmap>>,
    ring_ptr: *mut ffi::netmap_ring,
}
unsafe impl Send for AsyncNetmapTxRing {}
// unsafe impl Sync for AsyncNetmapTxRing {} // Sync is tricky if methods were &self

impl AsyncWrite for AsyncNetmapTxRing {
    /// Attempts to write data from `buf` into the Netmap TX ring.
    ///
    /// This method integrates with Tokio's event loop. It will:
    /// 1. Check for available space in the TX ring.
    /// 2. If space is available, copy the data from `buf` into a Netmap slot and advance the ring.
    ///    Returns `Poll::Ready(Ok(bytes_written))`.
    /// 3. If the ring is full, it registers the current task for wakeup when the
    ///    underlying Netmap file descriptor becomes writable and returns `Poll::Pending`.
    ///
    /// After writing data, `poll_flush` must be called to make the packets
    /// visible to the NIC; it performs the `NIOCTXSYNC` ioctl that hands
    /// queued packets to the kernel.
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let self_mut = self.get_mut();
        loop {
            let ring = unsafe { &*self_mut.ring_ptr };
            let head = ring.head;
            let tail = ring.tail;
            let num_slots = ring.num_slots;
            let max_payload = ring.nr_buf_size as usize;

            // Ring is full if head + 1 == tail (modulo num_slots)
            // This is a common way to represent a full circular buffer of N slots using N-1 items.
            let is_full = (head + 1) % num_slots == tail;

            if is_full {
                match self_mut.shared_fd_netmap.poll_write_ready(cx) {
                    Poll::Ready(Ok(mut ready_guard)) => {
                        ready_guard.clear_ready();
                        // FD is ready (space might be available). Loop to try
                        // writing again; a NIOCTXSYNC (in poll_flush) refreshes
                        // `tail` before re-checking space.
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)), // Poll error
                    Poll::Pending => return Poll::Pending, // Not ready, waker registered
                }
            } else {
                // Space is available
                if buf.is_empty() {
                    return Poll::Ready(Ok(0)); // Nothing to write
                }
                if buf.len() > max_payload {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        NetmapError::PacketTooLarge(buf.len()),
                    )));
                }

                let current_slot_idx = head % num_slots;
                // Safety: slot access is within num_slots.
                let slot = unsafe {
                    &mut *(*self_mut.ring_ptr)
                        .slot
                        .as_mut_ptr()
                        .add(current_slot_idx as usize)
                };

                // Copy data to the slot buffer
                // Safety: slot buffer is valid, buf.len() <= max_payload (nr_buf_size)
                let dst = unsafe { ffi::NETMAP_BUF(self_mut.ring_ptr, slot.buf_idx) };
                let slot_buf_slice =
                    unsafe { std::slice::from_raw_parts_mut(dst as *mut u8, buf.len()) };
                slot_buf_slice.copy_from_slice(buf);
                slot.len = buf.len() as u16;
                slot.flags = 0; // Clear flags, e.g. NS_BUF_CHANGED if it was set

                // Advance our head pointer
                // Safety: ring_ptr is valid.
                unsafe {
                    let mutable_ring = &mut *self_mut.ring_ptr;
                    let new_head = (head + 1) % num_slots;
                    mutable_ring.head = new_head;
                    mutable_ring.cur = new_head; // cur usually follows head in TX
                }
                return Poll::Ready(Ok(buf.len())); // Successfully wrote one packet
            }
        }
    }

    /// Flushes any buffered data to the Netmap TX ring, making it available to the NIC.
    ///
    /// This method performs the necessary synchronization with the kernel by
    /// calling the `NIOCTXSYNC` ioctl, making pending writes visible to the NIC.
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Safety: ring_ptr is valid. NIOCTXSYNC is an _IO ioctl that takes no
        // argument and syncs all TX rings owned by the descriptor.
        unsafe {
            let self_mut = self.get_mut(); // Pin::get_mut is safe within poll_ methods if not moving self_mut
            let fd = self_mut.shared_fd_netmap.get_ref().as_raw_fd();
            let ret = libc::ioctl(fd, ffi::NIOCTXSYNC);
            if ret == -1 {
                return Poll::Ready(Err(io::Error::last_os_error()));
            }
        }
        Poll::Ready(Ok(()))
    }

    /// Attempts to shut down the write side of this `AsyncNetmapTxRing`.
    ///
    /// This typically involves flushing any buffered data.
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.poll_flush(cx) {
            Poll::Ready(Ok(_)) => Poll::Ready(Ok(())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}
