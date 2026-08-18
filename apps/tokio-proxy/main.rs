//! `tokio-proxy` — asynchronous frame relay over a netmap pipe using Tokio.
//!
//! Opens the two endpoints of a netmap pipe and runs a Tokio task per
//! direction, relaying any bytes read from one endpoint's RX ring into the
//! other endpoint's TX ring. This demonstrates integrating `netmap-rs` with
//! `tokio::time` (instead of busy-polling) on the purely virtual pipe device.
//!
//! Usage:
//! ```text
//! tokio-proxy [-n PIPE_NUMBER]
//! ```
//!
//! Requires the `tokio-async` feature. Only the in-memory pipe device is
//! touched — no physical NIC or host network stack is involved.

use std::time::Duration;

use netmap_rs::tokio_async::{AsyncNetmapRxRing, AsyncNetmapTxRing, TokioNetmap};
use netmap_rs::NetmapBuilder;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pipe = args
        .iter()
        .enumerate()
        .find(|(_, a)| a.as_str() == "-n")
        .and_then(|(i, _)| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("5");
    // Unique pipe id so several runs (or other apps) do not collide.
    let master = format!("netmap:pipe{{{pipe}");
    let slave = format!("netmap:pipe}}{pipe}");

    // Open both endpoints of the pipe.
    let nm_master = NetmapBuilder::new(&master).build()?;
    let nm_slave = NetmapBuilder::new(&slave).build()?;

    // Wrap them for async use.
    let tokio_master = TokioNetmap::new(nm_master)?;
    let tokio_slave = TokioNetmap::new(nm_slave)?;

    let mut rx_master = tokio_master.rx_ring(0)?;
    let mut tx_master = tokio_master.tx_ring(0)?;
    let mut rx_slave = tokio_slave.rx_ring(0)?;
    let mut tx_slave = tokio_slave.tx_ring(0)?;

    println!("relaying {master} -> {slave} and {slave} -> {master}");

    // Relay master->slave and slave->master concurrently.
    tokio::try_join!(
        relay(&mut rx_master, &mut tx_slave, "master->slave"),
        relay(&mut rx_slave, &mut tx_master, "slave->master")
    )?;

    Ok(())
}

/// Read one frame from `rx` and write it to `tx`, looping forever. The
/// `AsyncRead`/`AsyncWrite` impls trigger the netmap RX/TX sync ioctls
/// (`NIOCRXSYNC`/`NIOCTXSYNC`) on each poll.
async fn relay(
    rx: &mut AsyncNetmapRxRing,
    tx: &mut AsyncNetmapTxRing,
    name: &'static str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buf = vec![0u8; 2048];
    loop {
        // AsyncReadExt::read on AsyncNetmapRxRing keeps the task registered
        // for wakeup while no packet is available (it returns Pending).
        let n = rx.read(&mut buf).await?;
        if n == 0 {
            // Zero-length netmap reads mean the slot was empty; skip.
            tokio::time::sleep(Duration::from_millis(1)).await;
            continue;
        }
        tx.write_all(&buf[..n]).await?;
        tx.flush().await?;
        println!("{name}: relayed {n} bytes");
    }
}
