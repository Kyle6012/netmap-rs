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
    pub fn new (max_size: usize) -> Self {
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
    pub fn recv(&self) -> Option<Frame> {
        let mut queue = self.0.queue.lock().unwrap();
        queue.pop_front().map(|v|Frame::new(&v))
    }
}
