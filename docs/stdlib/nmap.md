# nmap — network scanning

Port/service discovery helpers (raw sockets where the platform allows;
falls back to connect scans otherwise).

```zen
import nmap

print nmap.scan("127.0.0.1", "80,443", "connect", null)
```

## Functions

| Function | Description |
|----------|-------------|
| `scan(host, spec, type, iface)` | Port scan; `spec` like `"80,443"` or `"1-1024"`. |
| `syn_scan(host, spec, iface)` | SYN scan. |
| `connect_scan(host, spec, iface)` | TCP connect scan. |
| `top_ports(host, n, iface)` | Scan the top-`n` ports. |
| `service(host, port, iface)` | Banner grab and product guess. |
| `version_scan(host, open_ports, iface)` | Service versions for open ports. |
| `arp_discover(cidr, iface)` | ARP host discovery on a subnet. |

Port specs also accept ranges (`"8000-8100"`); helpers expand CIDRs and
byte orders internally.
