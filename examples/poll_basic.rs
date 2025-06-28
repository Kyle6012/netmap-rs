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
use polling::{Event, Poller};

// Use a unique pipe name for this example
const PIPE_NAME_POLL: &str = "netmap:pipe{poll_example_456}";
const NUM_PACKETS_TO_SEND: usize = 5;

fn main() -> Result<(), Box<dyn Error>> {
    println!("Netmap Polling Example using '{}'", PIPE_NAME_POLL);

    // --- Setup Pipe Endpoints ---
    // Endpoint A will send, Endpoint B will receive.
    let mut pipe_a = NetmapBuilder::new(PIPE_NAME_POLL)
        .num_tx_rings(1)
        .num_rx_rings(1) // Though not used for RX in A for this simple example
        .build()
        .expect("Failed to open pipe endpoint A");

    let mut pipe_b = NetmapBuilder::new(PIPE_NAME_POLL)
        .num_tx_rings(1) // Though not used for TX in B for this simple example
        .num_rx_rings(1)
        .build()
        .expect("Failed to open pipe endpoint B");

    let mut tx_a = pipe_a.tx_ring(0).expect("Pipe A: Failed to get TX ring");
    let mut rx_b = pipe_b.rx_ring(0).expect("Pipe B: Failed to get RX ring");

    // --- Polling Setup ---
    // Get the raw file descriptor for pipe_b (receiver)
    let fd_b = pipe_b.as_raw_fd();

    // Create a Poller
    let poller = Poller::new().expect("Failed to create Poller");

    // Register fd_b for readability (POLLIN). Key 0 is arbitrary for this example.
    // We use level-triggered polling by default with the `polling` crate.
    poller.add(fd_b, Event::readable(0)).expect("Failed to register fd_b with Poller");

    let mut packets_sent = 0;
    let mut packets_received = 0;
    let mut main_loop_iterations = 0;

    // Buffer for poll events
    let mut events = Vec::new();

    println!("Starting event loop. Will send {} packets.", NUM_PACKETS_TO_SEND);
    println!("Monitoring pipe_b's fd ({}) for readable events (packets from pipe_a).", fd_b);

    loop {
        main_loop_iterations += 1;
        events.clear(); // Clear events from previous iteration

        // --- Sender Logic (pipe_a) ---
        if packets_sent < NUM_PACKETS_TO_SEND {
            // In a real app, you might also poll fd_a for writability (POLLOUT)
            // before attempting to send, especially if sends can fill up the ring.
            // For simplicity here, we try to send directly if space is likely.
            // Let's add a simple POLLOUT check for demonstration:

            let fd_a = pipe_a.as_raw_fd();
            // Temporarily add fd_a for writability check
            poller.add(fd_a, Event::writable(1)).expect("Failed to add fd_a for write polling");

            // Wait for a short time to see if fd_a becomes writable
            match poller.wait(&mut events, Some(Duration::from_millis(0))) { // Non-blocking check
                Ok(_) => {
                    let mut can_write_to_a = false;
                    for ev in &events {
                        if ev.key == 1 && ev.writable { // Event for fd_a and it's writable
                            can_write_to_a = true;
                            break;
                        }
                    }
                    if can_write_to_a || tx_a.num_slots() - (tx_a.head() - tx_a.tail() + tx_a.num_slots() as u32) % tx_a.num_slots() as u32 > 1 { // Heuristic: check space if poll didn't signal
                        let mut payload = format!("Packet #{}", packets_sent).into_bytes();
                        payload.resize(60, 0); // Pad to typical minimum packet size

                        match tx_a.send(&payload) {
                            Ok(_) => {
                                tx_a.sync(); // Make packet visible
                                println!("[Sender A] Sent packet #{} ({} bytes)", packets_sent, payload.len());
                                packets_sent += 1;
                            }
                            Err(Error::InsufficientSpace) => {
                                println!("[Sender A] TX ring full, will try later.");
                                // tx_a.sync(); // Sync to update tail pointer if needed
                            }
                            Err(e) => {
                                eprintln!("[Sender A] Error sending packet: {:?}", e);
                                break; // Exit on other errors
                            }
                        }
                    } else {
                         // println!("[Sender A] fd_a not writable yet via poll, or no space.");
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => { /* No events, expected */ }
                Err(e) => {
                    eprintln!("[Sender A] Polling error for fd_a: {:?}", e);
                    break;
                }
            }
            // IMPORTANT: Remove fd_a if we only want to poll it transiently, or modify its interest.
            // For this example, we re-add fd_b for readability if it was overwritten by fd_a.
            // A better way is to use Poller::modify or have separate pollers if keys are same.
            // Or ensure keys are different and keep both registered.
            // Here, we simply re-register/modify fd_b for readability after the write check.
            // If fd_a was added with key 1, and fd_b with key 0, they are distinct.
            // Let's ensure fd_b is still monitored for reads.
            // If add is called again for an existing fd, it acts like modify.
            poller.modify(fd_a, Event::none(1)).expect("Failed to remove interest from fd_a"); // Stop polling fd_a for now
        }

        // --- Receiver Logic (pipe_b) ---
        // Wait for events on registered file descriptors (specifically fd_b for reading)
        // Timeout of 100ms for this example loop
        match poller.wait(&mut events, Some(Duration::from_millis(100))) {
            Ok(_) => {
                for ev in &events {
                    if ev.key == 0 { // Event for fd_b (receiver)
                        if ev.readable {
                            // println!("[Receiver B] fd {} readable event received.", fd_b);
                            // CRUCIAL: Sync RX ring after poll indicates readability
                            rx_b.sync();

                            let mut received_in_batch = 0;
                            while let Some(frame) = rx_b.recv() {
                                if frame.is_empty() { continue; }
                                received_in_batch += 1;
                                println!(
                                    "[Receiver B] Received packet #{} ({} bytes): {:?}",
                                    packets_received,
                                    frame.len(),
                                    String::from_utf8_lossy(&frame.payload()[..frame.payload().iter().position(|&x| x == 0).unwrap_or(frame.len())])
                                );
                                packets_received += 1;
                            }
                            if received_in_batch > 0 {
                                // println!("[Receiver B] Processed {} packets in this batch.", received_in_batch);
                            } else {
                                // This can happen if poll indicated ready, but by the time we sync and recv,
                                // packets are gone (e.g. another thread/process got them, though not in this example)
                                // or if it was a spurious wakeup. For Netmap, usually means sync and check again.
                                // println!("[Receiver B] fd readable, but no packets after sync/recv. Ring head: {}, tail: {}, cur: {}", rx_b.head(), rx_b.tail(), rx_b.cur());
                            }
                        }
                        if ev.writable { /* Not expecting writable events for fd_b with key 0 */ }
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // Timeout is fine, just means no events in this period.
                // Useful for periodic tasks or checking exit conditions.
                // println!("[Event Loop] Poll timed out. Iteration: {}", main_loop_iterations);
            }
            Err(e) => {
                eprintln!("[Event Loop] Polling error: {:?}", e);
                break; // Exit on other errors
            }
        }

        // Update interest for fd_b for next iteration (level-triggered, so often re-arms automatically)
        // poller.modify(fd_b, Event::readable(0)).expect("Failed to re-arm fd_b");


        if packets_received >= NUM_PACKETS_TO_SEND && packets_sent >= NUM_PACKETS_TO_SEND {
            println!("All {} packets sent and received. Exiting.", NUM_PACKETS_TO_SEND);
            break;
        }

        if main_loop_iterations > (NUM_PACKETS_TO_SEND * 10) + 20 && (packets_received < NUM_PACKETS_TO_SEND) {
             println!("Potential stall or slow processing, exiting. Sent: {}, Received: {}", packets_sent, packets_received);
             break;
        }
        // Small delay to make console output more readable if very fast
        // std::thread::sleep(Duration::from_millis(10));
    }

    // Clean up poller by removing descriptors (optional, as Poller::drop will do it)
    poller.delete(fd_b).ok(); // Ignore error if already removed or never added properly
    // poller.delete(fd_a).ok(); // fd_a was transient

    println!("Example finished. Total iterations: {}", main_loop_iterations);
    Ok(())
}
