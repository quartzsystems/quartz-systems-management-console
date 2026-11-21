// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suites 2, 3 and 12: DHCP on QEMU's user network; a seeded static
//! address on a VLAN; nothing listening on the network, and nothing
//! sent by the console but DHCP, DNS and NTP.

use std::sync::Arc;
use std::time::Duration;

use release::{fact, screen};

use super::{
    DHCP_ADDRESS, DHCP_GATEWAY, MAC, Run, booted, channel, fact_of, posted, screen_with, seed_disk,
    seed_toml,
};
use crate::qemu::session::Machine;

/// Suite 2: the address and the gateway QEMU's DHCP server gives are on
/// the status screen.
pub fn dhcp(run: &Run<'_>) -> Result<String, String> {
    let machine = Machine {
        nic: Some(MAC.to_string()),
        ..run.machine()?
    };
    let mut guest = booted(run, &machine)?;
    let deadline = run.deadline();
    let line = format!("{DHCP_ADDRESS} on eth0, gateway {DHCP_GATEWAY}");
    guest.seen(&posted(&refcode::ADDRESS_ACQUIRED, &line), deadline)?;
    screen_with(
        &mut guest,
        &[screen::row(screen::MANAGEMENT, &line)],
        deadline,
    )?;
    let status = channel(&mut guest)?.status()?;
    if fact_of(&status, fact::MANAGEMENT)? != line {
        return Err(format!(
            "the status's management line is `{}`",
            fact_of(&status, fact::MANAGEMENT)?
        ));
    }
    Ok(format!("`{line}` on the status screen and in the status"))
}

/// The static address the seed gives, its gateway and its VLAN.
const STATIC: &str = "192.0.2.10/24";
const STATIC_GATEWAY: &str = "192.0.2.1";
const VLAN: u16 = 100;

/// Suite 3: a seed with a static address on a VLAN is applied, and the
/// VLAN's interface exists.
pub fn static_vlan(run: &Run<'_>) -> Result<String, String> {
    let network = format!(
        "mode = \"static\"\naddress = \"{STATIC}\"\ngateway = \"{STATIC_GATEWAY}\"\nvlan = {VLAN}\n"
    );
    let mut machine = Machine {
        nic: Some(MAC.to_string()),
        ..run.machine()?
    };
    machine.disks.push(seed_disk(
        run,
        &seed_toml("name = \"mc-test\"\n", &network),
    )?);
    let mut guest = run.boot(&machine)?;
    let deadline = run.deadline();
    guest.expect(&posted(&refcode::SEED_APPLIED, ""), deadline)?;
    guest.expect(&posted(&refcode::STATE_LOADED, "host mc-test"), deadline)?;
    super::base_ready(&mut guest, deadline)?;
    let iface = format!("eth0.{VLAN}");
    let line = format!("{STATIC} on {iface}, gateway {STATIC_GATEWAY}");
    screen_with(
        &mut guest,
        &[
            screen::row(screen::MANAGEMENT, &line),
            screen::row(screen::HOST, "mc-test"),
        ],
        deadline,
    )?;
    let channel = guest.channel(deadline)?;
    let links = channel.ls("/sys/class/net")?;
    if !links.contains(&iface) {
        return Err(format!("no {iface} among the links {links:?}"));
    }
    let written = channel.text("/data/state/network.toml")?;
    if !written.contains(STATIC) || !written.contains(&format!("vlan = {VLAN}")) {
        return Err(format!("network.toml is not the seed's: {written}"));
    }
    Ok(format!(
        "seeded: `{line}`, {iface} exists, network.toml holds the seed's address"
    ))
}

/// What one frame the console sent was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sent {
    /// ARP.
    Arp,
    /// UDP to this port.
    Udp(u16),
    /// TCP to this port.
    Tcp(u16),
    /// `ICMPv6` of this type.
    Icmp6(u8),
    /// Anything else: its `EtherType` and protocol.
    Other(u16, u8),
}

