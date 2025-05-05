use std::ffi::CString;
use std::marker::PhantomData;
use std::os::unix::io::{AsRawFd,RawFd};
use std::ptr;
use std::sync::Arc;

use crate::error::Error;
use crate::ring::{Ring, RxRing, TxRing};
use crate::ffi;

///builder for configuring a netmap interface

pub struct NetmapBuilder {
    ifname: String,
    num_tx_rings: usize,
    num_rx_rings: usize,
    flags:u32,
}

impl NetmapBuilder {
    /// Creates a new buider for the given interface
    pub fn new(ifname: &str) -> Self {
        Self {
            ifname: ifname.to_string(),
            num_tx_rings: 1,
            num_rx_rings: 1,
            flags: 0,
        }
    }

    pub fn num_tx_rings(mut self, num: usize) -> Self {
        self.num_tx_rings = num;
        self
    }

    pub fn num_rx_rings(mut self, num: usize) -> Self {
        self.num_rx_rings = num;
        self
    }

    pub fn flags(mut self, flags: u32) -> Self {
        self.flags = flags;
        self
    }

    /// Open Netmap Interface
    pub fn open(self) -> Result<Netmap, Error> {
        Netmap::open(&self.ifname, self.num_tx_rings, self.num_rx_rings, self.flags)
    }
}

/// A Netmap Interface instance
pub struct Netmap {
    desc: *mut ffi::nm_desc,
    num_tx_rings: usize,
    num_rx_rings: usize,
    // ENSURES NETMAP IS SED NOT SYNC
    _marker: PhantomData<*mut u8>,
}

unsafe impl Send for Netmap {}

impl Netmap {
    /// Open a Netmap interface with default settings 
    pub fn open (ifname: &str, num_tx_rings: usize, num_rx_rings: usize, flags: u32) -> Result<Self, Error> {
        let c_ifname = CString::new(ifname).map_err(|_| Error::BindFail("Invalid interface name".to_string()))?;
        
        let req = ffi::nmreq  {
            nr_name: [0; ffi::NM_IFNAMSIZE as usize],
            nr_version: ffi::NETMAP_API,
            nr_offset: 0,
            nr_memsize: 0,
            nr_tx_slots: 0,
            nr_rx_slots: 0,
            nr_tx_rings: num_tx_rings as u16,
            nr_rx_rings: num_rx_rings as u16,
            nr_ringid: 0,
            nr_flags: flags,
            nr_arg1: 0,
            nr_arg2: 0,
            spare: [0; 2],
        };

        let desc = unsafe {
            ffi::nm_open(c_ifname.as_ptr(), & req as *const _. flags, ptr::null_mut())
        };

        if desc.is_null() {
            return Err(Error::BindFail(format!("Failed to open interface {}", ifname)));
        }

        Ok(Self {
            desc,
            num_tx_rings,
            num_rx_rings,
            _marker: PhantomData,
        })
    }

    /// Get number of TX rings
    pub fn num_tx_rings(&self) -> usize {
        self.num_tx_rings
    }

    /// Get number of RX rings
    pub fn num_rx_rings(&self) -> usize {
        self.num_rx_rings
    }

    /// Get a TX ring by index
    pub fn tx_ring(&self, index: usize) -> Result<TxRing, Error> {
        if index >= self.num_tx_rings {
            return Err(Error:: InvalidRingIndex(index));
        }

        unsafe {
            letring = ffi::NETMAP_TXRING((*self.desc).nifp, index as u32);
            Ok(TxRing::new(ring, index))
        }
    }

    /// Get RX ring by index
    pub fn rx_ring(&self, index: usize) -> Result<TxRing, Error> {
        if index >= self.num_rx_rings{
            return Err(Error::InvalidRingIndex(index));
        }
        unsafe {
            let ring = ffi::NETMAP_RXRING((*self.desc).nifp, index as u32);
            Ok(RxRing::new(ring, index))
        }
    }
}

impl Drop for Netmap {
    fn drop(&mut self) {
        unsafe {
            ffi::nm_close(self.desc);
        }
    }
}

impl AsRawFd for Netmap {
    fn as_raw_fd(&self) -> RawFd {
        unsafe { (*self.desc).fd }
    }
}
