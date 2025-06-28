# netmap-min-sys

This crate provides raw FFI (Foreign Function Interface) bindings to the [netmap](http://info.iet.unipi.it/~luigi/netmap/) C library. Netmap is a framework for very fast packet I/O from userspace.

`netmap-min-sys` is a typical `*-sys` crate, meaning it does not provide any high-level abstractions or safety guarantees over the C API. It is intended to be used as a foundation for higher-level Rust crates that can provide safe and idiomatic interfaces to netmap (such as the [`netmap-rs`](https://crates.io/crates/netmap-rs) crate).

## Features

*   Raw FFI bindings to `netmap_user.h` and `netmap.h`.
*   Bindings are generated using `bindgen` at build time.
*   Conditional compilation of bindings via the `sys` feature flag.

## Prerequisites

To build and use this crate, you need to have the netmap kernel module installed and the netmap header files available on your system. Please refer to the official [netmap documentation](https://github.com/luigirizzo/netmap) for instructions on how to install and configure netmap.

Typically, this involves:
1.  Cloning the netmap source code: `git clone https://github.com/luigirizzo/netmap.git`
2.  Compiling and installing the kernel module and headers (refer to netmap's `README.md` for specific instructions for your OS).

## Usage

Add this crate as a dependency in your `Cargo.toml`:

```toml
[dependencies]
netmap-min-sys = "your_desired_version" # Replace with the actual version
```

By default, the `sys` feature is often enabled by dependent crates that need to link against the actual netmap library. If you only need the type definitions, you might be able to disable default features, but typical usage involves linking.

### Example (Conceptual - from a C perspective)

Since this crate provides raw bindings, using it directly in Rust involves `unsafe` code. Here's a conceptual idea of what the C functions being bound do:

```c
// (This is C code, not Rust, for illustration)
#include <net/netmap_user.h>
#include <stdio.h>
#include <poll.h>

// ... (code to open a netmap interface using nm_open)
// ... (code to interact with netmap_if, netmap_ring, netmap_slot)
// ... (code to use poll() for waiting for packets)
// ... (code to close the interface using nm_close)
```

A Rust crate using `netmap-min-sys` would call these C functions via the generated FFI bindings.

## Building

The crate uses `bindgen` to generate Rust bindings from the C header files specified in `wrapper.h`. This process happens automatically when you build the crate, provided the netmap headers are discoverable by your C compiler/`bindgen`.

## Author

*   Meshack Bahati (Kenya)

## License
*   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE)).
*   MIT license ([LICENSE-MIT](LICENSE-MIT)).


---
