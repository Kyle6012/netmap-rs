# netmap-rs

`netmap-rs` provides safe, zero-cost abstractions for [Netmap](http://info.iet.unipi.it/~luigi/netmap/) kernel-bypass networking in Rust. It aims to offer high-performance packet I/O by leveraging Netmap's efficient memory-mapped ring buffers.

## Features

*   **Zero-copy packet I/O:** Directly access packet buffers in memory shared with the kernel.
*   **High Performance:** Designed for low-latency and high-throughput applications.
*   **Safe Abstractions:** Provides a safe Rust API over the underlying `netmap` C structures.
*   **Feature Flags:** Customizable build via feature flags (e.g., `sys` for core Netmap functionality, `tokio-async` for Tokio integration).

## Prerequisites

### System Requirements

**IMPORTANT:** This crate requires the Netmap C library to be installed on your system. Without it, the `sys` feature will not work.

### Installing Netmap C Library

#### On Linux

1. **Install build dependencies:**
   ```bash
   # Ubuntu/Debian
   sudo apt-get update
   sudo apt-get install build-essential git linux-headers-$(uname -r)
   
   # CentOS/RHEL
   sudo yum install gcc git kernel-devel-$(uname -r)
   ```

2. **Download and build netmap:**
   ```bash
   git clone https://github.com/luigirizzo/netmap.git
   cd netmap/LINUX
   ./configure
   make
   sudo make install
   ```

3. **Load the kernel module:**
   ```bash
   sudo insmod netmap.ko
   # Verify it's loaded
   ls /dev/netmap
   ```

#### On FreeBSD

Netmap is included by default in FreeBSD 11+. No additional installation is required.

#### Custom Installation Paths

If you installed netmap in a non-standard location, set the `NETMAP_LOCATION` environment variable:

```bash
export NETMAP_LOCATION=/opt/netmap
# Then build your project
```

## Adding netmap-rs to your project

To use `netmap-rs` in your project, add it to your `Cargo.toml`.

**Crucially, for most use cases, you will need to enable the `sys` feature.** This feature compiles and links against the necessary `netmap` C libraries and enables the core structures like `NetmapBuilder`, `Netmap`, `TxRing`, and `RxRing`.

```toml
[dependencies]
netmap-rs = { version = "0.3", features = ["sys"] }
```

If you intend to use `netmap-rs` with Tokio for asynchronous operations, you should also enable the `tokio-async` feature:

```toml
[dependencies]
netmap-rs = { version = "0.3", features = ["sys", "tokio-async"] }
```

## Basic Usage Example

Here's a basic example of how to open a Netmap interface, send, and receive a packet. This example assumes you have a loopback interface or a setup where packets sent on an interface can be received on it.

```rust
use netmap_rs::prelude::*;
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
        .build()?;

    // Get handles to the first transmission and reception rings.
    let mut tx_ring = nm.tx_ring(0)?;
    let mut rx_ring = nm.rx_ring(0)?;

    // Prepare a packet to send.
    let packet_data = b"hello netmap!";

    // Send the packet.
    // The `send` method queues the packet.
    tx_ring.send(packet_data)?;
    // `sync` ensures that queued packets are made available to the hardware.
    tx_ring.sync();
    println!("Sent packet: {:?}", packet_data);

    // Attempt to receive the packet.
    let mut received = false;
    for _ in 0..5 { // Try a few times with a delay
        // `sync` on the rx_ring tells the kernel we are done with previously received packets
        // and updates the ring's state to see new packets.
        rx_ring.sync();
        while let Some(frame) = rx_ring.recv() {
            println!("Received packet: {:?}", frame.payload());
            assert_eq!(frame.payload(), packet_data);
            received = true;
            break;
        }
        if received {
            break;
        }
        sleep(Duration::from_millis(100)); // Wait a bit for the packet to arrive
    }

    if !received {
        eprintln!("Failed to receive the packet back.");
        // Depending on the setup (e.g. loopback interface), this might indicate an issue.
    }

    Ok(())
}
```

## Public API

This section provides a detailed overview of the public API of `netmap-rs`.

### `NetmapBuilder`

The `NetmapBuilder` is used to configure and create a `Netmap` instance.

*   **`NetmapBuilder::new(ifname_str: &str) -> Self`**

    Creates a new builder for the given Netmap interface name. `ifname_str` can be a simple interface name like `"eth0"`, or `"eth0^"` to access the host stack.

    ```rust
    use netmap_rs::NetmapBuilder;

    let builder = NetmapBuilder::new("eth0");
    ```

*   **`num_tx_rings(self, num: usize) -> Self`**

    Sets the desired number of transmission (TX) rings.

    ```rust
    use netmap_rs::NetmapBuilder;

    let builder = NetmapBuilder::new("eth0").num_tx_rings(2);
    ```

*   **`num_rx_rings(self, num: usize) -> Self`**

    Sets the desired number of reception (RX) rings.

    ```rust
    use netmap_rs::NetmapBuilder;

    let builder = NetmapBuilder::new("eth0").num_rx_rings(2);
    ```

*   **`flags(self, flags: u32) -> Self`**

    Sets additional flags for the Netmap request. See `<net/netmap_user.h>` for available flags.

*   **`build(self) -> Result<Netmap, Error>`**

    Consumes the builder and attempts to open the Netmap interface, returning a `Netmap` instance.

    ```rust
    use netmap_rs::NetmapBuilder;

    let nm = NetmapBuilder::new("eth0").build();
    ```

### `Netmap`

A `Netmap` instance represents an open Netmap interface.

*   **`num_tx_rings(&self) -> usize`**

    Returns the number of configured TX rings.

*   **`num_rx_rings(&self) -> usize`**

    Returns the number of configured RX rings.

*   **`is_host_if(&self) -> bool`**

    Returns `true` if the `Netmap` instance is configured for host stack rings.

*   **`tx_ring(&self, index: usize) -> Result<TxRing, Error>`**

    Returns a handle to a specific TX ring.

*   **`rx_ring(&self, index: usize) -> Result<RxRing, Error>`**

    Returns a handle to a specific RX ring.

### `Ring`

Represents a generic Netmap ring.

*   **`index(&self) -> usize`**

    Returns the index of the ring.

*   **`num_slots(&self) -> usize`**

    Returns the total number of slots in the ring.

*   **`sync(&self)`**

    Synchronizes the ring with the NIC, making sent packets available to the hardware and updating the ring's state to see new packets.

### `TxRing`

A handle to a transmission (TX) ring.

*   **`send(&mut self, buf: &[u8]) -> Result<(), Error>`**

    Sends a single packet. The data in `buf` is copied to a slot in the ring.

*   **`max_payload_size(&self) -> usize`**

    Returns the maximum payload size for a single packet in this ring.

*   **`reserve_batch(&mut self, count: usize) -> Result<BatchReservation, Error>`**

    Reserves space for sending a batch of packets. Returns a `BatchReservation` instance.

### `BatchReservation`

A reservation for a batch of packets to be sent.

*   **`packet(&mut self, index: usize, len: usize) -> Result<&mut [u8], Error>`**

    Gets a mutable slice for a packet in the batch. You can write your packet data to this slice.

*   **`commit(self)`**

    Commits the batch, making the packets visible to the NIC.

### `RxRing`

A handle to a reception (RX) ring.

*   **`recv(&mut self) -> Option<Frame>`**

    Receives a single packet from the ring. Returns a `Frame` if a packet is available.

*   **`recv_batch(&mut self, batch: &mut [Frame]) -> usize`**

    Receives a batch of packets. The `batch` slice is filled with available frames, and the number of received frames is returned.

### `Frame`

A `Frame` represents a received packet. It can be either a zero-copy view of a packet buffer (from a `Netmap` ring) or an owned buffer (in fallback mode).

*   **`new(data: &'a [u8]) -> Self`**: Creates a new frame from a borrowed byte slice (zero-copy).
*   **`new_owned(data: Vec<u8>) -> Self`**: Creates a new frame from an owned vector of bytes (for fallback).
*   **`len(&self) -> usize`**: Returns the length of the frame.
*   **`is_empty(&self) -> bool`**: Returns `true` if the frame is empty.
*   **`payload(&self) -> &[u8]`**: Returns a slice containing the packet's payload.

    ```rust
    if let Some(frame) = rx_ring.recv() {
        println!("Received packet of length {}: {:?}", frame.len(), frame.payload());
    }
    ```

### Async API (`tokio-async` feature)

When the `tokio-async` feature is enabled, you can use the following async wrappers for non-blocking I/O with Tokio.

#### `TokioNetmap`

The `TokioNetmap` is the entry point for async operations.

*   **`TokioNetmap::new(netmap: Netmap) -> io::Result<Self>`**

    Creates a new `TokioNetmap` by wrapping a `Netmap` instance.

    ```rust
    use netmap_rs::NetmapBuilder;
    use netmap_rs::tokio_async::TokioNetmap;

    # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let nm = NetmapBuilder::new("eth0").build()?;
    let tokio_nm = TokioNetmap::new(nm)?;
    # Ok(())
    # }
    ```

*   **`rx_ring(&self, ring_idx: usize) -> Result<AsyncNetmapRxRing, Error>`**

    Returns an async wrapper for a specific RX ring.

*   **`tx_ring(&self, ring_idx: usize) -> Result<AsyncNetmapTxRing, Error>`**

    Returns an async wrapper for a specific TX ring.

#### `AsyncNetmapRxRing`

An `AsyncRead` implementation for a Netmap RX ring.

*   You can use the methods from `tokio::io::AsyncReadExt` to read from the ring, for example `read()`.

    ```rust
    # use netmap_rs::NetmapBuilder;
    # use netmap_rs::tokio_async::TokioNetmap;
    # use tokio::io::AsyncReadExt;
    # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    # let nm = NetmapBuilder::new("eth0").build()?;
    # let tokio_nm = TokioNetmap::new(nm)?;
    let mut rx_ring = tokio_nm.rx_ring(0)?;
    let mut buf = [0; 1500];
    let n = rx_ring.read(&mut buf).await?;
    # Ok(())
    # }
    ```

#### `AsyncNetmapTxRing`

An `AsyncWrite` implementation for a Netmap TX ring.

*   You can use the methods from `tokio::io::AsyncWriteExt` to write to the ring, for example `write_all()` and `flush()`.

    ```rust
    # use netmap_rs::NetmapBuilder;
    # use netmap_rs::tokio_async::TokioNetmap;
    # use tokio::io::AsyncWriteExt;
    # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    # let nm = NetmapBuilder::new("eth0").build()?;
    # let tokio_nm = TokioNetmap::new(nm)?;
    let mut tx_ring = tokio_nm.tx_ring(0)?;
    tx_ring.write_all(b"hello async netmap").await?;
    tx_ring.flush().await?;
    # Ok(())
    # }
    ```

### `Error` Enum

The `Error` enum represents all possible errors that can occur in `netmap-rs`.

*   `Io(io::Error)`: An I/O error from the underlying system.
*   `WouldBlock`: The operation would block.
*   `BindFail(String)`: Failed to bind to a Netmap interface.
*   `InvalidRingIndex(usize)`: The specified ring index is out of bounds.
*   `PacketTooLarge(usize)`: The packet is too large for the ring buffer.
*   `InsufficientSpace`: There is not enough space in the ring buffer.
*   `UnsupportedPlatform(String)`: The platform is not supported.
*   `FallbackUnsupported(String)`: The feature is not supported in fallback mode.

### Fallback API

For platforms without Netmap support, a fallback implementation is provided.

*   **`create_fallback_channel(max_size: usize) -> (FallbackTxRing, FallbackRxRing)`**

    Creates a connected pair of fallback TX and RX rings that simulate a Netmap pipe.

    ```rust
    use netmap_rs::fallback::create_fallback_channel;

    let (tx, rx) = create_fallback_channel(64);
    tx.send(b"hello fallback").unwrap();
    if let Some(frame) = rx.recv() {
        assert_eq!(frame.payload(), b"hello fallback");
    }
    ```

## Troubleshooting

### Build Errors

#### "netmap_user.h not found"

This means the Netmap C library is not installed or not found. Make sure to:

1. Install the Netmap C library (see Prerequisites section)
2. Set `NETMAP_LOCATION` if installed in a non-standard path

#### "undefined reference to `nm_open`"

This indicates the Netmap library is not being linked properly. Ensure:

1. The `sys` feature is enabled in Cargo.toml
2. Netmap is properly installed with the library files

#### Feature Flag Issues

If you get errors like "NetmapBuilder not found", make sure you have enabled the `sys` feature:

```toml
[dependencies]
netmap-rs = { version = "0.3", features = ["sys"] }
```

### Runtime Errors

#### "Failed to open interface"

Common causes:

1. **Permission issues**: You need root/sudo access to use netmap
   ```bash
   sudo ./your_program
   ```

2. **Interface doesn't exist**: Check available interfaces with:
   ```bash
   ip link show
   ```

3. **Netmap kernel module not loaded**:
   ```bash
   sudo insmod netmap.ko
   ```

4. **Driver not supported**: Not all network drivers support netmap. Check supported drivers:
   ```bash
   cd netmap/LINUX
   ./configure --show-drivers
   ```

#### "Operation would block"

This is normal behavior when the ring buffer is full or empty. Implement proper retry logic in your application.

## Advanced Usage

### Thread-per-Ring Pattern

For maximum performance, dedicate threads to individual rings:

```rust
use netmap_rs::prelude::*;
use std::thread;

fn main() -> Result<(), Error> {
    let nm = NetmapBuilder::new("eth0")
        .num_tx_rings(4)
        .num_rx_rings(4)
        .build()?;

    let mut handles = vec![];

    // Spawn RX threads
    for i in 0..nm.num_rx_rings() {
        let rx_ring = nm.rx_ring(i)?;
        let handle = thread::spawn(move || {
            // Process packets on this ring
            // ...
        });
        handles.push(handle);
    }

    // Spawn TX threads
    for i in 0..nm.num_tx_rings() {
        let tx_ring = nm.tx_ring(i)?;
        let handle = thread::spawn(move || {
            // Send packets on this ring
            // ...
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    Ok(())
}
```

### Async Support

Enable the `tokio-async` feature for async/await support:

```rust
use netmap_rs::tokio_async::*;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nm = TokioNetmap::new("eth0").await?;
    let mut rx_ring = nm.async_rx_ring(0).await?;
    
    loop {
        if let Some(frame) = rx_ring.recv().await? {
            println!("Received: {:?}", frame.payload());
        }
        sleep(Duration::from_millis(10)).await;
    }
}
```

## Examples

The `examples/` directory contains several complete examples:

- `ping_pong.rs` - Basic send/receive example
- `sliding_window_arq.rs` - Reliable delivery with ARQ
- `fec.rs` - Forward Error Correction
- `thread_per_ring.rs` - Thread-per-ring pattern

Run examples with:

```bash
cargo run --example ping_pong --features sys
```

## Performance Tips

1. **Use batch operations** where possible to amortize system call overhead
2. **Pin threads to cores** using `core_affinity` for consistent performance
3. **Pre-allocate buffers** to avoid allocation during packet processing
4. **Use multiple rings** to leverage multi-core systems
5. **Consider NUMA topology** when pinning threads to cores

## License

This project is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## AUTHOR
- Meshack Bahati Ouma - CS major (Maseno University (Kenya))

- **Email**: bahatikylemeshack@gmail.com

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Acknowledgments

- The Netmap project for the excellent kernel-bypass networking framework
- The Rust community for the safe systems programming language
