#![cfg(feature = "sys")]
#![allow(elided_lifetimes_in_paths)]
#![allow(mismatched_lifetime_syntaxes)]

use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr;
use std::slice;

use crate::error::Error;
use crate::ffi;
use crate::frame::Frame;

/// A Netmap ring (tx/rx)
pub struct Ring<'a> {
    ring: *mut ffi::netmap_ring,
    fd: i32,
    index: usize,
    direction: RingDirection,
    _marker: PhantomData<&'a mut ffi::netmap_ring>,
}

/// Direction of a [`Ring`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingDirection {
    /// Transmission ring (synced with `NIOCTXSYNC`).
    Tx,
    /// Reception ring (synced with `NIOCRXSYNC`).
    Rx,
}

unsafe impl<'a> Send for Ring<'a> {}

/// A TX ring
pub struct TxRing<'a>(Ring<'a>);

/// An RX ring
pub struct RxRing<'a>(Ring<'a>);

impl<'a> Deref for TxRing<'a> {
    type Target = Ring<'a>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> Deref for RxRing<'a> {
    type Target = Ring<'a>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> Ring<'a> {
    /// Create a new ring
    pub(crate) fn new(ring: *mut ffi::netmap_ring, fd: i32, index: usize) -> Self {
        Self {
            ring,
            fd,
            index,
            direction: RingDirection::Tx,
            _marker: PhantomData,
        }
    }

    /// Get the ring index (the ID of this ring).
    pub fn index(&self) -> usize {
        self.index
    }

    /// Current `head` pointer of the ring.
    pub fn head(&self) -> u32 {
        unsafe { (*self.ring).head }
    }

    /// Current `tail` pointer of the ring.
    pub fn tail(&self) -> u32 {
        unsafe { (*self.ring).tail }
    }

    /// Ring direction: 0 = TX, 1 = RX.
    pub fn dir(&self) -> u16 {
        unsafe { (*self.ring).dir }
    }

    /// Direction of this ring (TX or RX).
    pub fn direction(&self) -> RingDirection {
        self.direction
    }

    /// Get the total number of slots in this ring.
    pub fn num_slots(&self) -> usize {
        unsafe { (*self.ring).num_slots as usize }
    }

    /// Returns `true` if the ring has at least one free slot.
    ///
    /// A ring is considered full when `(head + 1) % num_slots == tail`.
    pub fn has_free_slots(&self) -> bool {
        unsafe {
            let ring = self.ring;
            let head = (*ring).head;
            let tail = (*ring).tail;
            let num_slots = (*ring).num_slots;
            (head + 1) % num_slots != tail
        }
    }

    /// sync the ring with the NIC
    ///
    /// TX rings are synced with `NIOCTXSYNC`, RX rings with `NIOCRXSYNC`.
    pub fn sync(&self) {
        unsafe {
            let cmd = match self.direction {
                RingDirection::Tx => ffi::NIOCTXSYNC,
                RingDirection::Rx => ffi::NIOCRXSYNC,
            };
            libc::ioctl(self.fd, cmd, 0);
        }
    }
}

impl<'a> TxRing<'a> {
    /// create a new tx ring
    pub(crate) fn new(ring: *mut ffi::netmap_ring, fd: i32, index: usize) -> Self {
        let mut r = Ring::new(ring, fd, index);
        r.direction = RingDirection::Tx;
        Self(r)
    }

    /// send a single packet
    pub fn send(&mut self, buf: &[u8]) -> Result<(), Error> {
        if buf.len() > self.max_payload_size() {
            return Err(Error::PacketTooLarge(buf.len()));
        }

        unsafe {
            let ring = self.0.ring;
            let cur = (*ring).cur;
            let slot = (*ring).slot.as_mut_ptr().add(cur as usize);
            let dst = ffi::NETMAP_BUF(ring, (*slot).buf_idx) as *mut u8;

            // copy data to the slot
            ptr::copy_nonoverlapping(buf.as_ptr(), dst, buf.len());

            (*slot).len = buf.len() as u16;
            (*ring).head = (*ring).cur.wrapping_add(1);
            (*ring).cur = (*ring).head;

            Ok(())
        }
    }

    /// get the maximum payload size for this ring
    pub fn max_payload_size(&self) -> usize {
        unsafe { (*self.0.ring).nr_buf_size as usize }
    }

    /// reserve space for batch sending
    pub fn reserve_batch(&mut self, count: usize) -> Result<BatchReservation<'a>, Error> {
        unsafe {
            let ring_ptr = self.0.ring;
            let head = (*ring_ptr).head;
            let tail = (*ring_ptr).tail;
            let num_slots = (*ring_ptr).num_slots;

            // Calculate available space. Netmap rings are full when head == tail + 1 (modulo num_slots)
            // So, available space is num_slots - 1 - current_used_slots
            // current_used_slots = (head - tail + num_slots) % num_slots
            let current_used_slots = (head.wrapping_sub(tail).wrapping_add(num_slots)) % num_slots;
            let available_slots = (num_slots - 1).saturating_sub(current_used_slots) as usize;

            if available_slots < count {
                return Err(Error::InsufficientSpace);
            }
        }

        Ok(BatchReservation {
            ring: self.0.ring,
            start: unsafe { (*self.0.ring).head },
            count,
            _marker: PhantomData,
        })
    }
}

/// a batch reservation for tx packets
pub struct BatchReservation<'a> {
    ring: *mut ffi::netmap_ring,
    start: u32,
    count: usize,
    _marker: PhantomData<&'a mut ffi::netmap_ring>,
}

