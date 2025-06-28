# netmap-rs

[![Crates.io](https://img.shields.io/crates/v/netmap-rs.svg)](https://crates.io/crates/netmap-rs)
[![Docs.rs](https://docs.rs/netmap-rs/badge.svg)](https://docs.rs/netmap-rs)

Safe, idiomatic, and zero-cost abstractions for the [Netmap](http://info.iet.unipi.it/~luigi/netmap/) kernel-bypass networking framework.

Netmap allows for extremely fast packet I/O by bypassing the kernel's network stack, enabling direct communication between applications and network hardware. `netmap-rs` provides a Rust interface to these capabilities, prioritizing safety and ease of use without sacrificing performance.

This crate builds upon the raw FFI bindings provided by [`netmap-min-sys`](https://crates.io/crates/netmap-min-sys) (when the `sys` feature is enabled).

## Features

-   **Zero-Copy Packet I/O**: Directly access packet buffers in memory-mapped regions for maximum throughput and minimum latency.
-   **Safe Abstractions**: Wraps unsafe C FFI calls in safe Rust APIs.
-   **High-Level Interface**: Provides `Netmap`, `TxRing`, `RxRing`, and `Frame` types for easy management of netmap resources.
-   **Builder Pattern**: Fluent `NetmapBuilder` for configuring netmap interfaces.
-   **Batch Operations**: Efficiently send and receive packets in batches.
-   **Fallback Mode**: Includes a software-based fallback implementation for platforms or environments where native Netmap is unavailable, allowing for broader compatibility and easier development.
-   **Thread-per-Ring Design**: Facilitates architectures where each network ring is managed by a dedicated thread, often pinned to a specific core for performance.

## Prerequisites

-   **For Native Netmap (Linux/FreeBSD)**:
    -   Netmap kernel module installed and loaded.
    -   Netmap header files available.
    -   See the [official netmap documentation](https://github.com/luigirizzo/netmap) for installation instructions.
-   **Rust**: Version 1.60 or later.

## Usage

Add `netmap-rs` to your `Cargo.toml`:

```toml
[dependencies]
netmap-rs = "0.1" # Replace with the latest version from crates.io
```

By default, `netmap-rs` tries to use the native Netmap system interface via the `sys` feature. If Netmap is not available or you are on an unsupported platform, it can operate in a fallback mode.

## Basic Example

This example demonstrates how to open a netmap interface, send a packet, and receive it back (e.g., on a loopback interface or if another host sends it back).

```rust
use netmap_rs::prelude::*;
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Error> {
    // Attempt to open a netmap interface.
    // Replace "netmap:eth0" with your desired interface (e.g., "netmap:lo" for loopback).
    // The `.build()?` method finalizes the configuration and opens the interface.
    let nm = NetmapBuilder::new("netmap:eth0") // Or your specific netmap-enabled interface
        .num_tx_rings(1)    // Configure one transmission ring
        .num_rx_rings(1)    // Configure one reception ring
        .build()?;

    // Get handles to the first transmission and reception rings.
    let mut tx_ring = nm.tx_ring(0)?;
    let mut rx_ring = nm.rx_ring(0)?;

    // Prepare a packet to send.
    let packet_data = b"hello netmap-rs!";

    // Send the packet.
    // The `send` method might not transmit immediately; it queues the packet in the ring.
    tx_ring.send(packet_data)?;
    // `sync` on the tx_ring tells the hardware to check for new packets in the ring.
    tx_ring.sync();
    println!("Sent packet: {:?}", packet_data);

    // Attempt to receive the packet.
    // This often requires the interface to be in loopback or for an external entity
    // to send the packet back.
    let mut received = false;
    println!("Attempting to receive packet(s)...");
    for _ in 0..10 { // Try a few times with a delay
        // `sync` on the rx_ring updates its state to see new packets from the hardware
        // and tells the kernel we are done with previously received packets.
        rx_ring.sync();
        while let Some(frame) = rx_ring.recv() {
            println!("Received packet: {:?}", frame.payload());
            if frame.payload() == packet_data {
                println!("Successfully received the sent packet back!");
                received = true;
            }
            // If expecting multiple packets, continue processing here.
        }
        if received {
            break;
        }
        sleep(Duration::from_millis(200)); // Wait a bit for the packet to arrive/loopback
    }

    if !received {
        eprintln!("Failed to receive the specific packet back. This might be expected depending on your network setup (e.g., if not using a loopback or a dedicated echo server).");
    }

    Ok(())
}
```

For more advanced examples, please see the files in the `examples` directory of the crate. These include:
*   `ping_pong.rs`: A simple ping-pong example.
*   `fec.rs`: Demonstrates Forward Error Correction.
*   `sliding_window_arq.rs`: Implements a basic Automatic Repeat Request (ARQ) for reliable delivery.
*   `thread_per_ring.rs`: Shows a single thread managing a ring pair (can be adapted for fallback mode).
*   `multi_thread_rings.rs`: A more comprehensive example demonstrating how to use multiple TX/RX rings with dedicated threads pinned to CPU cores for each ring pair, suitable for high-performance packet forwarding or processing.
*   `host_receive.rs`: Shows how to receive packets from the host operating system's network stack for a given interface (e.g., by opening "netmap:eth0^").
*   `host_transmit.rs`: Shows how to transmit packets into the host operating system's network stack for a given interface.
*   `pipe_threads.rs`: Demonstrates intra-process (thread-to-thread) communication using a Netmap pipe.
*   `pipe_sender_process.rs` & `pipe_receiver_process.rs`: Example pair for inter-process communication using Netmap pipes.
*   `poll_basic.rs`: Shows basic usage of `poll()` with Netmap file descriptors for non-blocking I/O readiness notification.
*   `tokio_pipe_async.rs`: (Requires `tokio-async` feature) Demonstrates asynchronous packet send/receive over a Netmap pipe using Tokio and the `AsyncNetmap*Ring` wrappers.

## Interacting with the Host Stack

Netmap allows applications to interact directly with the host operating system's network stack, effectively creating a high-speed path to inject packets into the kernel or receive packets from it, bypassing normal socket APIs for the data path. `netmap-rs` supports this by recognizing the `^` suffix on interface names.

When you create a `NetmapBuilder` with an interface name like `"netmap:eth0^"` or `"em1^"`, `netmap-rs` configures the underlying Netmap request to access the host stack rings associated with `eth0` (or `em1`).

```rust
use netmap_rs::prelude::*;

# fn run() -> Result<(), Error> {
// To capture packets from the host stack of 'eth0'
let nm_host_rx = NetmapBuilder::new("netmap:eth0^")
    .num_rx_rings(1) // Request one host RX ring
    .build()?;
let mut host_rx_ring = nm_host_rx.rx_ring(0)?;
// Now use host_rx_ring.recv() to get packets from the host stack

// To inject packets into the host stack of 'eth0'
let nm_host_tx = NetmapBuilder::new("netmap:eth0^")
    .num_tx_rings(1) // Request one host TX ring
    .build()?;
let mut host_tx_ring = nm_host_tx.tx_ring(0)?;
// Now use host_tx_ring.send() to inject packets into the host stack
# Ok(())
# }
```

The `Netmap::is_host_if()` method can be used to check if a `Netmap` instance is configured for host stack rings. The `examples/host_receive.rs` and `examples/host_transmit.rs` files provide practical demonstrations. Accessing host stack rings typically requires appropriate system permissions (e.g., root).

## Using Netmap Pipes for IPC

Netmap pipes provide a mechanism for zero-copy inter-process communication (IPC) or intra-process (thread-to-thread) communication. A pipe is identified by a unique name, and each endpoint of the pipe typically gets one transmission (TX) ring and one reception (RX) ring.

To use a Netmap pipe with `netmap-rs`:
1.  Choose a unique name for your pipe, e.g., "my_pipe_1".
2.  Use `NetmapBuilder::new("pipe{my_pipe_1}")` (or `"netmap:pipe{my_pipe_1}"`) to create builders for each endpoint.
3.  Call `.build()` on each builder to get `Netmap` instances. The first successful open creates the pipe and becomes its master; subsequent opens of the same pipe name attach as slaves/peers.
4.  Use the TX ring of one endpoint to send data and the RX ring of the other endpoint to receive it. Communication is bidirectional.

```rust
use netmap_rs::prelude::*;

# fn run() -> Result<(), Error> {
const PIPE_ID: &str = "pipe{example_ipc}";

// Endpoint A (e.g., in one thread or process)
let nm_pipe_a = NetmapBuilder::new(PIPE_ID)
    // .num_tx_rings(1) // Optional, defaults to 1 for pipes
    // .num_rx_rings(1) // Optional, defaults to 1 for pipes
    .build()?;
let mut tx_a = nm_pipe_a.tx_ring(0)?;
let mut rx_a = nm_pipe_a.rx_ring(0)?;

// Endpoint B (e.g., in another thread or process)
let nm_pipe_b = NetmapBuilder::new(PIPE_ID).build()?;
let mut tx_b = nm_pipe_b.tx_ring(0)?;
let mut rx_b = nm_pipe_b.rx_ring(0)?;

// Thread A sends to Thread B
// tx_a.send(b"Hello from A")?;
// tx_a.sync();
// let received_by_b = rx_b.recv();

// Thread B can send back to Thread A
// tx_b.send(b"Reply from B")?;
// tx_b.sync();
// let received_by_a = rx_a.recv();
# Ok(())
# }
```
Refer to `examples/pipe_threads.rs` for a thread-to-thread example and `examples/pipe_sender_process.rs` / `examples/pipe_receiver_process.rs` for inter-process examples.

## Polling and Asynchronous Operations

### Basic Polling
Netmap file descriptors can be used with polling mechanisms like `poll()`, `select()`, `epoll`, or `kqueue` to wait for I/O readiness without busy-looping. The `Netmap` struct implements `AsRawFd` to provide the necessary file descriptor.
- `POLLIN` typically indicates that an RX ring has packets available (after an `rx_ring.sync()`) or a TX ring has space.
- `POLLOUT` typically indicates that a TX ring has space available for sending.
The `examples/poll_basic.rs` demonstrates using the `polling` crate for this purpose. Remember that Netmap's `poll` is generally level-triggered, and `sync()` calls on rings are crucial after readiness events.

### Tokio Asynchronous Support (Optional Feature)
`netmap-rs` provides support for asynchronous operations using Tokio via the `tokio-async` feature flag. To enable it, add to your `Cargo.toml`:
```toml
netmap-rs = { version = "0.1", features = ["tokio-async", "sys"] }
```
This feature provides the following types in the `netmap_rs::tokio_async` module (also re-exported at `netmap_rs::*` when the feature is enabled):
- `TokioNetmap`: Wraps a `Netmap` instance to integrate with Tokio's event loop.
- `AsyncNetmapRxRing`: Implements `tokio::io::AsyncRead` for a Netmap RX ring.
- `AsyncNetmapTxRing`: Implements `tokio::io::AsyncWrite` for a Netmap TX ring.

```rust
# #[cfg(all(feature = "tokio-async", feature="sys"))]
# async fn run_async_example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
use netmap_rs::{NetmapBuilder, TokioNetmap}; // TokioNetmap is available if feature is on
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// 1. Open Netmap interface and wrap with TokioNetmap
let netmap_desc = NetmapBuilder::new("netmap:pipe{my_async_pipe}").build()?;
let tokio_netmap = TokioNetmap::new(netmap_desc)?;

// 2. Get async ring handles
let mut async_tx_ring = tokio_netmap.tx_ring(0)?;
let mut async_rx_ring = tokio_netmap.rx_ring(0)?;

// 3. Use in async code
// async_tx_ring.write_all(b"hello async").await?;
// async_tx_ring.flush().await?;
// let mut buf = [0u8; 128];
// let n = async_rx_ring.read(&mut buf).await?;
# Ok(())
# }
```
The `examples/tokio_pipe_async.rs` example shows these types in action.

**Important Note on Async Wrappers:** The current `AsyncRead` and `AsyncWrite` implementations have placeholders for the crucial `ioctl` calls (`NIOCRXSYNC`, `NIOCTXSYNC`) needed for proper Netmap ring synchronization with the kernel. **These `ioctl`s MUST be correctly implemented within the async wrappers for them to function reliably.** Refer to the docstrings in `src/tokio_async.rs` for more details.

## Platform Support

| Platform        | Native Netmap Status | Fallback Mode | Notes                                                                 |
|-----------------|----------------------|---------------|-----------------------------------------------------------------------|
| Linux           | ✅ Supported         | ✅ Available  | Requires Netmap kernel module and headers.                            |
| FreeBSD         | ✅ Supported         | ✅ Available  | Netmap originated on FreeBSD.                                         |
| macOS           | ⚠️ Experimental     | ✅ Available  | Native support might be limited or require specific configurations.   |
| Windows         | ❌ Not Supported     | ✅ Available  | Operates only in fallback mode.                                       |
| Other Unix-like | ❔ Untested          | ✅ Available  | May work with native Netmap if supported; fallback mode is available. |

## Error Handling

The library uses the `netmap_rs::Error` enum to report errors. This includes I/O errors, configuration issues, and problems during packet operations.

## Fallback Mode

If the `sys` feature is not enabled or if `netmap-rs` cannot initialize a native Netmap interface at runtime, it can operate in a fallback mode. This mode simulates Netmap's ring buffer behavior in software, using standard socket APIs (or in-memory queues for loopback-like examples). While performance will be significantly lower than native Netmap, it allows for development and testing on systems without Netmap or for applications where extreme performance is not the primary concern.

## Author

*   Meshack Bahati (Kenya)

## License


*   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE ).
*   MIT license ([LICENSE-MIT](LICENSE-MIT ).
---

For more details on the API, please refer to the [API documentation on docs.rs](https://docs.rs/netmap-rs).
