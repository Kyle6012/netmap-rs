//! Netmap Inter-Process Pipe Sender Example
//!
//! This program acts as the sender (potentially the master if run first)
//! for a Netmap pipe. It opens a predefined pipe name and sends a few
//! messages.
//!
//! To use this, first run `pipe_receiver_process` in another terminal, then run this.
//!
//! Usage:
//! cargo run --example pipe_sender_process --features sys
//!
//! (Ensure `pipe_receiver_process` is running or started shortly after)

use std::error::Error;
use std::thread;
use std::time::Duration;

use netmap_rs::prelude::*;

const PIPE_NAME: &str = "netmap:pipe{interproc_example_789}"; // Must match receiver
const NUM_PACKETS_IPC: usize = 3;
const PACKET_BASE_PAYLOAD_IPC: &[u8] = b"IPC Hello from Process A, msg=";

fn main() -> Result<(), Box<dyn Error>> {
    println!("[Sender Process] Starting.");
    println!("[Sender Process] Attempting to open pipe: {}", PIPE_NAME);

    let pipe_ep = NetmapBuilder::new(PIPE_NAME)
        .num_tx_rings(1) // Pipes default to 1 TX, 1 RX.
        .num_rx_rings(1)
        .build()
        .map_err(|e| format!("[Sender Process] Failed to open pipe endpoint: {:?}. Is receiver running?", e))?;

    println!("[Sender Process] Pipe endpoint opened. TX rings: {}, RX rings: {}", pipe_ep.num_tx_rings(), pipe_ep.num_rx_rings());

    if pipe_ep.num_tx_rings() == 0 {
        eprintln!("[Sender Process] No TX rings available. Exiting.");
        return Ok(());
    }
    let mut tx_ring = pipe_ep.tx_ring(0)?;

    println!("[Sender Process] Will send {} packets.", NUM_PACKETS_IPC);
    for i in 0..NUM_PACKETS_IPC {
        let mut payload = PACKET_BASE_PAYLOAD_IPC.to_vec();
        payload.extend_from_slice(i.to_string().as_bytes());

        print!("[Sender Process] Sending packet {} ({} bytes): {:?}...", i, payload.len(), std::str::from_utf8(&payload).unwrap_or("non-utf8"));
        match tx_ring.send(&payload) {
            Ok(_) => {
                tx_ring.sync();
                println!(" Sent.");
            }
            Err(e) => {
                eprintln!("[Sender Process] Failed to send packet {}: {:?}", i, e);
                return Err(Box::new(e));
            }
        }
        thread::sleep(Duration::from_secs(1)); // Give receiver time to pick up
    }

    println!("[Sender Process] All packets sent. Exiting.");
    Ok(())
}
