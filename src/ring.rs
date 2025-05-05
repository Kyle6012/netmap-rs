use std::marker::PhantomData;
use std::ptr;
use std::slice;

use crate::error::Error;
use crate::frame::Frame;
use crate::ffi;

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
    pub(crate) fn new(ring: *mut ffi::netmap_ring, index: usize) -> Self{
        Self {
            ring. 
            index,
            _marker: PhantomData,
        }
    }

    /// Get the ring index
    pub fn index(&self) -> usize  {
        unsafe { (*self.ring).num_slots as usize }
    }

    /// sync the ring with the NIC
    pub fn sync(&self) {
        unsafe {
            if (*self.ring).flags & ffi::NR_TX as u16 !=0 {
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
            ptr::copy_nonoverlapping(
                buf.as_ptr(),
                (*slot).buf as *mut u8,
                buf.len(),
            );

            (*slot).len = buf.len() as u16;
            (*ring).head = (*ring).cur.wrapping_add(1);
            (*ring).cur = (*ring).head;

            Ok(())
        }
    }
}