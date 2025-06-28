//! Netmap Inter-Process Pipe Receiver Example
//!
//! This program acts as the receiver (potentially the slave if run second,
//! or master if run first) for a Netmap pipe. It opens a predefined pipe
//! name and listens for messages.
//!
//! To use this, run this program first in one terminal, then run
//! `pipe_sender_process` in another terminal.
//!
//! Usage:
//! cargo run --example pipe_receiver_process --features sys
//!
//! (Then run `pipe_sender_process` in another terminal)

use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use netmap_rs::prelude::*;

const PIPE_NAME: &str = "netmap:pipe{interproc_example_789}"; // Must match sender
const NUM_EXPECTED_PACKETS_IPC: usize = 3;

fn main() -> Result<(), Box<dyn Error>> {
    println!("[Receiver Process] Starting.");
    println!("[Receiver Process] Attempting to open pipe: {}", PIPE_NAME);

    let pipe_ep = NetmapBuilder::new(PIPE_NAME)
        .num_tx_rings(1) // Pipes default to 1 TX, 1 RX.
        .num_rx_rings(1)
        .build()
        .map_err(|e| format!("[Receiver Process] Failed to open pipe endpoint: {:?}", e))?;

    println!("[Receiver Process] Pipe endpoint opened. RX rings: {}, TX rings: {}", pipe_ep.num_rx_rings(), pipe_ep.num_tx_rings());

    if pipe_ep.num_rx_rings() == 0 {
        eprintln!("[Receiver Process] No RX rings available. Exiting.");
        return Ok(());
    }
    let mut rx_ring = pipe_ep.rx_ring(0)?;

    let running = Arc::new(AtomicBool::new(true));
    let r_clone = running.clone();
    ctrlc::set_handler(move || {
        println!("\n[Receiver Process] Ctrl-C received, stopping...");
        r_clone.store(false, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    println!("[Receiver Process] Listening for packets... Press Ctrl-C to stop.");

    let mut packets_received_count = 0;
    while running.load(Ordering::Relaxed) && packets_received_count < NUM_EXPECTED_PACKETS_IPC {
        rx_ring.sync();
        let mut received_in_batch = 0;
        while let Some(frame) = rx_ring.recv() {
            if frame.is_empty() {
                continue;
            }
            received_in_batch +=1;
            println!(
                "[Receiver Process] Received packet {} ({} bytes): {:?}",
                packets_received_count, frame.len(), std::str::from_utf8(frame.payload()).unwrap_or("non-utf8")
            );
            packets_received_count += 1;
            if packets_received_count == NUM_EXPECTED_PACKETS_IPC {
                break;
            }
        }
        if !running.load(Ordering::Relaxed) { break; }

        if received_in_batch == 0 {
            thread::sleep(Duration::from_millis(100)); // Wait if no packets
        }
    }

    if packets_received_count >= NUM_EXPECTED_PACKETS_IPC {
        println!("[Receiver Process] Received expected {} packets.", NUM_EXPECTED_PACKETS_IPC);
    } else {
        println!("[Receiver Process] Stopped. Received {} out of {} expected packets.", packets_received_count, NUM_EXPECTED_PACKETS_IPC);
    }

    Ok(())
}
