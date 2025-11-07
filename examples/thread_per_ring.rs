//! Thread-per-ring with core pinning example

use core_affinity::CoreId;
use netmap_rs::prelude::*;
#[cfg(feature = "sys")]
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Error> {
    #[cfg(feature = "sys")]
    let nm_sys = {
        let nm = NetmapBuilder::new("netmap:eth0")
            .num_tx_rings(4)
            .num_rx_rings(4)
            .open()?;
        Arc::new(nm)
    };

    let core_ids = core_affinity::get_core_ids().unwrap_or_else(|| {
        eprintln!("Warning: Could not get core IDs. Thread pinning will not occur.");
        Vec::new()
    });

    let num_sim_threads = 4; // For fallback mode, simulate this many threads

    #[cfg(feature = "sys")]
    let num_rx_rings_to_spawn = nm_sys.num_rx_rings();
    #[cfg(not(feature = "sys"))]
    let num_rx_rings_to_spawn = num_sim_threads;

    // Spawn one thread per RX ring (or simulated)
    for i in 0..num_rx_rings_to_spawn {
        #[cfg(feature = "sys")]
        let nm_clone_sys = nm_sys.clone();

        let core_id_to_pin = if !core_ids.is_empty() {
            Some(core_ids[i % core_ids.len()])
        } else {
            None
        };

        thread::spawn(move || {
            if let Some(core_id) = core_id_to_pin {
                if core_affinity::set_for_current(core_id) {
                    println!("RX thread {} nominally pinned to core {:?}", i, core_id);
                } else {
                    eprintln!("RX thread {}: Failed to pin to core {:?}", i, core_id);
                }
            } else {
                 println!("RX thread {} not pinned (no core_ids available or pinning failed).", i);
            }

            #[cfg(feature = "sys")]
            {
                let mut rx_ring = nm_clone_sys.rx_ring(i).unwrap();
                println!("RX thread {} (sys) started on core {:?}", i, core_id_to_pin.map(|c| c.id));

                let mut counter = 0;
                let start = std::time::Instant::now();

                loop {
                    if let Some(_frame) = rx_ring.recv() {
                        counter += 1;

                        if counter % 1000 == 0 {
                            let elapsed = start.elapsed().as_secs_f64();
                            println!("RX {} (sys): {:.2} pkt/sec", i, counter as f64 / elapsed);
                        }
                    }
                }
            }
            #[cfg(not(feature = "sys"))]
            {
                println!("RX thread {} (fallback) started.", i);
                // Simulate some work or just idle
                loop {
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });
    }

    #[cfg(feature = "sys")]
    let num_tx_rings_to_spawn = nm_sys.num_tx_rings();
    #[cfg(not(feature = "sys"))]
    let num_tx_rings_to_spawn = num_sim_threads;

    // Spawn one thread per TX ring (or simulated)
    for i in 0..num_tx_rings_to_spawn {
        #[cfg(feature = "sys")]
        let nm_clone_sys = nm_sys.clone();

        let core_id_to_pin = if !core_ids.is_empty() {
            Some(core_ids[i % core_ids.len()])
        } else {
            None
        };

        thread::spawn(move || {
            if let Some(core_id) = core_id_to_pin {
                 if core_affinity::set_for_current(core_id) {
                    println!("TX thread {} nominally pinned to core {:?}", i, core_id);
                } else {
                    eprintln!("TX thread {}: Failed to pin to core {:?}", i, core_id);
                }
            } else {
                println!("TX thread {} not pinned (no core_ids available or pinning failed).", i);
            }

            #[cfg(feature = "sys")]
            {
                let mut tx_ring = nm_clone_sys.tx_ring(i).unwrap();
                println!("TX thread {} (sys) started on core {:?}", i, core_id_to_pin.map(|c| c.id));

                let payload = vec![0u8; 64];
                let mut counter = 0;
                let start = std::time::Instant::now();

                loop {
                    tx_ring.send(&payload).unwrap();
                    tx_ring.sync();
                    counter += 1;

                    if counter % 1000 == 0 {
                        let elapsed = start.elapsed().as_secs_f64();
                        println!("TX {} (sys): {:.2} pkt/sec", i, counter as f64 / elapsed);
                    }
                    thread::sleep(Duration::from_micros(10));
                }
            }
            #[cfg(not(feature = "sys"))]
            {
                println!("TX thread {} (fallback) started.", i);
                // Simulate some work or just idle
                loop {
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });
    }

    // Keep main thread alive
    println!("Main thread running. System threads (if any) are processing packets.");
    println!("Fallback threads (if any) are simulating activity.");
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
