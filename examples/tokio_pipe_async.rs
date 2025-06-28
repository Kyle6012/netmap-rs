//! Tokio Async Netmap Pipe Example
//!
//! This example demonstrates how to use the `AsyncNetmapRxRing` and `AsyncNetmapTxRing`
//! wrappers for asynchronous I/O with Netmap pipes using Tokio.
//!
//! It sets up two Netmap pipe endpoints:
//! - Endpoint A: Acts as the sender.
//! - Endpoint B: Acts as the receiver.
//!
//! Two Tokio tasks are spawned:
//! - The sender task writes a few packets to its `AsyncNetmapTxRing`.
//! - The receiver task reads packets from its `AsyncNetmapRxRing`.
//!
//! Usage:
//! cargo run --example tokio_pipe_async --features "tokio-async sys"
//!
//! Note: This example relies on the correct implementation of NIOCRXSYNC/NIOCTXSYNC
//! ioctls within the AsyncRead/AsyncWrite wrappers.

#![cfg(feature = "tokio-async")]

use std::error::Error;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// Assuming netmap_rs prelude might not yet include the tokio types, import directly.
use netmap_rs::NetmapBuilder;
use netmap_rs::tokio_async::{TokioNetmap, AsyncNetmapRxRing, AsyncNetmapTxRing};


// Use a unique pipe name for this example
const ASYNC_PIPE_NAME: &str = "netmap:pipe{tokio_async_example_789}";
const ASYNC_NUM_PACKETS: usize = 5;
const ASYNC_PACKET_SIZE: usize = 60; // Minimum Ethernet frame size

async fn sender_task(mut tx_ring: AsyncNetmapTxRing) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("[Async Sender] Task started.");
    for i in 0..ASYNC_NUM_PACKETS {
        let mut payload = format!("AsyncPacket #{}", i).into_bytes();
        payload.resize(ASYNC_PACKET_SIZE, 0); // Pad to ensure fixed size

        print!("[Async Sender] Sending packet #{} ({} bytes)...", i, payload.len());

        // Write the packet
        tx_ring.write_all(&payload).await?;
        // Flush to ensure it's sent (calls NIOCTXSYNC)
        tx_ring.flush().await?;

        println!(" Sent.");
        tokio::time::sleep(Duration::from_millis(50)).await; // Small delay
    }
    // Shutdown the write side (optional, calls flush)
    // tx_ring.shutdown().await?;
    println!("[Async Sender] All packets sent and flushed.");
    Ok(())
}

async fn receiver_task(mut rx_ring: AsyncNetmapRxRing) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("[Async Receiver] Task started. Waiting for packets...");
    let mut packets_received = 0;
    let mut receive_buffer = vec![0u8; ASYNC_PACKET_SIZE * 2]; // Buffer large enough for typical MTU

    while packets_received < ASYNC_NUM_PACKETS {
        print!("[Async Receiver] Attempting to read packet #{}...", packets_received);
        match rx_ring.read(&mut receive_buffer).await {
            Ok(0) => {
                // EOF typically means the other side closed.
                println!(" EOF received. Assuming sender finished.");
                break;
            }
            Ok(n) => {
                let received_payload = &receive_buffer[..n];
                println!(
                    " Received {} bytes: '{}'",
                    n,
                    String::from_utf8_lossy(&received_payload[..std::cmp::min(n, 20)]) // Print first 20 chars
                );

                // Verification (simple check based on expected format)
                let expected_prefix = format!("AsyncPacket #{}", packets_received);
                if !received_payload.starts_with(expected_prefix.as_bytes()) {
                    eprintln!(
                        "[Async Receiver] Payload mismatch! Expected prefix: '{}', Got: '{}'",
                        expected_prefix,
                        String::from_utf8_lossy(&received_payload[..std::cmp::min(n, expected_prefix.len())])
                    );
                }
                packets_received += 1;
            }
            Err(e) => {
                eprintln!("[Async Receiver] Error reading from ring: {}", e);
                return Err(Box::new(e));
            }
        }
    }

    if packets_received >= ASYNC_NUM_PACKETS {
        println!("[Async Receiver] Successfully received all {} packets.", ASYNC_NUM_PACKETS);
    } else {
        println!("[Async Receiver] Finished. Received {} out of {} packets.", packets_received, ASYNC_NUM_PACKETS);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    println!(
        "Tokio Async Netmap Pipe Example using '{}'",
        ASYNC_PIPE_NAME
    );

    // --- Setup Pipe Endpoints ---
    // Create Netmap instances first
    let netmap_a = NetmapBuilder::new(ASYNC_PIPE_NAME)
        .num_tx_rings(1)
        .num_rx_rings(1) // Netmap pipes have 1TX/1RX by default
        .build()
        .map_err(|e| format!("Failed to open pipe endpoint A (master): {:?}", e))?;

    let netmap_b = NetmapBuilder::new(ASYNC_PIPE_NAME)
        .num_tx_rings(1)
        .num_rx_rings(1)
        .build()
        .map_err(|e| format!("Failed to open pipe endpoint B (slave): {:?}", e))?;

    // Wrap them with TokioNetmap for async capability
    let tokio_netmap_a = TokioNetmap::new(netmap_a)?;
    let tokio_netmap_b = TokioNetmap::new(netmap_b)?;

    // Get the async ring wrappers
    // Endpoint A sends, Endpoint B receives.
    let async_tx_a = tokio_netmap_a.tx_ring(0)?;
    let async_rx_b = tokio_netmap_b.rx_ring(0)?;

    println!("[Main] Pipe endpoints created and wrapped for Tokio.");

    // Spawn sender and receiver tasks
    let sender_handle = tokio::spawn(sender_task(async_tx_a));
    let receiver_handle = tokio::spawn(receiver_task(async_rx_b));

    // Wait for both tasks to complete
    let sender_result = sender_handle.await?;
    if let Err(e) = sender_result {
        eprintln!("[Main] Sender task failed: {}", e);
    }

    let receiver_result = receiver_handle.await?;
    if let Err(e) = receiver_result {
        eprintln!("[Main] Receiver task failed: {}", e);
    }

    println!("[Main] Example finished.");
    Ok(())
}
