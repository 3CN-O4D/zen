# nmap — Pentest-grade network scanner (written in Zen)

The `nmap` module is a full-featured network scanner implemented **in Zen**
on top of the raw `scapy` primitives. It performs host discovery, SYN and
connect scanning, service/version detection, and OS fingerprinting.

Raw-socket scans (SYN, ARP, UDP, handshake) require **root**:

```bash
sudo zen script.z
```

Set the target network interface (`nmap.IFACE`) for non-default-route nets:

```zen
import nmap
nmap.IFACE = "zt6jy3cwak"        # ZeroTier / VPN interface
```

## Host discovery

| Function | Description |
| --- | --- |
| `nmap.arp_discover(cidr, iface)` | ARP who-has sweep over a subnet; returns `[{ip, mac}, ...]` |

```zen
var hosts = nmap.arp_discover("192.168.193.0/24", "zt6jy3cwak")
for h in hosts { print h.ip, h.mac }
```

## Port scanning

| Function | Description |
| --- | --- |
| `nmap.syn_scan(host, ports, iface)` | SYN (half-open) scan; returns `{host, open, closed, filtered, state}` |
| `nmap.connect_scan(host, ports, iface)` | Full TCP connect scan (works without root) |
| `nmap.scan(host, ports, type, iface)` | Dispatch: `"syn"` or `"connect"` |
| `nmap.top_ports(host, n, iface)` | Scan the top-n most common ports quickly |
| `nmap.quick_scan(host, iface)` | Top 100 common ports |
| `nmap.full_scan(host, iface)` | Scan all 65535 ports |

Port spec accepts ranges and lists: `"1-1024"`, `"22,80,443"`, `"1-1000,8080"`.

```zen
var r = nmap.syn_scan("192.168.193.170", "1-1000", "zt6jy3cwak")
print "open ports:", r.open
print "state:", r.state
```

## Service / version detection (~ -sV)

`service()` probes each open port with a protocol-appropriate request and
extracts `product` + `version`. Handshakes and banners are read over a normal
TCP.stream. The `_proto_hint()` table routes the right probe by port.

| Function | Description |
| --- | --- |
| `nmap.service(host, port, iface)` | Probe one port; returns `{port, state, service, product, version, banner}` |
| `nmap.version_scan(host, open_ports, iface)` | Run `service()` across a list of open ports |

Supported fingerprints: SSH (OpenSSH + version), HTTP (Server header →
nginx/Apache + version), SMTP (Postfix/Exim), FTP (vsftpd/ProFTPD/Pure-FTPd),
POP3/IMAP (Dovecot/Courier), MySQL, Redis, NNTP, Telnet.

```zen
var s = nmap.service("192.168.193.170", 22, "zt6jy3cwak")
print s.service, s.product, s.version   # ssh OpenSSH 8.9p1
```

## OS detection (~ -O)

`os_detect()` crafts SYN probes (via scappy `sr1`) and fingerprints the OS
using TTL distance, TCP window size, and service hints.

| Function | Description |
| --- | --- |
| `nmap.os_detect(host, iface)` | Fingerprint OS; returns `{host, osfamily, ttl, hints, evidence}` |

Recognizes Linux/Unix (TTL 64), Windows (TTL 128), network devices (TTL 255),
refined by local service hints.

```zen
var o = nmap.os_detect("192.168.193.170", "zt6jy3cwak")
print o.osfamily, o.ttl      # linux 64
```

## UDP scan (~ -sU)

Sends zero-length UDP probes and watches for ICMP port-unreachable (type 3
code 3) to mark closed ports, and UDP replies to mark open ports. Unanswered
ports are `open|filtered`. Results are best-effort (UDP is rate-limited and
unreliable, same as real nmap).

| Function | Description |
| --- | --- |
| `nmap.udp_scan(host, ports, iface)` | Returns `{host, open, closed, open_filtered, state}` |

```zen
var u = nmap.udp_scan("192.168.193.170", "53,161,9993", "zt6jy3cwak")
print u.open, u.closed, u.open_filtered
```

## NSE-style scripts (~ --script)

| Function | Description |
| --- | --- |
| `nmap.script_ssh_enum(host, port, iface)` | SSH version → known-CVE + user-enum notes |
| `nmap.script_smtp_userenum(host, port, users, iface)` | VRFY/EXPN user enumeration |
| `nmap.script_redis_unauth(host, port, iface)` | Redis unauthenticated INFO access |
| `nmap.script_ftp_anon(host, port, iface)` | FTP anonymous login check |
| `nmap.script_http_headers(host, port, iface)` | Server header + missing security headers |
| `nmap.script_mysql_empty(host, port, users, iface)` | MySQL handshake/greeting check |
| `nmap.scripts(host, open_ports, iface)` | Auto-run appropriate scripts against open ports |

```zen
var s = nmap.script_ssh_enum("192.168.193.170", 22, "zt6jy3cwak")
print s.vulnerable, s.cve, s.notes

var h = nmap.script_http_headers("192.168.193.162", 8080, "zt6jy3cwak")
print h.server, h.title, h.missing
```

## Target specification

`nmap._parse_targets(spec)` expands an nmap-style target string into a flat
host list. Supports single IPs, comma lists, CIDR (`/xx`), octet ranges
(`192.168.1.1-20`), space-separated multi-target, and `--exclude`.

```zen
nmap._parse_targets("192.168.193.0/29 --exclude 192.168.193.1,192.168.193.3")
nmap._parse_targets("10.0.0.1,10.0.0.5")
nmap._parse_targets("192.168.1.1-20")
```

## Configuration globals

| Variable | Default | Description |
| --- | --- | --- |
| `nmap.IFACE` | `""` | Network interface for raw scans (autodetect if empty) |
| `nmap.TIMING` | `3` | 0..5 timing template (paranoid→insane) |
| `nmap.SRC_PORT` | `40000` | Base source port for raw scanned connections |

## Reference behavior

On the example ZeroTier subnet `192.168.193.0/24` these results matched the
real `nmap` binary exactly:

| Target | Real nmap open ports | Zen nmap open ports |
| --- | --- | --- |
| 192.168.193.170 | 22 (ssh) | 22, service `ssh` (OpenSSH banner), OS `linux` |
| 192.168.193.162 | 111, 8080 | 111, 8080 (connect scan) |
| 192.168.193.106 | — | none in scanned range |
