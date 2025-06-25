use netmap_rs::fallback::{create_fallback_channel, FallbackRxRing, FallbackTxRing};
use netmap_rs::prelude::Error; // Only Error is explicitly used from prelude
use std::thread;
use std::time::Duration;


#[test]
fn test_fallback_ring() {
    let (tx_ring, rx_ring) = create_fallback_channel(32);

    // test single packet
    tx_ring.send(b"test").unwrap();
    let frame = rx_ring.recv().unwrap();
    assert_eq!(frame.payload(), b"test");

    // test would block
    // Ring capacity is 32. It's currently empty as the first packet was sent and received.
    // Send 32 packets to fill it completely.
    for i in 0..32 {
        tx_ring.send(&[i as u8]).unwrap(); // Use different payloads to be sure
    }
    // Now the queue has 32 elements. The next send should fail.
    match tx_ring.send(b"overflow") {
        Err(Error::WouldBlock) => { /* Expected */ }
        Err(e) => panic!("Expected WouldBlock, got {:?}", e),
        Ok(_) => panic!("Expected WouldBlock, but send succeeded"),
    }
}

#[test]
fn test_threaded_fallback() {
    let (tx_ring, rx_ring): (FallbackTxRing, FallbackRxRing) = create_fallback_channel(32);
    let num_packets = 10;

    let tx_handle = thread::spawn(move || {
        for i in 0..num_packets {
            loop {
                match tx_ring.send(&[i as u8]) {
                    Ok(_) => break,
                    Err(Error::WouldBlock) => thread::sleep(Duration::from_millis(1)), // Yield if full
                    Err(e) => panic!("Send error: {:?}", e),
                }
            }
            thread::sleep(Duration::from_millis(5)); // Small delay to allow receiver to catch up occasionally
        }
    });

    let rx_handle = thread::spawn(move || {
        let mut received_packets = Vec::new();
        for _ in 0..num_packets {
            loop {
                if let Some(frame) = rx_ring.recv() {
                    received_packets.push(frame.payload().to_vec());
                    break;
                }
                thread::sleep(Duration::from_millis(1)); // Yield if no packet
            }
        }
        // Check if all packets were received (order might not be guaranteed by this simple test alone,
        // but for a single producer/consumer on VecDeque, it should be).
        assert_eq!(received_packets.len(), num_packets);
        for i in 0..num_packets {
            assert_eq!(received_packets[i], vec![i as u8]);
        }
    });

    tx_handle.join().unwrap();
    rx_handle.join().unwrap();
}
