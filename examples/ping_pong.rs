#![cfg(feature = "sys")]

use netmap_rs::prelude::*;
use std::time::Duration;

fn main() -> Result<(), Error> {
    let nm = NetmapBuilder::new("netmap:eth0")
        .nm_tx_rings(1)
        .num_rx_rings(1)
        .build()?;

    let mut tx_ring = nm.tx_ring(0)?;
    let mut rx_ring = nm.rx_ring(0)?;

    // send a packet
    tx_ring.send(b"hello world")?;
    tx_ring.sync();

    // receive packets
    let mut received = false;
    for _ in 0..10 {
        // try a few times
        while let Some(frame) = rx_ring.recv() {
            println!("Received packet: {:?}", frame.payload());
            assert_eq!(frame.payload(), b"hello world");
            received = true;
            break;
        }
        if received {
            break;
        }
        std::thread::sleep(Duration::from_millis(100)); // wait for packets
        rx_ring.sync(); // tell kernel we are done with previous packets
    }

    if !received {
        return Err(Error::WouldBlock); // or some other error
    }

    Ok(())
}
