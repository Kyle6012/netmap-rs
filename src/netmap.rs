#![cfg(feature = "sys")]

use std::ffi::CString;
use std::marker::PhantomData;
use std::os::unix::io::{AsRawFd, RawFd};
use std::ptr;
use std::sync::Arc;

use crate::error::Error;
use crate::ffi;
use crate::ring::{Ring, RxRing, TxRing};

/// Builder for configuring and opening a Netmap interface.
///
/// This builder allows specifying the interface name, number of TX/RX rings,
/// and other flags. It supports Netmap's convention for accessing host stack
/// rings by appending a `^` to the interface name (e.g., "netmap:eth0^").
///
/// # Examples
///
/// ```no_run
/// use netmap_rs::NetmapBuilder;
///
/// // Open hardware rings of eth0
/// let nm_hw = NetmapBuilder::new("netmap:eth0")
///     .num_tx_rings(2)
///     .num_rx_rings(2)
///     .build();
///
/// // Open host stack rings of eth0
/// let nm_host = NetmapBuilder::new("netmap:eth0^")
///     .num_tx_rings(1)
///     .num_rx_rings(1)
///     .build();
/// ```
pub struct NetmapBuilder {
    ifname_raw: String, // Stores the raw interface name as provided by user
    // Parsed from ifname_raw, without netmap: prefix or ^/* suffixes.
    // For OS interfaces, this is the OS name (e.g. "eth0").
    // For VALE/pipes, this is the full VALE/pipe name (e.g. "vale0:1", "pipe{abc").
    base_ifname: String,
    wants_host_rings: bool, // True if ifname ends with '^'
    is_pipe_if: bool,       // True if ifname is a pipe (e.g. "pipe{name}")

    // These will be interpreted as HW, Host, or Pipe rings based on above flags
    req_num_tx_rings: u16,
    req_num_rx_rings: u16,

    // For nr_arg1, nr_arg2 (slots, etc.) - advanced usage
    // For now, let Netmap decide these based on rings, or expose later.
    // req_tx_slots: u32,
    // req_rx_slots: u32,

    /// For `nr_flags` like `NETMAP_NO_TX_POLL`, `NETMAP_DO_RX_POLL`, etc.
    /// Registration mode flags (`NR_REG_*`) will be handled internally based on ifname suffix.
    additional_flags: u32,
}

impl NetmapBuilder {
    /// Creates a new builder for the given Netmap interface name.
    ///
    /// The `ifname_str` specifies the network interface to use with Netmap.
    /// It can be a simple OS interface name (e.g., "eth0"), which will typically be
    /// prefixed with "netmap:" by this library if not already present and if it does not
    /// appear to be a VALE or pipe interface name.
    ///
    /// To access **host stack rings** for an interface, append `^` to the
    /// interface name (e.g., "eth0^" or "netmap:eth0^"). This signals Netmap
    /// to attach to the host's network stack for that interface rather than
    /// the hardware rings directly.
    ///
    /// To access **VALE switch ports**, use the `valeXXX:portYYY` syntax
    /// (e.g., "vale0:1"). For **Netmap pipes**, use names like "pipe{123".
    ///
    /// The "netmap:" prefix is optional for simple OS interface names; if not provided,
    /// it will be added. For VALE ports or pipes, provide the full Netmap-specific name
    /// (e.g., "vale0:myport", "pipe{abc").
    ///
    /// # Examples
    ///
    /// ```
    /// use netmap_rs::NetmapBuilder;
    ///
    /// let builder_hw = NetmapBuilder::new("eth0"); // For hardware rings, "netmap:eth0" is used.
    /// let builder_host = NetmapBuilder::new("netmap:em1^"); // For host stack rings of em1.
    /// let builder_vale = NetmapBuilder::new("vale0:myport"); // For a VALE port.
    /// let builder_pipe = NetmapBuilder::new("pipe{myipc}"); // For a Netmap pipe.
    /// ```
    pub fn new(ifname_str: &str) -> Self {
        let mut raw_name_to_use = ifname_str.to_string();
        let mut base_name_for_req = ifname_str;

        // Handle "netmap:" prefix intelligently
        if let Some(stripped_prefix) = ifname_str.strip_prefix("netmap:") {
            base_name_for_req = stripped_prefix;
        } else {
            // If no "netmap:" prefix, and it's not a VALE/pipe/special name, add it.
            if !ifname_str.contains(':') && !ifname_str.starts_with("pipe{") {
                raw_name_to_use = format!("netmap:{}", ifname_str);
                // base_name_for_req remains ifname_str for nr_name, netmap expects base OS name.
            }
            // If it contains ':' (like "vale0:1") or is "pipe{...", use as is for raw_name_to_use.
            // base_name_for_req will be this full name for nmreq.nr_name for VALE/pipes.
        }

        let wants_host_rings = base_name_for_req.ends_with('^');
        let mut effective_base_name_for_req = base_name_for_req.to_string();
        if wants_host_rings {
            effective_base_name_for_req.pop(); // Remove '^' for nr_name
        }
        // VALE/pipe names like "vale0:1" or "pipe{abc" go directly into nr_name.
        // OS interface names like "eth0" also go into nr_name.
        // The raw_name_to_use (e.g. "netmap:eth0^") is for nm_open's first argument.

        let is_pipe = base_name_for_req.starts_with("pipe{") && base_name_for_req.ends_with('}');

        // For pipes, default to 1 TX and 1 RX ring if user doesn't specify.
        // For other types, default to 0 (all available).
        let default_rings = if is_pipe { 1 } else { 0 };

        Self {
            ifname_raw: raw_name_to_use,
            base_ifname: effective_base_name_for_req, // This is what goes into nmreq.nr_name
            wants_host_rings,
            is_pipe_if: is_pipe,
            req_num_tx_rings: default_rings,
            req_num_rx_rings: default_rings,
            additional_flags: 0,
        }
    }

