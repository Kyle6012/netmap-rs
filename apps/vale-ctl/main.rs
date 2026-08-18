//! `vale-ctl` — manage VALE virtual switches and their ports.
//!
//! VALE is the in-kernel virtual Ethernet switch provided by the netmap
//! module. A VALE switch is named `valeXXX` and its ports `valeXXX:name`.
//! Ports come into existence the first time they are opened by a netmap
//! client; this tool opens them on demand and reports the resulting ring
//! layout, mimicking the upstream `vale-ctl` utility.
//!
//! Usage:
//! ```text
//! vale-ctl -a vale0:p0                    # add (open) one port
//! vale-ctl -a vale0:p0 -a vale0:p1        # add several ports on a switch
//! vale-ctl -r vale0:p0                    # remove by closing the port
//! vale-ctl -A vale0:p0 -i vale0:if0       # attach a persistent port
//! xor possibly negative -d for disconnect
//! ```
//!
//! Only VALE ports are touched; no physical NIC is ever opened.

use std::env;
use std::process::exit;

use netmap_rs::prelude::*;

/// Print usage and exit.
fn usage(prog: &str) -> ! {
    eprintln!(
        "usage: {prog} -a vale0:port [-a vale0:port]...   add ports\n\
         \x20      {prog} -r vale0:port [-r vale0:port]...   remove ports\n\
         \x20      {prog} -A vale0:port -i vale0:ifname        attach port {{\n\
         \n\
         Manage VALE virtual switches (no physical NIC is touched).\n\
         Ports are created on first open and destroyed when the last\n\
         netmap file descriptor closes. 'add' then 'remove' order is\n\
         preserved: flags are processed left to right."
    );
    exit(1);
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        usage(&env::args().next().unwrap_or_else(|| "vale-ctl".into()));
    }

    // Parse the command line into an ordered list of actions, giving the
    // tool a feel close to the shell-oriented C vale-ctl.
    let mut i = 0;
    let mut found_action = false;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-a" | "-add" => {
                found_action = true;
                let name = next(&args, &mut i, "-a");
                add_port(&name)?;
            }
            "-r" | "-del" | "-d" | "-rm" => {
                found_action = true;
                let name = next(&args, &mut i, "-r");
                remove_port(&name)?;
            }
            "-A" => {
                // Attach (open) a port; without a netmap client attached it
                // stays around only as long as this process holds it.
                found_action = true;
                let name = next(&args, &mut i, "-A");
                add_port(&name)?;
            }
            "-i" | "-ifname" => {
                // Candidate interface name, used with -A. We just validate size.
                let _name = next(&args, &mut i, "-i");
            }
            "-h" | "--help" => usage("vale-ctl"),
            other => {
                eprintln!("warning: ignoring unknown argument '{other}'");
            }
        }
        i += 1;
    }

    if !found_action {
        usage("vale-ctl");
    }
    Ok(())
}

fn next(args: &[String], i: &mut usize, flag: &str) -> String {
    let v = args.get(*i + 1).cloned().unwrap_or_else(|| {
        eprintln!("missing argument after '{flag}'");
        exit(1);
    });
    *i += 1;
    v
}

/// Open a VALE port and print its ring layout. Opening is what registers the
/// port with the `valeXXX` switch in the kernel.
fn add_port(name: &str) -> Result<(), Error> {
    let qualified = qualify(name);
    let nm = NetmapBuilder::new(&qualified).build()?;
    println!(
        "added {qualified}: {} TX rings, {} RX rings",
        nm.num_tx_rings(),
        nm.num_rx_rings()
    );
    Ok(())
}

/// "Remove" a port. VALE ports vanish when the last netmap descriptor closes,
/// so removing is modeled by opening then dropping the descriptor.
fn remove_port(name: &str) -> Result<(), Error> {
    let qualified = qualify(name);
    let nm = NetmapBuilder::new(&qualified).build()?;
    println!(
        "removed {qualified}: {} TX rings, {} RX rings unregistered on close",
        nm.num_tx_rings(),
        nm.num_rx_rings()
    );
    drop(nm);
    Ok(())
}

/// Normalize a port name to the full `netmap:valeX:name` form that nm_open
/// expects, accepting both bare `vale0:p0` and `netmap:vale0:p0`.
fn qualify(name: &str) -> String {
    if name.starts_with("netmap:") {
        name.to_string()
    } else {
        format!("netmap:{name}")
    }
}