/// Every frame of the pcap `bytes` sent from `mac`.
pub fn sent_from(bytes: &[u8], mac: [u8; 6]) -> Result<Vec<Sent>, String> {
    let header = bytes.get(..24).ok_or("the capture has no pcap header")?;
    if header[..4] != [0xd4, 0xc3, 0xb2, 0xa1] {
        return Err("the capture is not a little-endian pcap".into());
    }
    let mut out = Vec::new();
    let mut at = 24;
    while at + 16 <= bytes.len() {
        let len =
            u32::from_le_bytes([bytes[at + 8], bytes[at + 9], bytes[at + 10], bytes[at + 11]]);
        let len = usize::try_from(len).map_err(|e| e.to_string())?;
        let frame = bytes
            .get(at + 16..at + 16 + len)
            .ok_or("a frame runs past the capture")?;
        at += 16 + len;
        if frame.len() < 14 || frame[6..12] != mac {
            continue;
        }
        let kind = u16::from_be_bytes([frame[12], frame[13]]);
        let ip = &frame[14..];
        let port = |at: usize| {
            ip.get(at..at + 2)
                .map_or(0, |p| u16::from_be_bytes([p[0], p[1]]))
        };
        out.push(match kind {
            0x0806 => Sent::Arp,
            0x0800 => {
                let ihl = usize::from(ip.first().map_or(5, |b| b & 0x0f)) * 4;
                match ip.get(9) {
                    Some(17) => Sent::Udp(port(ihl + 2)),
                    Some(6) => Sent::Tcp(port(ihl + 2)),
                    p => Sent::Other(kind, p.copied().unwrap_or(0)),
                }
            }
            0x86dd => {
                let mut next = ip.get(6).copied().unwrap_or(0);
                let mut header = 40;
                // A hop-by-hop header, as MLD's reports have.
                if next == 0 {
                    next = ip.get(header).copied().unwrap_or(0);
                    header += (usize::from(ip.get(header + 1).copied().unwrap_or(0)) + 1) * 8;
                }
                match next {
                    58 => Sent::Icmp6(ip.get(header).copied().unwrap_or(0)),
                    17 => Sent::Udp(port(header + 2)),
                    6 => Sent::Tcp(port(header + 2)),
                    p => Sent::Other(kind, p),
                }
            }
            other => Sent::Other(other, 0),
        });
    }
    Ok(out)
}

/// Whether `sent` is DHCP, DNS or NTP to `ntp_port`, or what the link
/// itself needs: ARP, and IPv6's neighbour discovery and multicast
/// listener reports for the link-local address.
fn allowed(sent: &Sent, ntp_port: u16) -> bool {
    match sent {
        Sent::Arp => true,
        Sent::Udp(port) => [67, 53, 123, ntp_port].contains(port),
        // Neighbour solicitation and advertisement, MLDv1 and v2 reports.
        Sent::Icmp6(kind) => [135, 136, 131, 132, 143].contains(kind),
        Sent::Tcp(_) | Sent::Other(..) => false,
    }
}

/// The sockets of `/proc/net/<kind>` with their state and local port.
fn bound(text: &str) -> Vec<(u16, u8)> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let port = fields.get(1)?.rsplit_once(':')?.1;
            let state = fields.get(3)?;
            Some((
                u16::from_str_radix(port, 16).ok()?,
                u8::from_str_radix(state, 16).ok()?,
            ))
        })
        .collect()
}

/// The TCP state `LISTEN`.
const LISTEN: u8 = 0x0a;

/// The listening sockets of `/proc/net/tcp` (`v6` false) or `tcp6`:
/// each local address, as text, and port.
pub fn listening(text: &str, v6: bool) -> Vec<(String, u16)> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.get(3).and_then(|s| u8::from_str_radix(s, 16).ok()) != Some(LISTEN) {
                return None;
            }
            let (address, port) = fields.get(1)?.rsplit_once(':')?;
            let port = u16::from_str_radix(port, 16).ok()?;
            let address = if v6 {
                address.to_string()
            } else {
                // Four bytes of the address in the host's order.
                let n = u32::from_str_radix(address, 16).ok()?;
                std::net::Ipv4Addr::from(n.to_le_bytes()).to_string()
            };
            Some((address, port))
        })
        .collect()
}

/// Phase 3's rule for what listens: TCP 443 and 80 on the `open`
/// address `open` only, nothing on IPv6, nothing else.
pub fn only_web(tcp: &str, tcp6: &str, open: &str) -> Result<Vec<(String, u16)>, String> {
    if let Some((a, p)) = listening(tcp6, true).first() {
        return Err(format!("a TCP socket listens on IPv6: [{a}]:{p}"));
    }
    let on = listening(tcp, false);
    if let Some((a, p)) = on
        .iter()
        .find(|(a, p)| a != open || ![release::HTTPS_PORT, release::HTTP_PORT].contains(p))
    {
        return Err(format!("a TCP socket listens on {a}:{p}"));
    }
    Ok(on)
}

