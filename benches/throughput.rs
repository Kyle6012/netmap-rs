use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use netmap_rs::prelude::*;
use std::time::Duration;

fn throughput(c: &mut Criterion) {
    let nm = NetmapBuilder::new("netmap:eth0")
        .num_tx_rings(1)
        .num_rx_rings(1)
        .open()
        .expect("Failed to open Netmap interface");

    let mut tx_ring = nm.tx_ring(0).expect("Failed to get TX ring");
    let mut rx_ring = nm.rx_ring(0).expect("Failed to get RX ring");

    let mut group = c.benchmark_group("throughput");
    group.measurement_time(Duration::from_secs(5));

    for size in [64, 128, 256, 512, 1024, 1500].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        let payload = vec![0u8: *size];
        let batch_size = 64;

        group.benchmark_group(&format!("{}_bytes", size), |b| {
            b.iter(|| {
                // send batch
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

                // receive batch
                let mut frames = vec![Frame::default(); batch_size];
                let mut received = 0;
                while received < batch_size {
                    received += rx_ring.recv_batch(&mut frames[received..]);
                }
            });
        });
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().warm_up_time(Duration::from_secs(!));
    targets = throughput
}

criterion_main!(benches);
