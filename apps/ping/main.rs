//! `ping` — round-trip latency probe between two netmap pipe endpoints.
//!
//! Opens both ends of a netmap pipe (or two VALE ports) in one process and
//! measures the round-trip time of one packet crossing the pipe, similar to
//! how ICMP ping measures end-to-end latency. Because pipes/VALE are
//! in-kernel, this measures the netmap round-trip cost rather than a network.
//!
//! Usage:
//! ```text
//! ping [-c COUNT] [-i INTERVAL_US] [pipe|vale]
//! ```
//!
//! Open a pipe with `netmap:pipe{pingNN` as master and `pipe}pingNN` as the
//! peer, send MAX samples, and print a summary.

use std::time::{Duration, Instant};

use netmap_rs::prelude::*;

const DEFAULT_PIPE_MASTER: &str = "netmap:pipe{6";
const DEFAULT_PIPE_SLAVE: &str = "netmap:pipe}6";

fn parse_u(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .enumerate()
        .find(|(_, a)| a.as_str() == flag)
        .and_then(|(i, _)| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let count = parse_u(&args, "-c", 5);
    let interval_us = parse_u(&args, "-i", 1000);

    // Open both ends of the pipe.
    let master = NetmapBuilder::new(DEFAULT_PIPE_MASTER).build()?;
    let slave = NetmapBuilder::new(DEFAULT_PIPE_SLAVE).build()?;
    let mut mt = master.tx_ring(0)?;
    let mut mr = master.rx_ring(0)?;
    let mut st = slave.tx_ring(0)?;
    let mut sr = slave.rx_ring(0)?;

    println!(
        "pinging {} <-> {} ({} samples, {} us interval)",
        DEFAULT_PIPE_MASTER, DEFAULT_PIPE_SLAVE, count, interval_us
    );

    let mut rtts: Vec<Duration> = Vec::new();

    for seq in 0..count {
        let sent_at = Instant::now();

        // Master -> slave. First byte holds the sequence number, the rest
        // is the ping payload.
        let payload = b"ping-rs-netmap-123";
        let mut buf = [0u8; 16];
        buf[0] = seq as u8;
        let plen = buf.len() - 1;
        buf[1..].copy_from_slice(&payload[..plen]);
        mt.send(&buf)?;
        mt.sync();

        // Receive on the slave side, then reply back.
        let echoed = recv_with_timeout(&mut sr, &buf, Duration::from_millis(200))?;
        debug_assert_eq!(&echoed[..], &buf[..]);
        st.send(&buf)?;
        st.sync();

        // Receive the echo on the master side and measure the RTT.
        let back = recv_with_timeout(&mut mr, &buf, Duration::from_millis(200))?;
        debug_assert_eq!(&back[..], &buf[..]);
        rtts.push(sent_at.elapsed());

        println!(
            "seq {}: rtt = {:.3} us",
            seq,
            sent_at.elapsed().as_micros() as f64 / 1000.0
        );

        std::thread::sleep(Duration::from_micros(interval_us as u64));
    }

    // Summary, ping(8)-style.
    let n = rtts.len();
    let sum: Duration = rtts.iter().sum();
    let min = rtts.iter().min().cloned().unwrap_or_default();
    let max = rtts.iter().max().cloned().unwrap_or_default();
    let avg = if n > 0 {
        sum / n as u32
    } else {
        Duration::ZERO
    };
    println!(
        "--- pipe ping summary ---\n{} packets transmitted, {} received, {:.1} us avg, {:.1} us min, {:.1} us max",
        count,
        n,
        avg.as_micros() as f64 / 1000.0,
        min.as_micros() as f64 / 1000.0,
        max.as_micros() as f64 / 1000.0
    );
    Ok(())
}

/// Block until a frame equal to `expected` arrives on `rx`, syncing in a
/// loop. Returns the frame payload on success.
fn recv_with_timeout(
    rx: &mut RxRing,
    expected: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, Error> {
    let deadline = Instant::now() + timeout;
    loop {
        rx.sync();
        if deadline < Instant::now() {
            break;
        }
        while let Some(frame) = rx.recv() {
            if frame.payload() == expected {
                return Ok(frame.payload().to_vec());
            }
        }
        std::thread::sleep(Duration::from_micros(100));
    }
    Err(Error::WouldBlock)
}
