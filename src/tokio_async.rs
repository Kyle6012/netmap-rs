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
//! The current implementations of `AsyncRead::poll_read` and `AsyncWrite::poll_flush`
//! (and by extension `poll_write`) have **placeholders for crucial Netmap synchronization
//! operations** (specifically, `ioctl` calls with `NIOCRXSYNC` and `NIOCTXSYNC`).
//! Without the correct and fully implemented `ioctl` calls in these methods,
//! the async wrappers **will not function correctly** with the Netmap kernel module
//! (e.g., new packets may not become visible on RX rings, or TX packets may not
//! actually be sent by the NIC).
//!
//! **These synchronization points MUST be correctly implemented using the appropriate
//! FFI constants and `libc::ioctl` calls for these wrappers to be reliable.**
//! The complexity lies in ensuring the `ioctl`s are called with the correct arguments,
//! which typically involve a pointer to a `struct nmreq`.
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
//! let netmap_a = NetmapBuilder::new("netmap:pipe{myasyncpipe}").build()?;
//! let netmap_b = NetmapBuilder::new("netmap:pipe{myasyncpipe}").build()?;
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
        let ring_ptr = unsafe { ffi::NETMAP_RXRING((*netmap_instance.desc).nifp, ring_idx as u32) };

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
        let ring_ptr = unsafe { ffi::NETMAP_TXRING((*netmap_instance.desc).nifp, ring_idx as u32) };

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
/// **Note:** The correct functioning of this `AsyncRead` implementation relies heavily
/// on the proper, currently placeholder, implementation of `NIOCRXSYNC` ioctl calls
/// within its `poll_read` method for synchronizing with the Netmap kernel module.
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
    /// 1. Attempt to synchronize the ring with the kernel (currently a placeholder for `NIOCRXSYNC ioctl`).
    /// 2. Check for available packets in the ring.
    /// 3. If packets are available, copy one packet's data into `buf` and advance the ring.
    /// 4. If no packets are available, it registers the current task for wakeup
    ///    when the underlying Netmap file descriptor becomes readable and returns `Poll::Pending`.
    ///
    /// **Critical Note:** The synchronization step (ioctl) is crucial and currently
    /// simplified in this draft. It must be correctly implemented for this method
    /// to function reliably.
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let self_mut = self.get_mut();
        loop {
            // 1. Synchronize the ring with the kernel. This is crucial for Netmap.
            // We call NIOCRXSYNC on the main Netmap file descriptor.
            // This should update the userspace view of all RX rings managed by this descriptor.
            // The netmap(4) man page suggests `ioctl(fd, NIOCRXSYNC)` can be used.
            // The third argument for _IOWR ioctls is a pointer to the type specified.
            // For a general sync on the FD, often a NULL pointer is passed if the specific
            // content of struct nmreq isn't needed for this particular operation on the main FD.
            // However, to be safe and align with how Netmap often uses nmreq for context,
            // passing a minimal (e.g. zeroed) nmreq might be more robust if the kernel driver
            // dereferences the pointer. For NIOCRXSYNC/NIOCTXSYNC on the main port fd,
            // the kernel uses the nifp from the fd directly.
            // Let's try with 0 as the argument first, as per common simple ioctl usage for commands.
            // If this fails (e.g. EFAULT), we'll need to pass a pointer to a dummy nmreq.
            unsafe {
                let fd = self_mut.shared_fd_netmap.get_ref().as_raw_fd();
                let ret = libc::ioctl(fd, ffi::NIOCRXSYNC as libc::c_ulong, 0 as *mut ffi::nmreq);
                if ret == -1 {
                    // If ioctl fails, it's an OS error. Return it.
                    return Poll::Ready(Err(io::Error::last_os_error()));
                }
            }

            let ring = unsafe { &*self_mut.ring_ptr };
            // Ring pointers (head, tail, cur) should now be updated by the kernel side
            // due to NIOCRXSYNC. Our logic below uses these updated values.
            let mut head = ring.head;
            let mut tail = ring.tail;
            let num_slots = ring.num_slots;

            if head == tail {
                match self_mut.shared_fd_netmap.poll_read_ready_mut(cx) {
                    Poll::Ready(Ok(mut ready_guard)) => {
                        ready_guard.clear_ready();
                        // Re-check after poll indicated readiness
                        // Placeholder for proper sync via ioctl
                        // unsafe { let fd = self_mut.shared_fd_netmap.get_ref().as_raw_fd(); libc::ioctl(fd, ffi::NIOCRXSYNC as _, self_mut.ring_ptr); }
                        let updated_ring = unsafe { &*self_mut.ring_ptr };
                        head = updated_ring.head;
                        if head == tail { return Poll::Pending; }
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                }
            }
            // Process packet if head != tail
            let current_slot_idx = tail % num_slots;
            let slot = unsafe { &*ring.slot.add(current_slot_idx as usize) };
            let packet_len = slot.len as usize;

            if packet_len == 0 || buf.remaining() == 0 {
                unsafe {
                    let mutable_ring = &mut *self_mut.ring_ptr;
                    let new_tail = (tail + 1) % num_slots;
                    mutable_ring.cur = new_tail;
                    mutable_ring.tail = new_tail;
                }
                return Poll::Ready(Ok(()));
            }

            let len_to_copy = std::cmp::min(packet_len, buf.remaining());
            let packet_data = unsafe { std::slice::from_raw_parts(slot.buf as *const u8, len_to_copy) };
            buf.put_slice(packet_data);

            unsafe {
                let mutable_ring = &mut *self_mut.ring_ptr;
                let new_tail = (tail + 1) % num_slots;
                mutable_ring.cur = new_tail;
                mutable_ring.tail = new_tail;
            }
            return Poll::Ready(Ok(()));
        }
    }
}

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
    /// After writing data, `poll_flush` must be called to ensure the packets are made
    /// available to the NIC (this typically involves an `NIOCTXSYNC` ioctl).
    ///
    /// **Critical Note:** The synchronization for making space available (related to `NIOCTXSYNC`
    /// updating the `tail` pointer from the kernel's perspective) is currently simplified.
    /// A complete implementation relies on `poll_flush` being effective and potentially
    /// an initial sync if `NETMAP_NO_TX_POLL` is not used.
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
                match self_mut.shared_fd_netmap.poll_write_ready_mut(cx) {
                    Poll::Ready(Ok(mut ready_guard)) => {
                        ready_guard.clear_ready();
                        // FD is ready (space might be available). Loop to try writing again.
                        // An explicit NIOCTXSYNC might be needed here if NETMAP_NO_TX_POLL is not set,
                        // to ensure `tail` is up-to-date before re-checking space.
                        // ** This is a simplification for now. **
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)), // Poll error
                    Poll::Pending => return Poll::Pending, // Not ready, waker registered
                }
            } else { // Space is available
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
                let slot = unsafe { &mut *ring.slot.add(current_slot_idx as usize) };

                // Copy data to the slot buffer
                // Safety: slot->buf is valid, buf.len() <= max_payload (nr_buf_size)
                let slot_buf_slice = unsafe { std::slice::from_raw_parts_mut(slot.buf as *mut u8, buf.len()) };
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
    /// This method should perform the necessary synchronization with the kernel,
    /// typically by calling `ioctl` with `NIOCTXSYNC`.
    ///
    /// **Critical Note:** The synchronization step (ioctl) is crucial and currently
    /// a placeholder in this draft. It must be correctly implemented for this method
    /// to function reliably.
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Safety: ring_ptr is valid. This syncs pending writes to the NIC.
        // Similar to NIOCRXSYNC, NIOCTXSYNC called on the main Netmap FD should sync all TX rings.
        // The third argument is 0, assuming it's optional or ignored for a full TX sync on the FD.
        // This is based on netmap(4) man page `ioctl(fd, NIOCTXSYNC)`.
        unsafe {
            let self_mut = self.get_mut(); // Pin::get_mut is safe within poll_ methods if not moving self_mut
            let fd = self_mut.shared_fd_netmap.get_ref().as_raw_fd();
            let ret = libc::ioctl(fd, ffi::NIOCTXSYNC as libc::c_ulong, 0 as *mut ffi::nmreq);
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