    /// Sets the desired number of TX rings.
    ///
    /// - If the interface name provided to `new()` ends with `^` (for host rings),
    ///   this configures the number of **host TX rings**.
    /// - If the interface name is for a Netmap pipe (e.g., "pipe{name}"), this configures
    ///   the TX rings for that pipe endpoint. Netmap pipes typically have 1 TX ring.
    ///   Setting `num` to 0 defaults to 1 for pipes; requesting more than 1 may not be supported.
    /// - Otherwise, it configures **hardware TX rings**.
    ///
    /// Setting `num` to 0 generally means Netmap will attempt to allocate all available rings
    /// of the requested type. For pipes, the default (and typical maximum) is 1.
    pub fn num_tx_rings(mut self, num: usize) -> Self {
        self.req_num_tx_rings = num as u16;
        self
    }

    /// Sets the desired number of RX rings.
    ///
    /// - If the interface name provided to `new()` ends with `^` (for host rings),
    ///   this configures the number of **host RX rings**.
    /// - If the interface name is for a Netmap pipe (e.g., "pipe{name}"), this configures
    ///   the RX rings for that pipe endpoint. Netmap pipes typically have 1 RX ring.
    ///   Setting `num` to 0 defaults to 1 for pipes; requesting more than 1 may not be supported.
    /// - Otherwise, it configures **hardware RX rings**.
    ///
    /// Setting `num` to 0 generally means Netmap will attempt to allocate all available rings
    /// of the requested type. For pipes, the default (and typical maximum) is 1.
    pub fn num_rx_rings(mut self, num: usize) -> Self {
        self.req_num_rx_rings = num as u16;
        self
    }

    /// Sets additional flags for the Netmap request (`struct nmreq`'s `nr_flags` field).
    ///
    /// These flags are ORed with internally determined flags (such as those for
    /// registration mode, like `NR_REG_SW_ONLY` or `NR_REG_NIC_ONLY`, which are
    /// inferred from the interface name and its suffixes).
    ///
    /// Use this method to specify operational flags like `NETMAP_NO_TX_POLL`,
    /// `NETMAP_DO_RX_POLL`, `NETMAP_BDG_POLLING`, etc. Refer to the Netmap
    /// header `<net/netmap_user.h>` for the full list of available `nr_flags`.
    ///
    /// # Example
    /// ```no_run
    /// use netmap_rs::NetmapBuilder;
    /// # mod ffi { pub const NETMAP_NO_TX_POLL: u32 = 0x0004; } // Mock ffi for example
    ///
    /// let builder = NetmapBuilder::new("eth0")
    ///     .flags(ffi::NETMAP_NO_TX_POLL);
    /// ```
    pub fn flags(mut self, flags: u32) -> Self {
        self.additional_flags = flags;
        self
    }

