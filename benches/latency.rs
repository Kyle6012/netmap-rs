use criterion::{Criterion, black_box, criterion_group, criterion_main};
use netmap_rs::prelude::*;
use std::time::Duration;

fn single_packet_latency(c: &mut Criterion) {
    let nm = NetmapBuilder::new("netmap:eth0")
        .num_tx_rings(1)
        .num_rx_rings(1)
        .open()
        .expect("Failed to Open Netmap interface");

    let mut tx_ring = nm.tx_ring(0).expect("Failed to get TX ring");
    let mut rx_ring = nm.rx_ring(0).expect("Failed to get RX ring");
    let payload = vec![0u8; 64]; // 64 byte packet

    c.bench_function(
        "single_packet_round_trip" | b | {
            b.iter(|| {
                tx_ring.send(black_box(&payload)).expect("Send failed");
                tx_ring.sync();

                while rx_ring.recv().is_none() {
                    // spin until the packet is received
                }
            });
        },
    );
}

fn batch_latency(c: &mut Criterion) {
    let nm = NetmapBuilder::new("netmap:eth0")
        .num_tx_rings(1)
        .num_rx_rings(1)
        .open()
        .expect("Failed to open Netmap interface");

    let mut tx_ring = nm.tx_ring(0).expect("Failed to get TX ring");
    let mut rx_ring = nm.rx_ring(0).expect("Failed to get RX ring");
    let payload = vec![0u8; 64];
    let batch_size = 32;

    c.bench_function(&format!("batch_{}_packets", batch_size), |b| {
        b.iter(|| {
            let mut reservation = tx_ring
                .reserve_batch(black_box(batch_size))
                .expect("Reservation failed");

            for i in 0..batch_size {
                let pkt = reservation
                    .packet(i, payload.len())
                    .expect("Packet access failed");
                pkt.copy_from_slice(&payload);
            }

            reservation.commit();
            tx_ring.sync();

            let mut received = 0;
            while received < batch_size {
                if rx_ring.recv().is_none() {
                    received += 1;
                }
            }
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets = single_packet_latency, batch_latency
}

criterion_main!(benches);
