//! `pkt-gen` — packet generator / drainer for VALE ports and netmap pipes.
//!
//! This is a Rust counterpart of the upstream netmap `pkt-gen` tool. It
//! operates exclusively on virtual ports (VALE `vale0:name` and pipes
//! `pipe{name` / `pipe}name`), so it never touches a live network interface.
//!
//! Usage:
//! ```text
//! pkt-gen -i vale0:portname -f tx [-n NUM_PACKETS] [-s SIZE] [-b BATCH]
//! pkt-gen -i vale0:portname -f rx               # drain forever
//! pkt-gen -i netmap:pipe{pkt -f tx -n 1000       # pipes need both endpoints
//! ```
//!
//! # Examples
//!
//! Open two ports on the default VALE switch and pump 10k frames of 64 bytes
//! from one to the other:
//! ```text
//! pkt-gen -i vale0:src -f tx -n 10000 -s 64 &
//! pkt-gen -i vale0:dst -f rx
//! ```
//!
//! Note: VALE enforces the 14-byte Ethernet minimum frame size; keep `-s`
//! at or above 14 bytes.

use std::env;
use std::time::{Duration, Instant};

use netmap_rs::prelude::*;

/// Print usage and exit.
fn usage(prog: &str) -> ! {
    eprintln!(
        "usage: {prog} -i <port> -f <tx|rx> [-n <packets>] [-s <size>] [-b <batch>]\n\
         \n\
         {prog} - packet generator/drainer for VALE ports and netmap pipes.\n\
         \n\
         options:\n\
         \x20 -i <port>    port to use, e.g. vale0:p0 or netmap:pipe{{name\n\
         \x20 -f <dir>     'tx' to generate packets, 'rx' to drain a port\n\
         \x20 -n <packets> number of packets to send (default: unlimited)\n\
         \x20 -s <size>    packet payload size in bytes (default: 64, min 14 for VALE)\n\
         \x20 -b <batch>   packets sent per TX sync (default: 64)"
    );
    std::process::exit(1);
}

/// Simple inline argument parser: returns `Some(value)` for `-flag`, or the
/// first free positional argument for `-flag=false`.
fn opt(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .enumerate()
        .find(|(_, a)| a.as_str() == key)
        .and_then(|(i, _)| args.get(i + 1))
        .cloned()
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        usage(&env::args().next().unwrap_or_else(|| "pkt-gen".into()));
    }

    let ifname = opt(&args, "-i").unwrap_or_else(|| usage("pkt-gen"));
    // Accept both "vale0:p0" (like the C pkt-gen) and "netmap:vale0:p0".
    let netmap_name = if ifname.starts_with("netmap:") {
        ifname.clone()
    } else {
        format!("netmap:{}", ifname)
    };
    let dir = opt(&args, "-f").unwrap_or_else(|| usage("pkt-gen"));
    let num_packets: Option<usize> = opt(&args, "-n").and_then(|v| v.parse().ok());
    let size: usize = opt(&args, "-s").and_then(|v| v.parse().ok()).unwrap_or(64);
    let batch: usize = opt(&args, "-b").and_then(|v| v.parse().ok()).unwrap_or(64);

    // Open the port. Pipes default to a single TX/RX ring each.
    let nm = NetmapBuilder::new(&netmap_name).build()?;
    println!(
        "{}: opened {} ({} TX rings, {} RX rings, {} slots/ring)",
        dir,
        netmap_name,
        nm.num_tx_rings(),
        nm.num_rx_rings(),
        nm.tx_ring(0).map(|r| r.num_slots()).unwrap_or(0)
    );

    match dir.as_str() {
        "tx" => generate(nm, size, batch, num_packets),
        "rx" => drain(nm),
        other => {
            eprintln!("unknown direction '{other}'; use 'tx' or 'rx'");
            usage("pkt-gen");
        }
    }
}