    fn build_nmreq(&self) -> Result<ffi::nmreq, Error> {
        // Ensure base_ifname fits in nr_name (IFNAMSIZ - 1 for null terminator)
        if self.base_ifname.len() >= ffi::IFNAMSIZ as usize {
            return Err(Error::BindFail(format!(
                "Base interface name '{}' is too long.",
                self.base_ifname
            )));
        }
        let mut nr_name_bytes = [0i8; ffi::IFNAMSIZ as usize];
        for (i, byte) in self.base_ifname.bytes().enumerate() {
            nr_name_bytes[i] = byte as i8;
        }

        let mut req_flags = self.additional_flags;
        let mut hw_tx_rings = 0;
        let mut hw_rx_rings = 0;
        let mut host_tx_rings = 0;
        let mut host_rx_rings = 0;

        if self.is_pipe_if {
            // For pipes, nr_tx_rings and nr_rx_rings specify the rings for this endpoint.
            // nr_host_* rings are 0. No specific NR_REG_* flag is needed here,
            // as the pipe name itself implies the type.
            hw_tx_rings = self.req_num_tx_rings; // Netmap uses these for pipes
            hw_rx_rings = self.req_num_rx_rings; // Netmap uses these for pipes
        } else if self.wants_host_rings {
            req_flags |= ffi::NR_REG_SW_ONLY; // Request only host stack rings
            host_tx_rings = self.req_num_tx_rings;
            host_rx_rings = self.req_num_rx_rings;
            // hw_tx_rings and hw_rx_rings remain 0
        } else {
            // Default behavior: request hardware rings for physical/VALE interfaces.
            req_flags |= ffi::NR_REG_NIC_ONLY; // Request only NIC rings
            hw_tx_rings = self.req_num_tx_rings;
            hw_rx_rings = self.req_num_rx_rings;
            // host_tx_rings and host_rx_rings remain 0
        }

        Ok(ffi::nmreq {
            nr_name: nr_name_bytes,
            nr_version: ffi::NETMAP_API as u16,
            nr_offset: 0,
            nr_memsize: 0,
            nr_tx_slots: 0,  // Let netmap decide by default, or allow configuration later
            nr_rx_slots: 0,  // Let netmap decide by default
            nr_tx_rings: hw_tx_rings, // For pipes, these are used for the pipe's TX rings
            nr_rx_rings: hw_rx_rings, // For pipes, these are used for the pipe's RX rings
            nr_host_tx_rings: host_tx_rings,
            nr_host_rx_rings: host_rx_rings,
            nr_ringid: 0, // Request all rings for the component (NIC/host/pipe endpoint)
            nr_flags: req_flags,
            nr_arg1: 0,
            nr_arg2: 0,
            nr_arg3: 0, // Renamed from spare in newer netmap versions
            spare1: [0; 1], // Keep spare for compatibility if arg3 is not yet in ffi bindings
        })
    }

    /// Consumes the builder and attempts to open the Netmap interface.
    pub fn build(self) -> Result<Netmap, Error> {
        let req = self.build_nmreq()?;

        // Use the raw ifname (e.g., "netmap:eth0^") for nm_open, as netmap parses it.
        let c_ifname_raw = CString::new(self.ifname_raw.as_str())
            .map_err(|_| Error::BindFail(format!("Invalid raw interface name: {}", self.ifname_raw)))?;

        // The actual nm_open call
        // The third argument to nm_open (nm_ifp) is for reusing memory from another descriptor, pass null.
        let desc_ptr = unsafe { ffi::nm_open(c_ifname_raw.as_ptr(), &req as *const _, ptr::null_mut(), ptr::null_mut()) };

        if desc_ptr.is_null() {
            return Err(Error::BindFail(format!(
                "Failed to open interface via nm_open for '{}'. Errno: {}",
                self.ifname_raw, std::io::Error::last_os_error()
            )));
        }

        // Determine actual number of rings available from the descriptor
        let nifp = unsafe { (*desc_ptr).nifp };
        let (actual_num_tx, actual_num_rx, final_is_host_if) = if self.is_pipe_if {
            // For pipes, counts come from ni_tx_rings and ni_rx_rings, and it's not a host_if.
            (unsafe { (*nifp).ni_tx_rings } as usize, unsafe { (*nifp).ni_rx_rings } as usize, false)
        } else if self.wants_host_rings {
            (unsafe { (*nifp).ni_host_tx_rings } as usize, unsafe { (*nifp).ni_host_rx_rings } as usize, true)
        } else {
            (unsafe { (*nifp).ni_tx_rings } as usize, unsafe { (*nifp).ni_rx_rings } as usize, false)
        };

        Ok(Netmap {
            desc: desc_ptr,
            num_tx_rings: actual_num_tx,
            num_rx_rings: actual_num_rx,
            is_host_if: final_is_host_if,
            _marker: PhantomData,
        })
    }
}

