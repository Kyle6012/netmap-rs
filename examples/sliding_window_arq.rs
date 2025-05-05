//! Automatic Repeat Request (ARQ) with sliding window protocol example

use netmap_rs::prelude::*;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const WINDOW_SIZE: usize = 8;
const TIMEOUT: Duration = Duration::from_millis(100);

struct ArqSender {
    next_seq: u16,
    window: VecDeque<(u16, Instant)>,
}

impl ArqSender {
    fn new() -> Self {
        Self {
            next_seq: 0,
            window: VecDeque::with_capacity(WINDOW_SIZE),
        }
    }

    fn send_packets(&mut self, tx_ring: &mut TxRing) -> Result<(), Error> {
        while self.window.len() < WINDOW_SIZE {
            let seq = self.next_seq;
            let payload = seq.to_be_bytes();

            tx_ring.send(&payload)?;
            self.window.push_back((seq, Instant::now()));
            self.next_seq = self.next_seq.wrapping_add(1);
        }

        tx_ring.sync();
        Ok(())
    }

    fn check_timeouts(&mut self, tx_ring: &mut TxRing) -> Result<(), Error> {
        let now = Instant::now();
        for (seq, time) in &self.window {
            if now.duration_since(*time) > TIMEOUT {
                println!("Timeout for packet {}", seq);
                return self.send_packets(tx_ring);
            }
        }
        Ok(())
    }

    fn handle_ack(&mut self, ack_seq: u16) {
        while let Some((seq, _)) = self.window.front() {
            if *seq <= ack_seq {
                self.window.pop_front();
            } else {
                break;
            }
        }
    }
}

fn main() -> Result<(), Error> {
    let nm = NetmapBuilder::new("netmap:eth0")
        .num_tx_rings(1)
        .num_rx_rings(1)
        .open()?;

    let mut tx_ring = nm.tx_ring(0)?;
    let mut rx_ring = nm.rx_ring(0)?;
    let mut sender = ArqSender::new();

    sender.send_packets(&mut tx_ring)?;

    loop {
        // Check for timeouts
        sender.check_timeouts(&mut tx_ring)?;

        // Process ACKs
        if let Some(frame) = rx_ring.recv() {
            if frame.len() == 2 {
                let ack_seq = u16::from_be_bytes([frame[0], frame[1]]);
                println!("Received ACK: {}", ack_seq);
                sender.handle_ack(ack_seq);

                // Send more packets if window moved
                sender.send_packets(&mut tx_ring)?;
            }
        }

        if sender.next_seq >= 100 && sender.window.is_empty() {
            break; // All packets acknowledged
        }
    }

    println!("ARQ transmission complete");
    Ok(())
}
