
//! Thread-per-ring with core pinning example

use netmap_rs::prelude::*;
use core_affinity::CoreId;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Error> {
    let nm = Arc::new(NetmapBuilder::new("netmap:eth0")
        .num_tx_rings(4)
        .num_rx_rings(4)
        .open()?);

    let core_ids = core_affinity::get_core_ids().unwrap();

    // Spawn one thread per RX ring
    for i in 0..nm.num_rx_rings() {
        let nm = nm.clone();
        let core_id = core_ids[i % core_ids.len()];

        thread::spawn(move || {
            // Pin thread to core
            core_affinity::set_for_current(core_id);

            let mut rx_ring = nm.rx_ring(i).unwrap();
            println!("RX thread {} started on core {:?}", i, core_id);

            let mut counter = 0;
            let start = std::time::Instant::now();

            loop {
                if let Some(frame) = rx_ring.recv() {
                    counter += 1;
                    
                    if counter % 1000 == 0 {
                        let elapsed = start.elapsed().as_secs_f64();
                        println!("RX {}: {:.2} pkt/sec", i, counter as f64 / elapsed);
                    }
                }
            }
        });
    }

    // Spawn one thread per TX ring
    for i in 0..nm.num_tx_rings() {
        let nm = nm.clone();
        let core_id = core_ids[i % core_ids.len()];

        thread::spawn(move || {
            // Pin thread to core
            core_affinity::set_for_current(core_id);

            let mut tx_ring = nm.tx_ring(i).unwrap();
            println!("TX thread {} started on core {:?}", i, core_id);

            let payload = vec![0u8; 64];
            let mut counter = 0;
            let start = std::time::Instant::now();

            loop {
                tx_ring.send(&payload).unwrap();
                tx_ring.sync();
                counter += 1;

                if counter % 1000 == 0 {
                    let elapsed = start.elapsed().as_secs_f64();
                    println!("TX {}: {:.2} pkt/sec", i, counter as f64 / elapsed);
                }

                thread::sleep(Duration::from_micros(10));
            }
        });
    }

    // Keep main thread alive
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
