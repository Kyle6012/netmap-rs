//! Netmap Host Stack Receive Example
//!
//! This example demonstrates how to receive packets from the host stack
//! associated with a physical network interface using Netmap.
//!
//! It opens the specified interface with the `^` suffix (e.g., "netmap:eth0^")
//! to access its host stack rings. It then listens on the first host RX ring
//! and prints information about received packets.
//!
//! Usage:
//! cargo run --example host_receive --features sys -- <interface_name_with_caret>
//!
//! Example:
//! cargo run --example host_receive --features sys -- netmap:eth0^
//!
//! (Replace `eth0` with your actual network interface that you want to monitor)
//!
//! While this example is running, generate some traffic on the host that would
//! normally go through `eth0`. For example:
//! - `ping localhost` (if eth0 is part of routing for localhost, less likely for physical)
//! - `ping <some_ip_on_eth0_network>`
//! - Or any application generating network traffic through the specified interface.

use std::env;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use netmap_rs::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 || !args[1].contains('^') {
        eprintln!(
            "Usage: {} <interface_name_with_caret_suffix>",
            args[0]
        );
        eprintln!("Example: {} netmap:eth0^", args[0]);
        return Err("Invalid arguments: Interface name must include '^' suffix.".into());
    }

    let if_name = &args[1]; // e.g., "netmap:eth0^"

    println!(
        "Attempting to open host stack rings for interface: {}",
        if_name
    );

    // Build the Netmap interface configuration for host stack rings.
    // The `^` in the interface name signals to NetmapBuilder to request host rings.
    let nm_desc = NetmapBuilder::new(if_name)
        .num_rx_rings(1) // Request 1 host RX ring (or 0 for all available host RX rings)
        .num_tx_rings(0) // Not using TX in this example, 0 for default/all.
        .build()?;

    println!(
        "Successfully opened interface {}. Number of host RX rings available: {}",
        if_name,
        nm_desc.num_rx_rings()
    );

    if nm_desc.num_rx_rings() == 0 {
        eprintln!("No host RX rings available for interface {}. Exiting.", if_name);
        return Ok(());
    }

    // Get the first host RX ring
    let mut rx_ring = nm_desc.rx_ring(0)?;
    println!("Listening for packets on host RX ring 0...");

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\nCtrl-C received, stopping packet reception...");
        r.store(false, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    let mut packets_received = 0u64;
    while running.load(Ordering::Relaxed) {
        rx_ring.sync(); // Make new packets visible

        let mut batch_count = 0;
        while let Some(frame) = rx_ring.recv() {
            if frame.is_empty() {
                continue;
            }
            packets_received += 1;
            batch_count +=1;
            println!(
                "Received packet #{} on host ring: len = {} bytes. Payload (first 32 bytes): {:?}",
                packets_received,
                frame.len(),
                &frame.payload()[..std::cmp::min(frame.len(), 32)]
            );
        }

        if batch_count == 0 && running.load(Ordering::Relaxed) {
            // Sleep briefly if no packets to avoid busy-waiting
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    println!("\nFinished. Total packets received from host stack: {}", packets_received);
    Ok(())
}
