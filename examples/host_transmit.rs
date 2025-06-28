//! Netmap Host Stack Transmit Example
//!
//! This example demonstrates how to transmit packets into the host stack
//! associated with a physical network interface using Netmap.
//!
//! It opens the specified interface with the `^` suffix (e.g., "netmap:eth0^")
//! to access its host stack rings. It then constructs a simple UDP packet
//! and sends it via the first host TX ring.
//!
//! Usage:
//! cargo run --example host_transmit --features sys -- <interface_name_with_caret> <src_ip> <dst_ip> <dst_port>
//!
//! Example:
//! cargo run --example host_transmit --features sys -- netmap:eth0^ 192.168.1.100 192.168.1.1 5000
//!
//! (Replace `eth0` with your actual network interface and IPs/port accordingly)
//!
//! While this example is running, you can use `tcpdump` on the host to observe
//! the injected packet, e.g.:
//! `sudo tcpdump -i eth0 -n udp port 5000`
//! Or run a UDP server listening on the specified destination IP and port.

use std::env;
use std::error::Error;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::time::Duration;

use netmap_rs::prelude::*;

// For building Ethernet, IP, UDP headers.
// A full-fledged packet builder library would be better for complex packets.
const ETH_HDR_LEN: usize = 14;
const IPV4_HDR_LEN: usize = 20;
const UDP_HDR_LEN: usize = 8;

// Simple checksum calculation
fn ip_checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut i = 0;
    while i < data.len() {
        let word = if i + 1 < data.len() {
            (data[i] as u16) << 8 | (data[i+1] as u16)
        } else {
            (data[i] as u16) << 8
        };
        sum += word as u32;
        i += 2;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !sum as u16
}


fn build_udp_packet(
    src_mac: [u8; 6],
    dst_mac: [u8; 6],
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Vec<u8> {
    let mut packet = Vec::with_capacity(ETH_HDR_LEN + IPV4_HDR_LEN + UDP_HDR_LEN + payload.len());

    // Ethernet Header
    packet.extend_from_slice(&dst_mac); // Destination MAC
    packet.extend_from_slice(&src_mac); // Source MAC
    packet.extend_from_slice(&[0x08, 0x00]); // EtherType: IPv4

    // IPv4 Header
    let ipv4_start = packet.len();
    packet.push(0x45); // Version (4) and IHL (5, *4 = 20 bytes)
    packet.push(0x00); // DSCP, ECN
    let total_len = (IPV4_HDR_LEN + UDP_HDR_LEN + payload.len()) as u16;
    packet.extend_from_slice(&total_len.to_be_bytes()); // Total Length
    packet.extend_from_slice(&[0x00, 0x01]); // Identification (dummy)
    packet.extend_from_slice(&[0x00, 0x00]); // Flags (0), Fragment Offset (0)
    packet.push(64); // TTL
    packet.push(17); // Protocol: UDP (17)
    packet.extend_from_slice(&[0x00, 0x00]); // Header Checksum (placeholder)
    packet.extend_from_slice(&src_ip.octets()); // Source IP
    packet.extend_from_slice(&dst_ip.octets()); // Destination IP

    // Calculate IPv4 Checksum
    let ipv4_header_slice = &packet[ipv4_start..ipv4_start + IPV4_HDR_LEN];
    let checksum = ip_checksum(ipv4_header_slice);
    packet[ipv4_start + 10..ipv4_start + 12].copy_from_slice(&checksum.to_be_bytes());

    // UDP Header
    // let udp_start = packet.len();
    packet.extend_from_slice(&src_port.to_be_bytes()); // Source Port
    packet.extend_from_slice(&dst_port.to_be_bytes()); // Destination Port
    let udp_len = (UDP_HDR_LEN + payload.len()) as u16;
    packet.extend_from_slice(&udp_len.to_be_bytes()); // Length
    packet.extend_from_slice(&[0x00, 0x00]); // Checksum (optional for IPv4, 0 means no checksum)
                                            // Calculating UDP checksum requires pseudo-header, skipping for simplicity.

    // Payload
    packet.extend_from_slice(payload);

    packet
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 5 {
        eprintln!(
            "Usage: {} <interface_name_with_caret> <src_ip> <dst_ip> <dst_port>",
            args[0]
        );
        eprintln!("Example: {} netmap:eth0^ 192.168.1.100 192.168.1.1 5000", args[0]);
        return Err("Invalid arguments".into());
    }

    let if_name = &args[1];
    if !if_name.contains('^') {
         return Err("Invalid arguments: Interface name must include '^' suffix for host stack.".into());
    }
    let src_ip = Ipv4Addr::from_str(&args[2])?;
    let dst_ip = Ipv4Addr::from_str(&args[3])?;
    let dst_port = args[4].parse::<u16>()?;
    let src_port: u16 = 12345; // Arbitrary source port

    println!(
        "Attempting to open host stack rings for interface: {}",
        if_name
    );

    let nm_desc = NetmapBuilder::new(if_name)
        .num_tx_rings(1) // Request 1 host TX ring
        .num_rx_rings(0) // Not using RX
        .build()?;

    println!(
        "Successfully opened interface {}. Number of host TX rings available: {}",
        if_name,
        nm_desc.num_tx_rings()
    );

    if nm_desc.num_tx_rings() == 0 {
        eprintln!("No host TX rings available for interface {}. Exiting.", if_name);
        return Ok(());
    }

    let mut tx_ring = nm_desc.tx_ring(0)?;
    println!("Preparing to send packet to host TX ring 0...");

    // Dummy MAC addresses. For host stack injection, the kernel handles L2 framing
    // if the IP packet is routed correctly. However, netmap expects full frames.
    // For injecting into host stack, L2 might be implicit or require specific setup.
    // Let's provide dummy MACs; the host stack might ignore/replace them.
    // For a "real" scenario, one might need to query system for actual MACs or use a generic one.
    let src_mac: [u8; 6] = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x01];
    let dst_mac: [u8; 6] = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x02]; // Often broadcast/multicast or router MAC for external

    let payload = b"Hello from netmap-rs to host stack!";
    let packet = build_udp_packet(src_mac, dst_mac, src_ip, dst_ip, src_port, dst_port, payload);

    println!("Sending packet ({} bytes) to host stack: {:02X?}", packet.len(), &packet[..std::cmp::min(packet.len(), 48)]);

    match tx_ring.send(&packet) {
        Ok(_) => {
            tx_ring.sync(); // Ensure packet is processed
            println!("Packet sent successfully to host stack via Netmap.");
            println!("Try listening with: sudo tcpdump -i <base_if_name> -n udp port {} and host {}", dst_port, dst_ip);
        }
        Err(e) => {
            eprintln!("Failed to send packet: {:?}", e);
            return Err(Box::new(e));
        }
    }

    // Send a few packets
    for i in 0..3 {
        let dynamic_payload = format!("Hello #{} from netmap-rs to host stack!", i);
        let packet = build_udp_packet(src_mac, dst_mac, src_ip, dst_ip, src_port, dst_port, dynamic_payload.as_bytes());
        if tx_ring.send(&packet).is_ok() {
            println!("Sent dynamic packet #{}", i);
        } else {
            eprintln!("Failed to send dynamic packet #{}", i);
            break;
        }
        // Small delay between packets
        std::thread::sleep(Duration::from_millis(10));
    }
    tx_ring.sync();
    println!("Finished sending dynamic packets.");

    Ok(())
}
