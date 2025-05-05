//! Forward Error Correction (FEC) example using Reed-Solomon

use netmap_rs::prelude::*;
use reed_solomon_erasure::galois_8::ReedSolomon;
use std::time::Duration;

const DATA_SHARDS: usize = 4;
const PARITY_SHARDS: usize = 2;

fn main() -> Result<(), Error> {
    let nm = NetmapBuilder::new("netmap:eth0")
        .num_tx_rings(1)
        .num_rx_rings(1)
        .open()?;

    let mut tx_ring = nm.tx_ring(0)?;
    let mut rx_ring = nm.rx_ring(0)?;

    // Create encoder/decoder
    let rs = ReedSolomon::new(DATA_SHARDS, PARITY_SHARDS)?;

    // Original data
    let mut data: Vec<Vec<u8>> = (0..DATA_SHARDS).map(|i| vec![i as u8; 128]).collect();

    // Add parity shards
    let mut shards = data.clone();
    shards.resize(DATA_SHARDS + PARITY_SHARDS, vec![0; 128]);
    rs.encode(&mut shards)?;

    // Send all shards
    for shard in &shards {
        tx_ring.send(shard)?;
    }
    tx_ring.sync();

    // Simulate packet loss (drop 2 random shards)
    let mut received_shards = shards.clone();
    received_shards[1] = vec![0; 128]; // Mark as missing
    received_shards[4] = vec![0; 128]; // Mark as missing

    // Receive and reconstruct
    let mut reconstructed = received_shards.clone();
    rs.reconstruct(&mut reconstructed)?;

    // Verify reconstruction
    for i in 0..DATA_SHARDS {
        assert_eq!(
            reconstructed[i], data[i],
            "Reconstruction failed for shard {}",
            i
        );
    }

    println!("FEC test successful - data reconstructed correctly");
    Ok(())
}
