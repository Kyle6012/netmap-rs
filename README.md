# netmap-rs

`netmap-rs` provides safe, zero-cost abstractions for [Netmap](http://info.iet.unipi.it/~luigi/netmap/) kernel-bypass networking in Rust. It aims to offer high-performance packet I/O by leveraging Netmap's efficient memory-mapped ring buffers.

## Features

*   **Zero-copy packet I/O:** Directly access packet buffers in memory shared with the kernel.
*   **High Performance:** Designed for low-latency and high-throughput applications.
*   **Safe Abstractions:** Provides a safe Rust API over the underlying `netmap` C structures.
*   **Feature Flags:** Customizable build via feature flags (e.g., `sys` for core Netmap functionality, `tokio-async` for Tokio integration).

## Adding netmap-rs to your project

To use `netmap-rs` in your project, add it to your `Cargo.toml`.

**Crucially, for most use cases, you will need to enable the `sys` feature.** This feature compiles and links against the necessary `netmap` C libraries and enables the core structures like `NetmapBuilder`, `Netmap`, `TxRing`, and `RxRing`.

```toml
[dependencies]
netmap-rs = { version = "0.1.0", features = ["sys"] }
# Replace "0.1.0" with the desired version from crates.io
```

If you intend to use `netmap-rs` with Tokio for asynchronous operations, you should also enable the `tokio-async` feature:

```toml
[dependencies]
netmap-rs = { version = "0.1.0", features = ["sys", "tokio-async"] }
```

## Basic Usage Example

Here's a basic example of how to open a Netmap interface, send, and receive a packet. This example assumes you have a loopback interface or a setup where packets sent on an interface can be received on it.

```rust
use netmap_rs::NetmapBuilder; // Or use netmap_rs::prelude::*;
use netmap_rs::Error; // If not using prelude
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Error> {
    // Ensure you have enabled the "sys" feature for netmap-rs in your Cargo.toml
    // e.g., netmap-rs = { version = "...", features = ["sys"] }

    // Attempt to open a netmap interface.
    // Replace "eth0" with your desired interface.
    // NetmapBuilder will prefix with "netmap:" if needed.
    // Use "eth0^" to access host stack rings.
    let nm = NetmapBuilder::new("eth0") // Or "netmap:eth0"
        .num_tx_rings(1) // Configure one transmission ring
        .num_rx_rings(1) // Configure one reception ring
        .build()?; // This can fail if Netmap is not available or the interface doesn't exist.

    println!(
        "Opened netmap interface: {} TX rings, {} RX rings, Host interface: {}",
        nm.num_tx_rings(),
        nm.num_rx_rings(),
        nm.is_host_if()
    );

    // Get handles to the first transmission and reception rings.
    let mut tx_ring = nm.tx_ring(0)?;
    let mut rx_ring = nm.rx_ring(0)?;

    // Prepare a packet to send.
    let packet_data = b"hello netmap-rs!";

    // Send the packet.
    // The `send` method stages the packet in the ring's buffer.
    if tx_ring.send(packet_data).is_err() {
        eprintln!("Failed to stage packet for sending. Ring full or packet too large?");
        // Potentially call tx_ring.sync() here if you expect the ring to be full
        // and want to force transmission before trying to send again.
    }

    // `sync` makes queued packets available to the hardware for transmission.
    // It also updates the kernel's view of consumed buffers on the RX side.
    tx_ring.sync();
    println!("Sent packet: {:?}", packet_data);

    // Attempt to receive the packet.
    let mut received_packet = false;
    println!("Attempting to receive packet...");
    for _ in 0..10 { // Try a few times with a delay
        // `sync` on the rx_ring tells the kernel we are done with previously received packets
        // and updates the ring's state to make new packets visible.
        rx_ring.sync();

        while let Some(frame) = rx_ring.recv() {
            println!("Received packet: {:?}", frame.payload());
            // Simple loopback check
            if frame.payload() == packet_data {
                println!("Successfully received the sent packet!");
                received_packet = true;
            }
            // Release the received frame buffer back to Netmap
            // In this simplified recv() loop, the frame is dropped at the end of scope,
            // which handles buffer release if Frame implements Drop appropriately.
            // If Frame doesn't, manual management would be needed.
            // (Assuming Frame's Drop implementation correctly returns the buffer to the ring)
            if received_packet { break; }
        }
        if received_packet {
            break;
        }
        sleep(Duration::from_millis(200)); // Wait a bit for the packet to arrive/loopback
    }

    if !received_packet {
        eprintln!("Failed to receive the packet back. Ensure your interface is configured for loopback or testing appropriately.");
    }

    Ok(())
}
```

**Note on Netmap Setup:**
Using `netmap-rs` (with the `sys` feature) requires that your system has the Netmap kernel module installed, along with its userspace C libraries and development headers. You will also need `clang` installed, as it's used by the build process to generate bindings.

Please refer to the [official Netmap project page](http://info.iet.unipi.it/~luigi/netmap/) or the [Netmap GitHub repository](https://github.com/netmap/netmap) for instructions on how to compile and install Netmap on your operating system. You may also need appropriate permissions to access network interfaces via Netmap.

**Troubleshooting Build Issues (Netmap C Library Not Found):**

If the build fails with errors indicating that `net/netmap_user.h` or the Netmap library cannot be found, it means the build script for the `netmap-min-sys` dependency could not locate your Netmap C installation.

*   **Standard Paths:** The build script checks standard locations like `/usr/local`.
*   **Custom Installation Path:** If you have installed Netmap in a custom directory (e.g., `/opt/netmap`), you need to inform the build system by setting the `NETMAP_LOCATION` environment variable before running `cargo build`:
    ```bash
    NETMAP_LOCATION=/opt/netmap cargo build
    ```
    Replace `/opt/netmap` with the root directory of your Netmap installation (this directory should contain `include` and `lib` subdirectories for Netmap).
*   **Further Details:** For more information on build-time environment variables like `NETMAP_LOCATION` and `DISABLE_NETMAP_KERNEL` (for compiling without Netmap), refer to the `README.md` file within the `netmap-min-sys` crate.

## Building Examples

The crate includes several examples in the `examples/` directory. To run an example, ensure you have the `sys` feature enabled for the crate when building:

```bash
cargo run --example <example_name> --features netmap-rs/sys
```
(If `netmap-rs` is the current crate, you might not need the `netmap-rs/` prefix for features).

For example, to run `ping_pong`:
```bash
cargo run --example ping_pong --features sys
# Or if it's a dependency:
# cargo run --example ping_pong --features netmap-rs/sys
```
Make sure to adapt the interface names used within the examples to your specific setup.

## License

This crate is licensed under
*   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE)).
*   MIT license ([LICENSE-MIT](LICENSE-MIT)).