impl<'a> BatchReservation<'a> {
    /// get a mutable slice for packet in the batch
    pub fn packet(&mut self, index: usize, len: usize) -> Result<&mut [u8], Error> {
        if index >= self.count {
            return Err(Error::InvalidRingIndex(index));
        }

        unsafe {
            let slot_idx = (self.start + index as u32) % (*self.ring).num_slots;
            let slot = (*self.ring).slot.as_mut_ptr().add(slot_idx as usize);
            (*slot).len = len as u16;
            let src = ffi::NETMAP_BUF(self.ring, (*slot).buf_idx) as *mut u8;
            Ok(slice::from_raw_parts_mut(src, len))
        }
    }

    /// commit the batch (make packets visible to NIC)
    pub fn commit(self) {
        unsafe {
            (*self.ring).head = self.start + self.count as u32;
            (*self.ring).cur = (*self.ring).head;
        }
    }
}

impl<'a> RxRing<'a> {
    /// create a new rx ring
    pub(crate) fn new(ring: *mut ffi::netmap_ring, fd: i32, index: usize) -> Self {
        let mut r = Ring::new(ring, fd, index);
        r.direction = RingDirection::Rx;
        Self(r)
    }

    /// receive single packet
    pub fn recv(&mut self) -> Option<Frame> {
        unsafe {
            let ring = self.0.ring;
            if (*ring).head == (*ring).tail {
                return None;
            }

            let slot_idx = (*ring).head % (*ring).num_slots;
            let slot = (*ring).slot.as_mut_ptr().add(slot_idx as usize);
            let src = ffi::NETMAP_BUF(ring, (*slot).buf_idx) as *const u8;
            let buf = slice::from_raw_parts(src, (*slot).len as usize);

            (*ring).head = (*ring).head.wrapping_add(1);
            (*ring).cur = (*ring).head;

            Some(Frame::new(buf))
        }
    }

    /// receive a  batch of packets
    pub fn recv_batch(&mut self, batch: &mut [Frame]) -> usize {
        unsafe {
            let ring = self.0.ring;
            let avail = (*ring).tail.wrapping_sub((*ring).head) as usize;
            let count = avail.min(batch.len());

            for (i, frame) in batch.iter_mut().take(count).enumerate() {
                let slot_idx = ((*ring).head + i as u32) % (*ring).num_slots;
                let slot = (*ring).slot.as_mut_ptr().add(slot_idx as usize);
                let src = ffi::NETMAP_BUF(ring, (*slot).buf_idx) as *const u8;
                let buf = slice::from_raw_parts(src, (*slot).len as usize);

                *frame = Frame::new(buf);
            }
            (*ring).head = (*ring).head.wrapping_add(count as u32);
            (*ring).cur = (*ring).head;

            count
        }
    }
}