/// Fill the TX ring with `size`-byte payloads and sync each `batch` packets,
/// until `num_packets` have been sent (or forever when `None`).
fn generate(
    nm: Netmap,
    size: usize,
    batch: usize,
    num_packets: Option<usize>,
) -> Result<(), Error> {
    let mut tx = nm.tx_ring(0)?;
    if size > tx.max_payload_size() {
        return Err(Error::PacketTooLarge(size));
    }
    if size < 14 {
        // VALE silently drops frames shorter than the Ethernet minimum.
        println!("note: frames shorter than 14 bytes are dropped by VALE; padding to 14");
    }

    let fmt = |n: usize| {
        let b = n * size;
        if b >= 1024 * 1024 * 1024 {
            format!("{:.1} GiB", b as f64 / 1024.0 / 1024.0 / 1024.0)
        } else if b >= 1024 * 1024 {
            format!("{:.1} MiB", b as f64 / 1024.0 / 1024.0)
        } else if b >= 1024 {
            format!("{:.1} KiB", b as f64 / 1024.0)
        } else {
            format!("{b} B")
        }
    };

    println!(
        "sending up to {} frames of {} bytes in batches of {}...",
        num_packets
            .map(|n| n.to_string())
            .unwrap_or_else(|| "unlimited".into()),
        size,
        batch
    );

    let mut sent: usize = 0;
    let start = Instant::now();
    let mut last_report = Instant::now();
    let mut last_sent = 0usize;

    loop {
        if tx.has_free_slots() {
            // Send one batch. Fill each reserved slot with a distinct payload
            // so receivers can verify ordering/counting.
            let count = batch.min(tx.num_slots() - 1);
            let mut reservation = tx.reserve_batch(count)?;
            for i in 0..count {
                let seq = sent + i;
                let mut payload = vec![0u8; size.max(14)];
                payload.write_seq(seq, size);
                reservation
                    .packet(i, payload.len())?
                    .copy_from_slice(&payload);
                sent += 1;
            }
            reservation.commit();
            tx.sync();
        }

        if let Some(n) = num_packets {
            if sent >= n {
                break;
            }
        }

        // Report throughput every second, mirroring pkt-gen's live counter.
        if last_report.elapsed() >= Duration::from_secs(1) {
            let dt = last_report.elapsed().as_secs_f64();
            let ds = sent - last_sent;
            println!(
                "sent {} frames ({:.0}/s, {})",
                sent,
                ds as f64 / dt,
                fmt(ds)
            );
            last_report = Instant::now();
            last_sent = sent;
        }
    }

    let dt = start.elapsed().as_secs_f64();
    println!(
        "sent {sent} frames ({}/s, {}) in {:.2}s",
        sent as f64 / dt,
        fmt(sent),
        dt
    );
    Ok(())
}

/// Drain the RX ring until interrupted, printing per-second counters and the
/// sequence numbers seen (to expose drops/duplicates).
fn drain(nm: Netmap) -> Result<(), Error> {
    let mut rx = nm.rx_ring(0)?;
    println!("draining port (Ctrl-C to stop)...");

    let mut received: usize = 0;
    let mut last_report = Instant::now();
    let mut last_received = 0usize;
    let mut seq_ok = true;
    let mut last_seq: Option<usize> = None;

    loop {
        rx.sync();
        while let Some(frame) = rx.recv() {
            received += 1;
            let data = frame.payload();
            if !seq_ok {
                continue;
            }
            match data.read_seq(data.len()) {
                Some(seq) => {
                    if let Some(prev) = last_seq {
                        if seq != prev + 1 {
                            seq_ok = false;
                            eprintln!(
                                "sequence gap at frame #{received}: expected {}, got {seq}",
                                prev + 1
                            );
                        }
                    }
                    last_seq = Some(seq);
                }
                None => seq_ok = false, // non pkt-gen payload; stop verifying
            }
        }

        if last_report.elapsed() >= Duration::from_secs(1) {
            let dt = last_report.elapsed().as_secs_f64();
            let dr = received - last_received;
            println!(
                "received {} frames ({:.0}/s), seq ok: {}",
                received,
                dr as f64 / dt,
                seq_ok
            );
            last_report = Instant::now();
            last_received = received;
        }

        // Give the kernel a chance to deliver more packets.
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Slice extension helpers to embed/read a sequence number in a payload. The
/// number is stored little-endian in the first 8 bytes; the rest is padding.
trait PayloadSeq {
    fn write_seq(&mut self, seq: usize, size: usize);
    fn read_seq(&self, len: usize) -> Option<usize>;
}

impl PayloadSeq for [u8] {
    fn write_seq(&mut self, seq: usize, size: usize) {
        // Write seq in the first 8 bytes, pad with a recognizable byte pattern.
        for (i, b) in seq.to_le_bytes().iter().enumerate() {
            self[i] = *b;
        }
        for b in self.iter_mut().skip(8).take(size.saturating_sub(8)) {
            *b = 0x5a;
        }
    }

    fn read_seq(&self, len: usize) -> Option<usize> {
        if len < 8 {
            return None;
        }
        Some(usize::from_le_bytes(self[..8].try_into().ok()?))
    }
}
