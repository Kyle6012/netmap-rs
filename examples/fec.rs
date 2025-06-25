#![cfg(feature = "sys")]

use netmap_rs::prelude::*;
use reed_solomon_erasure::galois_8::ReedSolomon;
use std::time::Duration;

// Example: 2 data shards, 1 parity shard
const DATA_SHARDS: usize = 2;
const PARITY_SHARDS: usize = 1;
const TOTAL_SHARDS: usize = DATA_SHARDS + PARITY_SHARDS;

fn main() -> Result<(), Error> {
    let nm = NetmapBuilder::new("netmap:eth0") // Replace with your interface
        .num_tx_rings(1)
        .num_rx_rings(1)
        .build()?;

    let mut tx_ring = nm.tx_ring(0)?;
    let mut rx_ring = nm.rx_ring(0)?;

    let r = ReedSolomon::new(DATA_SHARDS, PARITY_SHARDS).unwrap();

    // Original data
    let original_data = b"Hello Netmap with FEC!".to_vec();
    let chunk_size = (original_data.len() + DATA_SHARDS - 1) / DATA_SHARDS;
    let mut shards = Vec::with_capacity(TOTAL_SHARDS);

    for i in 0..DATA_SHARDS {
        let start = i * chunk_size;
        let end = std::cmp::min(start + chunk_size, original_data.len());
        let mut shard = original_data[start..end].to_vec();
        shard.resize(chunk_size, 0); // Pad if necessary
        shards.push(shard);
    }
    for _ in 0..PARITY_SHARDS {
        shards.push(vec![0u8; chunk_size]);
    }

    // Encode
    r.encode(&mut shards).unwrap();

    // Simulate sending shards (e.g., as separate packets)
    println!("Sending shards...");
    for (i, shard) in shards.iter().enumerate() {
        // In a real scenario, prepend shard index or other metadata
        let mut packet_data = vec![i as u8]; // Shard index
        packet_data.extend_from_slice(shard);
        tx_ring.send(&packet_data)?;
        println!("Sent shard {}: len {}", i, shard.len());
    }
    tx_ring.sync();

    // Simulate receiving shards (and potentially losing one)
    let mut received_shards: Vec<Option<Vec<u8>>> = vec![None; TOTAL_SHARDS];
    let mut received_count = 0;

    println!("Receiving shards (simulating loss of shard 0)...");
    for _ in 0..10 { // Try to receive for a bit
        rx_ring.sync();
        while let Some(frame) = rx_ring.recv() {
            let payload = frame.payload();
            if payload.is_empty() { continue; }
            let shard_index = payload[0] as usize;

            // SIMULATE LOSS OF SHARD 0
            if shard_index == 0 && received_shards[0].is_none() && received_count < DATA_SHARDS {
                 println!("Simulated loss of shard 0");
                 received_shards[0] = Some(vec![]); // Mark as lost for reconstruction logic
                 // but don't actually store it / increment received_count for it yet
                 // to ensure reconstruction is attempted.
                 // For this test, we'll actually skip storing it to force reconstruction.
                 continue;
            }

            if shard_index < TOTAL_SHARDS && received_shards[shard_index].is_none() {
                received_shards[shard_index] = Some(payload[1..].to_vec());
                received_count += 1;
                println!("Received shard {}", shard_index);
            }
            if received_count >= DATA_SHARDS { break; }
        }
        if received_count >= DATA_SHARDS { break; }
        std::thread::sleep(Duration::from_millis(50));
    }


    if received_count < DATA_SHARDS {
        eprintln!("Did not receive enough shards to reconstruct.");
        return Ok(());
    }

    println!("Attempting reconstruction...");
    match r.reconstruct(&mut received_shards) {
        Ok(_) => {
            println!("Reconstruction successful!");
            let mut reconstructed_data = Vec::new();
            for i in 0..DATA_SHARDS {
                if let Some(shard_data) = &received_shards[i] {
                    reconstructed_data.extend_from_slice(shard_data);
                } else {
                    eprintln!("Missing data shard {} after reconstruction attempt.", i);
                    return Ok(());
                }
            }
            // Trim padding if original length known, or handle as per application logic
            reconstructed_data.truncate(original_data.len());

            if reconstructed_data == original_data {
                println!("Data successfully reconstructed: {:?}", String::from_utf8_lossy(&reconstructed_data));
            } else {
                eprintln!("Data mismatch after reconstruction!");
                eprintln!("Original:       {:?}", String::from_utf8_lossy(&original_data));
                eprintln!("Reconstructed:  {:?}", String::from_utf8_lossy(&reconstructed_data));
            }
        }
        Err(e) => {
            eprintln!("Reconstruction failed: {:?}", e);
        }
    }

    Ok(())
}
