//! Fallback implementation for platforms without Netnap support

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::error::Error;
use crate::frame::Frame;

#[derive(Clone)]
struct SharedRing {
    queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    max_size: usize,
}

/// fallback implememntation for a Netmap TX ring
pub struct FallbackTxRing(SharedRing);

/// fallback implememntation for a Netmap RX ring
pub struct FallbackRxRing(SharedRing);

impl FallbackTxRing {
    /// create new fallback TX ring
    pub fn new(max_size: usize) -> Self {
        Self(SharedRing {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            max_size,
        })
    }

    /// send a packet
    pub fn send(&self, buf: &[u8]) -> Result<(), Error> {
        let mut queue = self.0.queue.lock().unwrap();
        if queue.len() >= self.0.max_size {
            return Err(Error::WouldBlock);
        }
        queue.push_back(buf.to_vec());
        Ok(())
    }
}

impl FallbackRxRing {
    /// create a new fallback RX ring
    pub fn new(max_size: usize) -> Self {
        Self(SharedRing {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            max_size,
        })
    }

    /// recieve a packet
    pub fn recv(&self) -> Option<Frame<'static>> {
        let mut queue = self.0.queue.lock().unwrap();
        queue.pop_front().map(Frame::new_owned)
    }
}

/// Creates a connected pair of fallback TX and RX rings.
pub fn create_fallback_channel(max_size: usize) -> (FallbackTxRing, FallbackRxRing) {
    let shared_ring = SharedRing {
        queue: Arc::new(Mutex::new(VecDeque::new())),
        max_size,
    };
    (
        FallbackTxRing(shared_ring.clone()),
        FallbackRxRing(shared_ring),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_ring_returns_none() {
        let rx = FallbackRxRing::new(4);
        assert!(rx.recv().is_none());
    }

    #[test]
    fn send_recv_single() {
        let tx = FallbackTxRing::new(4);
        let rx = FallbackRxRing::new(4);
        // Not connected; create channel instead.
        drop(tx);
        drop(rx);
        let (tx, rx) = create_fallback_channel(4);
        tx.send(b"ping").unwrap();
        let frame = rx.recv().unwrap();
        assert_eq!(frame.payload(), b"ping");
    }

    #[test]
    fn send_recv_in_order() {
        let (tx, rx) = create_fallback_channel(16);
        for i in 0..8u8 {
            tx.send(&[i]).unwrap();
        }
        for i in 0..8u8 {
            let frame = rx.recv().unwrap();
            assert_eq!(frame.payload(), &[i]);
        }
        assert!(rx.recv().is_none());
    }

    #[test]
    fn full_ring_returns_would_block() {
        let (tx, rx) = create_fallback_channel(2);
        tx.send(b"a").unwrap();
        tx.send(b"b").unwrap();
        assert!(matches!(tx.send(b"c"), Err(Error::WouldBlock)));
        // Drain one and retry.
        rx.recv().unwrap();
        assert!(tx.send(b"c").is_ok());
    }

    #[test]
    fn oversize_buffers_are_copied() {
        let (tx, rx) = create_fallback_channel(4);
        let payload = vec![0xabu8; 4096];
        tx.send(&payload).unwrap();
        let frame = rx.recv().unwrap();
        assert_eq!(frame.payload(), payload.as_slice());
    }
}
