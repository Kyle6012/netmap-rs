//! Netmap Basic Polling Example
//!
//! This example demonstrates how to use `poll()` (via the `polling` crate)
//! with Netmap file descriptors to wait for I/O readiness without busy-looping.
//!
//! It sets up two Netmap pipe endpoints for intra-process communication:
//! - `pipe_a`: Acts as the sender.
//! - `pipe_b`: Acts as the receiver.
//!
//! The example shows:
//! 1. How to register a Netmap file descriptor with a `polling::Poller`.
//! 2. How to wait for `POLLIN` events on a receiver's RX ring.
//! 3. The necessity of calling `rx_ring.sync()` after a `POLLIN` event before `recv()`.
//! 4. How to wait for `POLLOUT` events on a sender's TX ring (indicating space is available).
//!
//! Usage:
//! cargo run --example poll_basic --features sys

use std::error::Error;
use std::os::unix::io::AsRawFd;
use std::time::Duration;

use netmap_rs::prelude::*;
use polling::{Event, Events, Poller};

// Use a unique pipe name for this example
const PIPE_NAME_POLL: &str = "netmap:pipe{poll456}";
const NUM_PACKETS_TO_SEND: usize = 5;

fn main() -> Result<(), Box<dyn Error>> {
    println!("Netmap Polling Example using '{}'", PIPE_NAME_POLL);

    // --- Setup Pipe Endpoints ---
    // Endpoint A will send, Endpoint B will receive.
    let pipe_a = NetmapBuilder::new(PIPE_NAME_POLL)
        .num_tx_rings(1)
        .num_rx_rings(1)
        .build()
        .expect("Failed to open pipe endpoint A");

    let pipe_b = NetmapBuilder::new(PIPE_NAME_POLL)
        .num_tx_rings(1)
        .num_rx_rings(1)
        .build()
        .expect("Failed to open pipe endpoint B");

    let mut tx_a = pipe_a.tx_ring(0).expect("Pipe A: Failed to get TX ring");
    let mut rx_b = pipe_b.rx_ring(0).expect("Pipe B: Failed to get RX ring");

    // --- Polling Setup ---
    // Get the raw file descriptors.
    let fd_b = pipe_b.as_raw_fd();
    let fd_a = pipe_a.as_raw_fd();

    // Create a Poller and register both descriptors.
    let poller = Poller::new().expect("Failed to create Poller");
    unsafe {
        poller
            .add(fd_b, Event::readable(0))
            .expect("Failed to register fd_b with Poller");
        poller
            .add(fd_a, Event::writable(1))
            .expect("Failed to register fd_a with Poller");
    }

    let mut packets_sent = 0;
    let mut packets_received = 0;
    let mut main_loop_iterations = 0;

    // Buffer for poll events
    let mut events = Events::new();

    println!(
        "Starting event loop. Will send {} packets.",
        NUM_PACKETS_TO_SEND
    );
    println!(
        "Monitoring pipe_b's fd ({}) for readable events (packets from pipe_a).",
        fd_b
    );

    loop {
        main_loop_iterations += 1;
        events.clear(); // Clear events from previous iteration

        // --- Sender Logic (pipe_a) ---
        if packets_sent < NUM_PACKETS_TO_SEND {
            // Poll for a short time to see if fd_a becomes writable.
            match poller.wait(&mut events, Some(Duration::from_millis(0))) {
                Ok(_) => {
                    let mut can_write_to_a = false;
                    for ev in events.iter() {
                        if ev.key == 1 && ev.writable {
                            can_write_to_a = true;
                            break;
                        }
                    }
                    if can_write_to_a || tx_a.has_free_slots() {
                        let mut payload = format!("Packet #{}", packets_sent).into_bytes();
                        payload.resize(60, 0); // Pad to typical minimum packet size

                        match tx_a.send(&payload) {
                            Ok(_) => {
                                tx_a.sync(); // Make packet visible
                                println!(
                                    "[Sender A] Sent packet #{} ({} bytes)",
                                    packets_sent,
                                    payload.len()
                                );
                                packets_sent += 1;
                            }
                            Err(netmap_rs::Error::InsufficientSpace) => {
                                println!("[Sender A] TX ring full, will try later.");
                            }
                            Err(e) => {
                                eprintln!("[Sender A] Error sending packet: {:?}", e);
                                break; // Exit on other errors
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => { /* No events, expected */ }
                Err(e) => {
                    eprintln!("[Sender A] Polling error for fd_a: {:?}", e);
                    break;
                }
            }
        }

        // --- Receiver Logic (pipe_b) ---
        match poller.wait(&mut events, Some(Duration::from_millis(100))) {
            Ok(_) => {
                for ev in events.iter() {
                    if ev.key == 0 && ev.readable {
                        // CRUCIAL: Sync RX ring after poll indicates readability.
                        rx_b.sync();

                        while let Some(frame) = rx_b.recv() {
                            if frame.is_empty() {
                                continue;
                            }
                            packets_received += 1;
                            let len = frame
                                .payload()
                                .iter()
                                .position(|&x| x == 0)
                                .unwrap_or(frame.len());
                            println!(
                                "[Receiver B] Received packet #{} ({} bytes): {:?}",
                                packets_received,
                                frame.len(),
                                String::from_utf8_lossy(&frame.payload()[..len])
                            );
                        }
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // Timeout is fine, just means no events in this period.
            }
            Err(e) => {
                eprintln!("[Event Loop] Polling error: {:?}", e);
                break; // Exit on other errors
            }
        }

        if packets_received >= NUM_PACKETS_TO_SEND && packets_sent >= NUM_PACKETS_TO_SEND {
            println!(
                "All {} packets sent and received. Exiting.",
                NUM_PACKETS_TO_SEND
            );
            break;
        }

        if main_loop_iterations > (NUM_PACKETS_TO_SEND * 10) + 20
            && (packets_received < NUM_PACKETS_TO_SEND)
        {
            println!(
                "Potential stall or slow processing, exiting. Sent: {}, Received: {}",
                packets_sent, packets_received
            );
            break;
        }
    }

    println!(
        "Example finished. Total iterations: {}",
        main_loop_iterations
    );
    Ok(())
}
