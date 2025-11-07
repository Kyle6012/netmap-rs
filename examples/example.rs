//! Example demonstrating the updated netmap-rs functionality
//! This example shows proper error handling and the updated API

use netmap_rs::prelude::*;
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Netmap-rs Updated Example ===");
    
    // First, let's demonstrate building without netmap (fallback mode)
    println!("\n1. Testing without sys feature (if available)...");
    
    // Now test with sys feature enabled
    println!("\n2. Testing with sys feature enabled...");
    
    // Try to open a netmap interface
    // This will fail if netmap is not installed, but we'll handle it gracefully
    let result = std::panic::catch_unwind(|| {
        NetmapBuilder::new("lo") // Use loopback interface for testing
            .num_tx_rings(1)
            .num_rx_rings(1)
            .build()
    });
    
    match result {
        Ok(Ok(nm)) => {
            println!("✓ Successfully opened netmap interface");
            
            // Get ring information
            println!("  - TX rings: {}", nm.num_tx_rings());
            println!("  - RX rings: {}", nm.num_rx_rings());
            println!("  - Is host interface: {}", nm.is_host_if());
            
            // Get ring handles
            let tx_result = nm.tx_ring(0);
            let rx_result = nm.rx_ring(0);
            
            match (tx_result, rx_result) {
                (Ok(mut tx_ring), Ok(mut rx_ring)) => {
                    println!("✓ Successfully got ring handles");
                    
                    // Test packet sending
                    let test_packet = b"Hello from updated netmap-rs!";
                    
                    match tx_ring.send(test_packet) {
                        Ok(_) => {
                            println!("✓ Successfully queued packet for transmission");
                            tx_ring.sync();
                            println!("✓ Synced TX ring");
                        }
                        Err(e) => println!("✗ Failed to send packet: {}", e),
                    }
                    
                    // Test packet receiving (non-blocking)
                    rx_ring.sync();
                    match rx_ring.recv() {
                        Some(frame) => {
                            println!("✓ Received packet: {} bytes", frame.len());
                            println!("  Packet data (first 50 bytes): {:?}", 
                                &frame.payload()[..frame.len().min(50)]);
                        }
                        None => println!("ℹ No packets available to receive"),
                    }
                    
                    // Test batch operations
                    println!("\n3. Testing batch operations...");
                    
                    match tx_ring.reserve_batch(5) {
                        Ok(mut batch) => {
                            println!("✓ Successfully reserved batch of 5 slots");
                            
                            // Fill batch with test packets
                            for i in 0..5 {
                                let packet_data = format!("Batch packet {}", i);
                                match batch.packet(i, packet_data.len()) {
                                    Ok(buf) => {
                                        buf.copy_from_slice(packet_data.as_bytes());
                                        println!("  ✓ Prepared packet {}: {}", i, packet_data);
                                    }
                                    Err(e) => println!("  ✗ Failed to get packet {}: {}", i, e),
                                }
                            }
                            
                            batch.commit();
                            println!("✓ Committed batch");
                        }
                        Err(e) => println!("✗ Failed to reserve batch: {}", e),
                    }
                    
                    // Test batch receive
                    let mut recv_batch = vec![Frame::new_owned(vec![0u8; 1500]); 10];
                    let received = rx_ring.recv_batch(&mut recv_batch);
                    if received > 0 {
                        println!("✓ Received {} packets in batch", received);
                    } else {
                        println!("ℹ No packets available for batch receive");
                    }
                }
                (Err(e), _) => println!("✗ Failed to get TX ring: {}", e),
                (_, Err(e)) => println!("✗ Failed to get RX ring: {}", e),
            }
        }
        Ok(Err(e)) => {
            println!("✗ Failed to open netmap interface: {}", e);
            println!("  This is expected if netmap is not installed or properly configured");
            println!("  See README.md for installation instructions");
        }
        Err(_) => {
            println!("✗ Panic occurred while trying to open netmap interface");
            println!("  This might indicate a more serious issue with the netmap installation");
        }
    }
    
    // Demonstrate error types
    println!("\n4. Error handling demonstration...");
    
    // Create some errors to show proper error handling
    let io_error: std::io::Error = std::io::Error::new(std::io::ErrorKind::NotFound, "test error");
    let netmap_error: Error = io_error.into();
    println!("✓ Converted IO error to netmap error: {}", netmap_error);
    
    // Show different error variants
    let errors = vec![
        Error::WouldBlock,
        Error::BindFail("test interface".to_string()),
        Error::InvalidRingIndex(42),
        Error::PacketTooLarge(9000),
        Error::InsufficientSpace,
        Error::UnsupportedPlatform("test platform".to_string()),
        Error::FallbackUnsupported("test feature".to_string()),
    ];
    
    for error in errors {
        println!("  Error variant: {}", error);
    }
    
    println!("\n=== Example completed ===");
    Ok(())
}