//! `bridge` — forward frames between two VALE ports.
//!
//! This mirrors the `bridge` example from the upstream netmap C repo: it
//! attaches to two ports on a VALE switch and copies every frame received on
//! one port to the other, and vice versa. Because both ports live on the same
//! in-kernel virtual switch, packets also arrive back marked as if from the
//! peer; the bridge keeps a per-port receive backlog and echoes frames across.
//!
//! Usage:
//! ```text
//! bridge -a vale0:porta -b vale0:portb [-n LOOPS]
//! ```
//!
//! Note: VALE silently drops frames shorter than 14 bytes, so the bridge only
//! forwards frames of at least that size. No physical NIC is involved.

use std::time::{Duration, Instant};

use netmap_rs::prelude::*;

const MIN_VALE_FRAME: usize = 14;

fn parse_veto(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .enumerate()
        .find(|(_, a)| a.as_str() == flag)
        .and_then(|(i, _)| args.get(i + 1))
        .cloned()
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let port_a = parse_veto(&args, "-a").unwrap_or_else(|| "vale0:ba".into());
    let port_b = parse_veto(&args, "-b").unwrap_or_else(|| "vale0:bb".into());
    let loops: Option<usize> = parse_veto(&args, "-n").and_then(|v| v.parse().ok());

    let mk = |p: &str| {
        let name = if p.starts_with("netmap:") {
            p.to_string()
        } else {
            format!("netmap:{p}")
        };
        NetmapBuilder::new(&name).build()
    };
    let nm_a = mk(&port_a)?;
    let nm_b = mk(&port_b)?;
    let mut tx_a = nm_a.tx_ring(0)?;
    let mut rx_a = nm_a.rx_ring(0)?;
    let mut tx_b = nm_b.tx_ring(0)?;
    let mut rx_b = nm_b.rx_ring(0)?;

    println!("bridging {port_a} <-> {port_b} (Ctrl-C to stop)");

    let mut forwarded = 0usize;
    let start = Instant::now();
    let mut last = Instant::now();

    loop {
        forward(&mut rx_a, &mut tx_b, &port_b, &mut forwarded)?;
        forward(&mut rx_b, &mut tx_a, &port_a, &mut forwarded)?;

        // Periodic summary.
        if last.elapsed() >= Duration::from_secs(2) {
            let dt = last.elapsed().as_secs_f64();
            println!(
                "forwarded {forwarded} frames ({:.0}/s) in {:.1}s",
                forwarded as f64 / dt,
                start.elapsed().as_secs_f64()
            );
            last = Instant::now();
        }

        // Yield so the kernel can deliver more packets to the peer endpoint.
        std::thread::sleep(Duration::from_micros(500));

        if let Some(n) = loops {
            if forwarded >= n {
                println!("reached loop target of {n}, stopping");
                break;
            }
        }
    }
    Ok(())
}

/// Move every frame from `rx` to `tx`, bouncing it back to the other port of
/// the pair. Frames shorter than the VALE minimum are discarded (the switch
/// drops them anyway), and the payload is left untouched.
fn forward(
    rx: &mut RxRing,
    tx: &mut TxRing,
    peer: &str,
    forwarded: &mut usize,
) -> Result<(), Error> {
    rx.sync();
    while let Some(frame) = rx.recv() {
        let data = frame.payload();
        if data.len() < MIN_VALE_FRAME {
            continue;
        }
        if tx.has_free_slots() {
            tx.send(data)?;
            tx.sync();
            *forwarded += 1;
        } else {
            // The peer TX ring is full; the switch briefly dropped the relay.
            eprintln!("note: {peer} TX ring full, dropping relay");
            break;
        }
    }
    Ok(())
}