/// A Netmap Interface instance, providing access to network rings.
///
/// `Netmap` instances are created using [`NetmapBuilder`](struct.NetmapBuilder.html).
/// It encapsulates the Netmap descriptor and provides methods to access
/// transmission (TX) and reception (RX) rings.
///
/// Depending on how it was built (e.g., with a `^` suffix in the interface name
/// passed to `NetmapBuilder::new`), this instance will provide access to either
/// hardware rings or host stack rings.
pub struct Netmap {
    desc: *mut ffi::nm_desc,
    num_tx_rings: usize, // Actual number of TX rings (either HW or Host based on is_host_if)
    num_rx_rings: usize, // Actual number of RX rings (either HW or Host based on is_host_if)
    is_host_if: bool,    // True if this interface represents host stack rings
    _marker: PhantomData<*mut u8>,
}

unsafe impl Send for Netmap {}

// The direct `Netmap::open()` static method was removed in favor of the builder pattern.
// Use `NetmapBuilder::new(ifname).build()` instead.

impl Netmap {
    /// Returns the number of configured TX rings.
    ///
    /// This count reflects either hardware TX rings or host TX rings,
    /// depending on whether the interface was opened for hardware access
    /// or for host stack interaction (e.g., using an interface name like "eth0^"
    /// with [`NetmapBuilder`](struct.NetmapBuilder.html)).
    pub fn num_tx_rings(&self) -> usize {
        self.num_tx_rings
    }

    /// Returns the number of configured RX rings.
    ///
    /// This count reflects either hardware RX rings or host RX rings,
    /// depending on whether the interface was opened for hardware access
    /// or for host stack interaction (e.g., using an interface name like "eth0^"
    /// with [`NetmapBuilder`](struct.NetmapBuilder.html)).
    pub fn num_rx_rings(&self) -> usize {
        self.num_rx_rings
    }

    /// Returns `true` if this `Netmap` instance is configured for host stack rings.
    ///
    /// This is determined by whether the interface name used to create this instance
    /// (via [`NetmapBuilder`](struct.NetmapBuilder.html)) ended with a `^` suffix
    /// (e.g., "netmap:eth0^").
    ///
    /// If `false`, the instance is configured for hardware rings (or VALE/pipe rings).
    pub fn is_host_if(&self) -> bool {
        self.is_host_if
    }

    /// Gets a handle to a specific Transmission (TX) ring.
    ///
    /// The `index` is relative to the type of rings this `Netmap` instance manages
    /// (hardware or host). For example, if `is_host_if()` is `true`, `tx_ring(0)`
    /// returns the first host TX ring.
    ///
    /// # Errors
    /// Returns `Error::InvalidRingIndex` if the `index` is out of bounds for the
    /// configured number of TX rings.
    pub fn tx_ring(&self, index: usize) -> Result<TxRing, Error> {
        if index >= self.num_tx_rings {
            return Err(Error::InvalidRingIndex(index));
        }

        unsafe {
            let ring = ffi::NETMAP_TXRING((*self.desc).nifp, index as u32);
            Ok(TxRing::new(ring, index))
        }
    }

    /// Gets a handle to a specific Reception (RX) ring.
    ///
    /// The `index` is relative to the type of rings this `Netmap` instance manages
    /// (hardware or host). For example, if `is_host_if()` is `true`, `rx_ring(0)`
    /// returns the first host RX ring.
    ///
    /// # Errors
    /// Returns `Error::InvalidRingIndex` if the `index` is out of bounds for the
    /// configured number of RX rings.
    pub fn rx_ring(&self, index: usize) -> Result<RxRing, Error> {
        if index >= self.num_rx_rings {
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
