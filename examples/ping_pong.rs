//! A simple ping-pong example demonstrating basic Netmap usage

use netmap_rs::prelude::*;
use std::time::{Duration, Instant};

fn main() -> Result<(), Error> {
    let nm = NetmapBuilder::new("netmap:eth0")
        .num_tx_rings(1)
        .num_rx_rings(1)
        .open()?;

    let mut tx_ring = nm.tx_ring(0)?;
    let mut rx_ring = nm.rx_ring(0)?;

    let ping_data = b"PING";
    let pong_data = b"PONG";

    // Ping
    tx_ring.send(ping_data)?;
    tx_ring.sync();

    let start = Instant::now();
    let timeout = Duration::from_secs(1);

    // Wait for pong
    loop {
        if let Some(frame) = rx_ring.recv() {
            if frame.payload() == pong_data {
                let rtt = start.elapsed();
                println!("Ping-pong round trip: {:?}", rtt);
                break;
            }
        }

        if start.elapsed() > timeout {
            return Err(Error::WouldBlock);
        }
    }

    Ok(())
}
