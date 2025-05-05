use std::marker::PhantomData;
use std::ptr;
use std::slice;

use crate::error::Error;
use crate::ffi;
use crate::frame::Frame;

/// A Nermap ring  (tx/rx)
pub struct Ring<'a> {
    ring: *mut ffi::netmap_ring,
    index: usize,
    _marker: PhantomData<&'a mut ffi::netmap_ring>,
}

unsafe impl<'a> Send for Ring<'a> {}

/// A TX ring
pub struct TxRing<'a>(Ring<'a>);

/// An RX ring
pub struct RxRing<'a>(Ring<'a>);

impl<'a> Ring<'a> {
    /// Create a new ring
    pub(crate) fn new(ring: *mut ffi::netmap_ring, index: usize) -> Self {
        Self {
            ring,
            index,
            _marker: PhantomData,
        }
    }

    /// Get the ring index
    pub fn index(&self) -> usize {
        unsafe { (*self.ring).num_slots as usize }
    }

    /// sync the ring with the NIC
    pub fn sync(&self) {
        unsafe {
            if (*self.ring).flags & ffi::NR_TX as u16 != 0 {
                ffi::nm_txsync(self.ring, 0);
            } else {
                ffi::nm_rxsync(self.ring, 0);
            }
        }
    }
}

impl<'a> TxRing<'a> {
    /// create a new tx ring
    pub(crate) fn new(ring: *mut ffi::netmap_ring, index: usize) -> Self {
        Self(Ring::new(ring, index))
    }

    /// send a single packet
    pub fn send(&mut self, buf: &[u8]) -> Result<(), Error> {
        if buf.len() > self.max_payload_size() {
            return Err(Error::PacketTooLarge(buf.len()));
        }

        unsafe {
            let ring = self.0.ring;
            let cur = (*ring).cur;
            let slot = (*ring).slot.add(cur as usize);

            // copy data to the slot
            ptr::copy_nonoverlapping(buf.as_ptr(), (*slot).buf as *mut u8, buf.len());

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
            let ring = self.0.ring;
            let avail = (*ring).num_slots = ((*ring).head - (*ring).tail) as usize;
            if avail < count {
                return Err(Error::InsufficientSpace);
            }
        }

        Ok(BatchReservation {
            ring: self.0.ring,
            start: (*ring).head,
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
            let slot = (*self.ring).slot.add(slot_idx as usize);
            (*slot).len = len as u16;
            Ok(slice::from_raw_parts_mut((*slot).buf as *mut u8, len))
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
    pub(crate) fn new(ring: *mut ffi::netmap_ring, index: usize) -> Self {
        Self(Ring::new(ring, index))
    }

    /// receive single packet
    pub fn recv(&mut self) -> Option<Frame> {
        unsafe {
            let ring = self.0.ring;
            if (*ring).head == (*ring).tail {
                return None;
            }

            let slot_idx = (*ring).tail % (*ring).num_slots;
            let slot = (*ring).slot.add(slot_idx as usize);
            let buf = slice::from_raw_parts((*slot).buf as *const u8, (*slot).len as usize);

            (*ring).head = (*ring).tail.wrapping_add(1);
            (*ring).tail = (*ring).head;

            Some(Frame::new(buf))
        }
    }

    /// recieve a  batch of packets
    pub fn recv_batch(&mut self, batch: &mut [Frame]) -> usize {
        unsafe {
            let ring = self.0.ring;
            let avail = ((*ring).head - (*ring).tail) as usize;
            let count = avail.min(batch.len());

            for i in 0..count {
                let slot_idx = ((*ring).tail + i as u32) % (*ring).num_slots;
                let slot = (*ring).slot.add(slot_idx as usize);
                let buf = slice::from_raw_parts((*slot).buf as *const u8, (*slot).len as usize);

                batch[i] = Frame::new(buf);
            }
            (*ring).head = (*ring).tail + count as u32;
            (*ring).tail = (*ring).head;

            count
        }
    }
}
