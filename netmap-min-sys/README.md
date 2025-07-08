# netmap-min-sys

This crate provides low-level FFI (Foreign Function Interface) bindings to the Netmap C library. Netmap is a framework for very fast packet I/O from userspace.

`netmap-min-sys` is a "sys" crate, meaning it primarily handles the C library linking and exposes raw, unsafe bindings. Higher-level, safe abstractions are provided by the `netmap-rs` crate, which depends on this one.

## Prerequisites

To compile and use this crate (and by extension, `netmap-rs` with its `sys` feature), you must have the Netmap C library and its development headers installed on your system.

This typically involves:
1.  **Installing Netmap:** Follow the instructions from the [official Netmap project](http://info.iet.unipi.it/~luigi/netmap/) or its [GitHub repository (netmap/netmap)](https://github.com/netmap/netmap) to compile and install the Netmap kernel module, libraries, and headers for your operating system.
2.  **Installing Clang:** The `bindgen` tool, used by this crate's build script to generate Rust bindings from C headers, requires `clang` to be installed. (e.g., `sudo apt install clang libclang-dev` on Debian/Ubuntu).

## Build Configuration

The build script (`build.rs`) for `netmap-min-sys` attempts to locate your Netmap installation.

### Standard Installation

If Netmap is installed in a standard system location (e.g., headers in `/usr/include` or `/usr/local/include`, and libraries in `/usr/lib` or `/usr/local/lib`), the build script should generally find it automatically. It defaults to checking `/usr/local` if no other hints are provided. The `build.rs` script was updated to explicitly pass the include path `${NETMAP_LOCATION}/include` (or the default `/usr/local/include`) to `bindgen`.

### Custom Netmap Installation Path (`NETMAP_LOCATION`)

If you have installed Netmap in a non-standard directory, you **must** inform the build script by setting the `NETMAP_LOCATION` environment variable before building your project. Set this variable to the root directory of your Netmap installation (i.e., the directory that contains the `include` and `lib` subdirectories for Netmap).

**Example:**

If Netmap is installed in `/opt/netmap` (so headers are in `/opt/netmap/include` and libraries in `/opt/netmap/lib`), you would build your project like this:

```bash
NETMAP_LOCATION=/opt/netmap cargo build
```

The build script will then:
*   Instruct `bindgen` to look for headers in `$NETMAP_LOCATION/include` (e.g., `/opt/netmap/include`).
*   Instruct the linker to look for libraries in `$NETMAP_LOCATION/lib` (e.g., `/opt/netmap/lib`).

### Disabling Netmap Kernel Integration (`DISABLE_NETMAP_KERNEL`)

For compilation on platforms where Netmap is not supported or available (e.g., macOS, Windows), or if you wish to compile `netmap-rs` without actual Netmap functionality (perhaps for using only its fallback mechanisms if any are fully independent), you can set the `DISABLE_NETMAP_KERNEL` environment variable.

```bash
DISABLE_NETMAP_KERNEL=1 cargo build
```

If this variable is set, the build script will:
*   Skip the `bindgen` process.
*   Generate an empty `binding.rs` file.
*   Not attempt to link against the Netmap library.

This allows the crate (and `netmap-rs`) to compile, but any attempt to use Netmap-specific functions will likely fail or be unavailable.

## Usage

This crate is not typically used directly. Instead, the `netmap-rs` crate provides safe Rust abstractions over the raw bindings exposed here. If you are using `netmap-rs`, ensure its `sys` feature is enabled, which will correctly pull in and configure this `-sys` crate.

## License

Licensed under either of
* Apache License, Version 2.0, (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)
at your option.
