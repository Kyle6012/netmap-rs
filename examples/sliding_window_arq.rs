#![cfg(feature = "sys")]

use netmap_rs::prelude::*;
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

const WINDOW_SIZE: usize = 4;
const TIMEOUT: Duration = Duration::from_millis(100);
const MAX_RETRIES: usize = 3;

struct Sender {
    next_seq_num: u32,
    base: u32,
    buffer: HashMap<u32, Vec<u8>>,
    timers: HashMap<u32, Instant>,
    retries: HashMap<u32, usize>,
}

impl Sender {
    fn new() -> Self {
        Self {
            next_seq_num: 0,
            base: 0,
            buffer: HashMap::new(),
            timers: HashMap::new(),
            retries: HashMap::new(),
        }
    }

    fn send_packets(&mut self, tx_ring: &mut TxRing) -> Result<(), Error> {
        while self.next_seq_num < self.base + WINDOW_SIZE {
            let packet = format!("Packet {}", self.next_seq_num).into_bytes();
            self.buffer.insert(self.next_seq_num, packet.clone());
            self.timers.insert(self.next_seq_num, Instant::now());
            self.retries.insert(self.next_seq_num, 0);

            tx_ring.send(&packet)?;
            println!("Sent: Packet {}", self.next_seq_num);
            self.next_seq_num += 1;
        }
        tx_ring.sync();
        Ok(())
    }

    fn check_timeouts(&mut self, tx_ring: &mut TxRing) -> Result<(), Error> {
        let now = Instant::now();
        let mut retransmit_packets = VecDeque::new();

        for (&seq_num, timer) in &self.timers {
            if now.duration_since(*timer) > TIMEOUT {
                if self.retries[&seq_num] < MAX_RETRIES {
                    if let Some(packet_data) = self.buffer.get(&seq_num) {
                        retransmit_packets.push_back((seq_num, packet_data.clone()));
                    }
                } else {
                    println!("Max retries for packet {} reached.", seq_num);
                    // Handle failure, e.g. remove packet or abort
                }
            }
        }

        for (seq_num, packet_data) in retransmit_packets {
            tx_ring.send(&packet_data)?;
            println!("Retransmitted: Packet {}", seq_num);
            self.timers.insert(seq_num, Instant::now());
            *self.retries.entry(seq_num).or_insert(0) += 1;
        }
        if !tx_ring.is_empty() {
            tx_ring.sync();
        }
        Ok(())
    }

    fn handle_ack(&mut self, ack_num: u32) {
        println!("Received ACK: {}", ack_num);
        self.base = ack_num + 1;
        self.buffer.retain(|&k, _| k >= self.base);
        self.timers.retain(|&k, _| k >= self.base);
        self.retries.retain(|&k, _| k >= self.base);
    }
}

fn main() -> Result<(), Error> {
    let nm = NetmapBuilder::new("netmap:eth0") // replace with your interface
        .num_tx_rings(1)
        .num_rx_rings(1)
        .build()?;

    let mut tx_ring = nm.tx_ring(0)?;
    let mut rx_ring = nm.rx_ring(0)?;
    let mut sender = Sender::new();

    loop {
        sender.send_packets(&mut tx_ring)?;
        sender.check_timeouts(&mut tx_ring)?;

        if let Some(frame) = rx_ring.recv() {
            if let Ok(ack_str) = std::str::from_utf8(frame.payload()) {
                if ack_str.starts_with("ACK ") {
                    if let Ok(ack_num) = ack_str[4..].parse::<u32>() {
                        sender.handle_ack(ack_num);
                    }
                }
            }
        }
        rx_ring.sync();

        if sender.base >= 10 { // Example: stop after 10 packets are ACKed
            println!("All packets sent and acknowledged.");
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}
