//! Netmap Intra-Process Pipe Example (Thread-to-Thread)
//!
//! This example demonstrates how to use Netmap pipes for zero-copy communication
//! between two threads within the same process.
//!
//! 1. A unique pipe name (e.g., "pipe{myipc}") is defined.
//! 2. The main thread opens this pipe name twice using `NetmapBuilder`:
//!    - The first `Netmap` instance effectively becomes the "master" endpoint of the pipe.
//!    - The second `Netmap` instance becomes the "slave" or peer endpoint.
//! 3. Two threads are spawned:
//!    - A sender thread uses the TX ring of the first `Netmap` instance.
//!    - A receiver thread uses the RX ring of the second `Netmap` instance.
//! 4. Packets are sent from the sender to the receiver and their contents are verified.
//! 5. A reply can be sent back from the receiver to the sender.
//!
//! Usage:
//! cargo run --example pipe_threads --features sys
//!
//! Note: Netmap pipes typically provide 1 TX and 1 RX ring per endpoint by default.

use std::error::Error;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use netmap_rs::prelude::*;

const PIPE_NAME: &str = "netmap:pipe{intraproc_example_123}"; // Unique pipe name
const NUM_PACKETS: usize = 5;
const PACKET_BASE_PAYLOAD: &[u8] = b"Hello from pipe sender, msg=";

fn sender_thread(
    mut tx_pipe_ep: Netmap,
    done_tx: mpsc::Sender<String>,
) -> Result<(), String> {
    println!("[Sender] Thread started. TX Rings: {}", tx_pipe_ep.num_tx_rings());
    if tx_pipe_ep.num_tx_rings() == 0 {
        return Err("[Sender] No TX rings available on pipe endpoint.".to_string());
    }
    let mut tx_ring = tx_pipe_ep
        .tx_ring(0)
        .map_err(|e| format!("[Sender] Failed to get TX ring: {:?}", e))?;

    for i in 0..NUM_PACKETS {
        let mut payload = PACKET_BASE_PAYLOAD.to_vec();
        payload.extend_from_slice(i.to_string().as_bytes());

        print!("[Sender] Sending packet {} ({} bytes): {:?}...", i, payload.len(), std::str::from_utf8(&payload).unwrap_or("non-utf8"));
        match tx_ring.send(&payload) {
            Ok(_) => {
                tx_ring.sync(); // Make packet visible to receiver
                println!(" Sent.");
            }
            Err(e) => {
                return Err(format!("[Sender] Failed to send packet {}: {:?}", i, e));
            }
        }
        thread::sleep(Duration::from_millis(10)); // Small delay
    }
    done_tx.send("[Sender] All packets sent.".to_string()).unwrap();
    Ok(())
}

fn receiver_thread(
    mut rx_pipe_ep: Netmap,
    done_rx: mpsc::Sender<String>,
) -> Result<(), String> {
    println!("[Receiver] Thread started. RX Rings: {}", rx_pipe_ep.num_rx_rings());
     if rx_pipe_ep.num_rx_rings() == 0 {
        return Err("[Receiver] No RX rings available on pipe endpoint.".to_string());
    }
    let mut rx_ring = rx_pipe_ep
        .rx_ring(0)
        .map_err(|e| format!("[Receiver] Failed to get RX ring: {:?}", e))?;

    let mut packets_received = 0;
    let mut missed_count = 0;
    const MAX_MISSES: usize = 100; // ~1 second of trying after sender might be done

    while packets_received < NUM_PACKETS && missed_count < MAX_MISSES {
        rx_ring.sync(); // Check for new packets
        let mut received_in_batch = 0;
        while let Some(frame) = rx_ring.recv() {
            if frame.is_empty() {
                continue;
            }
            received_in_batch +=1;
            let mut expected_payload = PACKET_BASE_PAYLOAD.to_vec();
            expected_payload.extend_from_slice(packets_received.to_string().as_bytes());

            println!(
                "[Receiver] Received packet {} ({} bytes): {:?}",
                packets_received, frame.len(), std::str::from_utf8(frame.payload()).unwrap_or("non-utf8")
            );

            assert_eq!(frame.payload(), expected_payload.as_slice(), "Packet content mismatch!");
            packets_received += 1;
            if packets_received == NUM_PACKETS {
                break;
            }
        }
        if received_in_batch == 0 {
            missed_count += 1;
            thread::sleep(Duration::from_millis(10)); // Wait if no packets
        } else {
            missed_count = 0; // Reset miss counter if packets were received
        }
    }

    if packets_received == NUM_PACKETS {
        done_rx.send(format!("[Receiver] Successfully received all {} packets.", NUM_PACKETS)).unwrap();
        Ok(())
    } else {
        Err(format!("[Receiver] Timed out. Received only {} out of {} packets.", packets_received, NUM_PACKETS))
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("Netmap Pipe Thread-to-Thread Communication Example");
    println!("Using pipe name: {}", PIPE_NAME);

    // Open the first endpoint of the pipe (master)
    // Pipes typically default to 1 TX and 1 RX ring.
    // Explicitly requesting 1 for clarity, or 0 for default.
    let pipe_ep1 = NetmapBuilder::new(PIPE_NAME)
        .num_tx_rings(1)
        .num_rx_rings(1)
        .build()
        .map_err(|e| format!("Failed to open pipe endpoint 1 (master): {:?}", e))?;
    println!("Pipe endpoint 1 (master) opened. TX rings: {}, RX rings: {}", pipe_ep1.num_tx_rings(), pipe_ep1.num_rx_rings());


    // Open the second endpoint of the pipe (slave/peer)
    let pipe_ep2 = NetmapBuilder::new(PIPE_NAME)
        .num_tx_rings(1)
        .num_rx_rings(1)
        .build()
        .map_err(|e| format!("Failed to open pipe endpoint 2 (slave): {:?}", e))?;
    println!("Pipe endpoint 2 (slave) opened. TX rings: {}, RX rings: {}", pipe_ep2.num_tx_rings(), pipe_ep2.num_rx_rings());

    let (done_tx_s, done_tx_r) = mpsc::channel();
    let (done_rx_s, done_rx_r) = mpsc::channel();

    // Spawn sender thread with pipe_ep1
    let sender_handle = thread::spawn(move || sender_thread(pipe_ep1, done_tx_s));

    // Spawn receiver thread with pipe_ep2
    let receiver_handle = thread::spawn(move || receiver_thread(pipe_ep2, done_rx_s));

    // Wait for threads to complete
    match sender_handle.join() {
        Ok(Ok(_)) => println!("{}", done_tx_r.recv().unwrap_or_default()),
        Ok(Err(e)) => eprintln!("[Main] Sender thread failed: {}", e),
        Err(e) => eprintln!("[Main] Sender thread panicked: {:?}", e),
    }

    match receiver_handle.join() {
        Ok(Ok(_)) => println!("{}", done_rx_r.recv().unwrap_or_default()),
        Ok(Err(e)) => eprintln!("[Main] Receiver thread failed: {}", e),
        Err(e) => eprintln!("[Main] Receiver thread panicked: {:?}", e),
    }

    println!("Example finished.");
    Ok(())
}
