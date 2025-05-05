#![cfg(all(unix, feature = "sys"))]
mod netmap_tests {
    use netmap_rs::prelude::*;
    use std::time::Duration;

    #[test]
    fn test_netmap_creation() {
        let nm = NetmapBuilder::new("netmap:eth0")
            .num_tx_rings(1)
            .num_rx_rings(1)
            .open();

        assert!(nm.is_ok(), "Failed to create Netmap instance")
    }

    #[test]
    fn test_ring_operations() {
        let nm = NetmapBuilder::new("netmap:eth0")
            .num_tx_rings(1)
            .num_rx_rings(1)
            .open()
            .expect("Failed to open Netmap interface");

        let mut tx_ring = nm.tx_ring(0).expect("Failed to get TX ring");
        let mut rx_ring = nm.rx_ring(0).expect("Failed to get RX ring");

        // test single packet
        tx_ring.send(b"test").expect("Send failed");
        tx_ring.sync();

        let start = std::time::Instant::now();
        let mut received = None;

        while start.elapsed() < Duration::from_secs(1) {
            if let Some(frame) = rx_ring.recv() {
                received = Some(frame.payload().to_vec());
                break;
            }
        }

        assert_eq!(received.as_deref(), Some(b"test".as_ref()));
    }

    #[test]
    fn test_batch_operations() {
        let nm = NetmapBuilder::new("netmap:eth0")
            .num_tx_rings(1)
            .num_rx_rings(1)
            .open()
            .expect("Failed to open Netmap interface");

        let mut tx_ring = nm.tx_ring(0).expect("Failed to get TX ring");
        let mut rx_ring = nm.rx_ring(0).expect("Failed to get RX ring");
        let batch_size = 8;

        // send batch
        let mut reservation = tx_ring
            .reserve_batch(batch_size)
            .expect("Reservation failed");
        for i in 0..batch_size {
            let pkt = reservation.packet(i, 1).expect("Packet access failed");
            pkt[0] = i as u8;
        }
        reservation.commit();
        tx_ring.sync();

        // receive batch
        let mut frames = vec![Frame::default(); batch_size];
        let mut received = 0;
        let start = std::time::Instant::now();

        while received < batch_size && start.elapsed() < Duration::from_secs(1) {
            received += rx_ring.recv_batch(&mut frames[received..]);
        }

        assert_eq!(received, batch_size);
        for (i, frame) in frames.iter().enumerate() {
            assert_eq!(frame.payload(), &[i as u8]);
        }
    }
}

#[cfg(not(all(unix, feature = "sys")))]
mod fallback_tests {
    #[test]
    fn test_fallback_mode() {
        // this just confirms the tests compile in fallback mode
        assert!(true);
    }
}
