# netmap-rs

Safe, zero-cost abstractions for the Netmap kernel-bypass networking API.

## Features

- Zero-copy packet I/O
- Thread-per-ring with core pinning
- Batch operations for high throughput
- Cross-platform support (with fallback implementations)
- Safe Rust abstractions over raw FFI

## Requirements

- Linux with Netmap support (or use fallback mode)
- Rust 1.60+

## Usage

Add to your  `Cargo.toml`:

```toml

[dependecies]
netmap-rs ="0.1"
```

## Basic Example

```rust
use netmap_rs::prelude::*;

fn main() -> Result<(), Error> {
    let nm = NetmapBuilder::new("netmap:eth0")
        .nm_tx_rings(1)
        .num_rx_rings(1)
        .open()?;

    let mut tx_ring = nm.tx_ring(0)?;
    let mut rx-ring = nm.rx_ring(0)?;

    // send a packet
    tx_ring.send(b"hello world")?;
    tx.ring.sync();

    // receive packets
    while let Some(frame) = rx_ring.recv(){
        println!("Received packet: {:?}", frame.payload());
    }
    Ok(())
}
```

## Platform Support

|Platform | Status | Notes                                 |
|---------|--------|---------------------------------------|
| Linux   |   ✅   | Requires Netmap kernel module         |
| macOS   |   ⚠️   | Limited support, may require fallback |
| Windows |   ⚠️   | Fallback mode only                    |


## License

Licensed under Apache 2.0


