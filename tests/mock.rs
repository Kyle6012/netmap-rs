use netmap_rs:: fallback::{FallbackRxRing, FallbackTxRing};
use netmap_rs::frame::Frame;

#[test]
fn test_fallback_ring() {
    let tx_ring = FallbackTxRing::new(32);
    let rx_ring = FallbackRxRing::new(32);

    // test single packet

    tx_ring.send(b"test").unwrap();
    assert_eq!(rx_ring.recv().unwrap().payload(), b"test");

    // test would block
    for _ in 0..31 {
        tx_ring.send(b"test").unwrap();
    }
    assert!(tx_ring.send(b"test").is_err()); // should be full
}
 #[test]
 fn test_threaded_fallback() {
    use std::thread;
    use std::time::Duration;

    let tx_ring = FallbackTxRing::new(32);
    let rx_ring = FallbackRxRing::new(32);

    let tx_handle = thread::spawn(move || {
        for i in 0..10 {
            tx_ring.send(&[i]).unwrap();
            thread::sleep(Duration::from_millis(10));
        }
    });

    let rx_handle = thread::spawn(move || {
        for i in 0..10 {
            while let Some(frame) = rx_ring.recv() {
                assert_eq!(frame.payload(), &[i]);
                break;
            }
        }
    });

    tx_handle.join().unwrap();
    rx_handle.join().unwrap();

 }