# scapy — Raw packet crafting and network scanning primitives

The `scapy` module provides low-level raw sockets for packet crafting,
sending, sniffer/receiving, and high-performance scanning primitives used by
the `nmap` module. Raw socket operations (send/sniff/SYN scan/ARP) require
**root**: run scripts with `sudo zen <script>`.

## Auto-detected values

| Function | Description |
| --- | --- |
| `scapy.src_ip([iface])` | Local IPv4 address (optionally for a given interface) |
| `scapy.src_mac([iface])` | Local MAC address (optionally for a given interface) |
| `scapy.hostname("host")` | Resolve a hostname to an IP address |

## Packet building

| Function | Description |
| --- | --- |
| `scapy.ip(src, dst, proto)` | Build an IP layer dict (`proto`: `"TCP"`, `"UDP"`, `"ICMP"` or number) |
| `scapy.tcp(sport, dport[, ...])` | Build a TCP layer dict. Set fields directly, e.g. `t.flags="S"`, `t.seq=...` |
| `scapy.udp(sport, dport[, ...])` | Build a UDP layer dict |
| `scapy.icmp(type, code[, ...])` | Build an ICMP layer dict |
| `scapy.raw(data)` | Build a Raw payload layer |
| `scapy.ether([dst, src, type])` | Build an Ethernet layer dict (`"IP"`/`"ARP"`/number) |
| `scapy.arp(psrc, pdst, op[, hwsrc, hwdst])` | Build an ARP layer dict (`op`: 1=request, 2=reply) |
| `scapy.build(layer)` | Serialize a layer dict (with nested `payload`) into a byte list |
| `scapy.parse(bytes)` | Parse an IP packet (TCP/UDP/ICMP) into layer dicts |
| `scapy.checksum(data)` | Compute an Internet checksum |
| `scapy.ip_to_int(ip)` / `scapy.int_to_ip(n)` | IPv4 <-> integer conversion |
| `scapy.cidr_expand("192.168.1.0/24")` | Expand a CIDR to a list of IP strings |
| `scapy.subnet_hosts(network, netmask)` | Enumerate usable hosts in a subnet |

### Building a packet (SYN example)

```zen
var ip  = scapy.ip("192.168.1.10", "192.168.1.1", "TCP")
var tcp = scapy.tcp(40000, 80)
tcp.flags = "S"
tcp.seq = 100000
var pkt   = {type: "IP", src: "192.168.1.10", dst: "192.168.1.1", proto: "TCP", payload: tcp}
var bytes = scapy.build(pkt)     # 40-byte SYN segment
scapy.send(pkt)                  # send it raw (root)
```

## Sending / sniffing

| Function | Description | Root |
| --- | --- | --- |
| `scapy.send(pkt[, iface])` | Send an IP packet via raw socket (bind to `iface` if given) | yes |
| `scapy.sendp(psrc, pdst[, iface])` | Send an Ethernet+ARP request frame | yes |
| `scapy.sniff([count, timeout_sec])` | Sniff N IPv4 packets up to a timeout, returns parsed list | yes |
| `scapy.sr1(pkt[, timeout_ms, iface])` | Send an IP packet, return first matching reply | yes |
| `scapy.srp1(psrc, pdst[, timeout_ms, iface])` | Send ARP request, return the reply (`{ip, mac}`) | yes |

**Parsed packet fields.** `scapy.sniff` and `scapy.sr1` return parsed packet
dicts. IP layer exposes `src`, `dst`, `ttl`; TCP exposes `sport`, `dport`,
`seq`, `ack`, `flags`, `window`, and (when present) `options` (e.g.
`mss=...`, `wscale=...`, `sack-ok`, `timestamp=...`). ICMP errors expose
`icmp_type`, `icmp_code`, and the embedded original-port fields
`orig_udp_dport` / `orig_tcp_dport` (used for UDP scanning).


## High-performance scanning primitives

These are native, packet-level scanners used by `nmap`.

| Function | Description | Root |
| --- | --- | --- |
| `scapy.syn_scan(host, ports, timeout_ms, sport, iface)` | Full-batch SYN scan. Returns `{open, closed, filtered}` port lists | yes |
| `scapy.handshake(host, port, sport, timeout_ms, iface)` | Do a full TCP 3-way handshake (SYN→SYN-ACK→ACK). Returns `{ok, seq}` | yes |
| `scapy.arp_scan(cidr, timeout_ms, iface)` | ARP host discovery over a subnet. Returns `[{ip, mac}, ...]` | yes |

### Notes

- `syn_scan` is fast (a full 65535-port scan of a LAN host completes in seconds)
  and classifies `open` (SYN-ACK), `closed` (RST), and `filtered` (no reply).
- `handshake` verifies a port is truly open AND performs the full TCP
  connection, useful for service probing.
- `arp_scan` is the reliable LAN host-discovery method (equivalent to
  nmap's ARP ping sweep / `-sn` on a local subnet).
- Choose the interface explicitly (e.g. `"zt6jy3cwak"`) when targets are on a
  non-default-route network, so crafted packets egress the correct device.