/// Suite 12: only the web server listens, on 443 and 80 of the `open`
/// address (from Phase 3), and the only traffic the console starts is
/// DHCP, DNS and NTP. A time server is configured
/// by address and another by name, so both DNS and NTP are seen.
pub fn sockets(run: &Run<'_>) -> Result<String, String> {
    let (server, responder) = super::time::responder(Arc::new(ntp::clock::ReadOnly))?;
    let port: u16 = server
        .rsplit_once(':')
        .and_then(|(_, p)| p.parse().ok())
        .ok_or("the responder's port")?;
    let host = format!("ntp_servers = [\"{server}\", \"time.invalid\"]\n");
    let dump = run.file("net.pcap");
    let _ = std::fs::remove_file(&dump);
    let mut machine = Machine {
        nic: Some(MAC.to_string()),
        dump: Some(dump.clone()),
        // A short path: the target directory's can pass SUN_LEN.
        monitor: Some(run.socket("monitor.sock")),
        ..run.machine()?
    };
    machine.disks.push(seed_disk(run, &seed_toml(&host, ""))?);
    let mut guest = booted(run, &machine)?;
    let deadline = run.deadline();
    guest.seen(&refcode::ADDRESS_ACQUIRED.tag(), deadline)?;
    guest.seen(&posted(&refcode::TIME_SYNCHRONIZED, &server), deadline)?;
    // A little longer, for whatever else the console would send.
    std::thread::sleep(Duration::from_secs(5));
    let channel = channel(&mut guest)?;
    let tcp = channel.text("/proc/net/tcp")?;
    let tcp6 = channel.text("/proc/net/tcp6")?;
    let mut udp = Vec::new();
    for kind in ["udp", "udp6"] {
        udp.extend(bound(&channel.text(&format!("/proc/net/{kind}"))?));
    }
    drop(guest);
    responder.stop();
    let open = DHCP_ADDRESS.split('/').next().unwrap_or(DHCP_ADDRESS);
    let web = only_web(&tcp, &tcp6, open)?;
    if web.len() != 2 {
        return Err(format!(
            "the web server listens on {web:?}, not on 443 and 80"
        ));
    }
    // The DHCP client's port, or a client's own port for one exchange
    // (the kernel's ephemeral range).
    if let Some((port, _)) = udp
        .iter()
        .find(|(port, _)| *port != 68 && !(32_768..61_000).contains(port))
    {
        return Err(format!("a UDP socket is bound to port {port}"));
    }
    let bytes = std::fs::read(&dump).map_err(|e| format!("{}: {e}", dump.display()))?;
    let mac = appliance::links::parse_mac(MAC).ok_or("the MAC")?;
    let sent = sent_from(&bytes, mac)?;
    if let Some(bad) = sent.iter().find(|s| !allowed(s, port)) {
        return Err(format!("the console sent {bad:?}"));
    }
    let count = |want: &dyn Fn(&Sent) -> bool| sent.iter().filter(|s| want(s)).count();
    let dhcp = count(&|s| *s == Sent::Udp(67));
    let dns = count(&|s| *s == Sent::Udp(53));
    let ntp = count(&|s| *s == Sent::Udp(port) || *s == Sent::Udp(123));
    if dhcp == 0 || dns == 0 || ntp == 0 {
        return Err(format!(
            "DHCP {dhcp}, DNS {dns}, NTP {ntp} frames: one of them was not seen"
        ));
    }
    Ok(format!(
        "TCP listens only on {} (the web server, on the open address), UDP bound only to \
         {:?}; of {} frames sent: DHCP {dhcp}, DNS {dns}, NTP {ntp}, and only ARP and IPv6 \
         neighbour discovery and listener reports besides",
        web.iter()
            .map(|(a, p)| format!("{a}:{p}"))
            .collect::<Vec<_>>()
            .join(" and "),
        udp.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
        sent.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sockets_of_proc_net_are_read() {
        let udp = "  sl  local_address rem_address   st tx_queue rx_queue\n\
                   0: 00000000:0044 00000000:0000 07 00000000:00000000 00:00000000\n";
        assert_eq!(bound(udp), [(68, 7)]);
        let tcp = "  sl  local_address rem_address   st\n 0: 0100007F:1F90 00000000:0000 0A 0\n";
        assert_eq!(bound(tcp), [(8080, LISTEN)]);
        let web = "  sl  local_address rem_address   st\n\
                   0: 0F02000A:01BB 00000000:0000 0A 0\n\
                   1: 0F02000A:0050 00000000:0000 0A 0\n";
        assert_eq!(
            only_web(web, "", "10.0.2.15").unwrap(),
            [
                ("10.0.2.15".to_string(), 443),
                ("10.0.2.15".to_string(), 80)
            ]
        );
        assert!(only_web(tcp, "", "127.0.0.1").is_err());
        assert!(only_web(web, "", "10.0.2.16").is_err());
    }

    #[test]
    fn a_frame_is_told_by_its_kind_and_port() {
        let mac = [0x52, 0x54, 0, 0x12, 0x34, 0x56];
        let mut pcap = vec![0xd4, 0xc3, 0xb2, 0xa1];
        pcap.resize(24, 0);
        let mut frame = vec![0xff; 6];
        frame.extend_from_slice(&mac);
        frame.extend_from_slice(&[0x08, 0x00, 0x45, 0, 0, 0, 0, 0, 0, 0, 0, 17]);
        frame.resize(14 + 20, 0);
        frame.extend_from_slice(&[0, 68, 0, 67]);
        let mut record = vec![0; 8];
        record.extend_from_slice(&u32::try_from(frame.len()).unwrap().to_le_bytes());
        record.extend_from_slice(&[0; 4]);
        pcap.extend_from_slice(&record);
        pcap.extend_from_slice(&frame);
        assert_eq!(sent_from(&pcap, mac).unwrap(), [Sent::Udp(67)]);
        assert_eq!(sent_from(&pcap, [0; 6]).unwrap(), []);
        assert!(allowed(&Sent::Udp(67), 1234));
        assert!(allowed(&Sent::Udp(1234), 1234));
        assert!(!allowed(&Sent::Tcp(443), 1234));
        assert!(!allowed(&Sent::Icmp6(133), 1234));
    }
}
