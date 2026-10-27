# Chapter 32: dns Module

The `dns` module provides DNS resolution: forward lookups (name → address), reverse lookups (address → name), and hostname information.

## 32.1 Resolve a hostname

```zen
import dns

let addrs = dns.resolve("example.com")
for addr in addrs {
    print(addr)
}
// 93.184.216.34
// 2606:2800:220:1:248:1893:25c8:1946
```

`dns.resolve(host, port?)` returns a list of socket addresses.

## 32.2 `gethostbyname` — single IPv4

```zen
import dns

let info = dns.gethostbyname("example.com")
print(info.hostname)   # example.com
print(info.addr_list)  # ["93.184.216.34", ...]
```

## 32.3 `gethostbyname_ex` — extended info

```zen
import dns

let info = dns.gethostbyname_ex("example.com")
print(info.hostname)      # example.com
print(info.alias_list)    # aliases
print(info.addr_list)     # addresses
```

## 32.4 `gethostbyaddr` — reverse lookup

```zen
import dns

let info = dns.gethostbyaddr("93.184.216.34")
print(info.hostname)   # example.com (or similar)
print(info.alias_list) # aliases
print(info.addr_list)  # addresses
```

## 32.5 `getfqdn` — fully qualified domain name

```zen
import dns

let fqdn = dns.getfqdn()
print(fqdn)  # myhost.localdomain

let fqdn2 = dns.getfqdn("example.com")
print(fqdn2) # example.com
```

## 32.6 Common patterns

### Check if a host resolves

```zen
import dns

func is_resolvable(host) {
    try {
        let addrs = dns.resolve(host)
        return len(addrs) > 0
    } catch {
        return false
    }
}

print(is_resolvable("example.com"))          # true
print(is_resolvable("nonexistent.invalid"))  # false
```

### Get the first IPv4 address

```zen
import dns

func first_ipv4(host) {
    let addrs = dns.resolve(host)
    for addr in addrs {
        if addr.contains(".") {  // IPv4 has dots
            return addr
        }
    }
    return nil
}

print(first_ipv4("example.com"))
```

### Resolve and connect

```zen
import dns
import socket

let addrs = dns.resolve("example.com", 443)
if len(addrs) > 0 {
    let addr = addrs[0]
    let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
    socket.connect(sock, addr, 443)
    // ... use socket ...
    socket.close(sock)
}
```

## 32.7 Quick reference

```zen
import dns

dns.resolve(host, port?)
dns.gethostbyname(host)
dns.gethostbyname_ex(host)
dns.gethostbyaddr(addr)
dns.getfqdn(host?)

let addrs = dns.resolve("example.com")
for addr in addrs { print(addr) }
```

## 32.8 Exercises

1. Write a function `resolve_all(host)` that prints all addresses for a host.
2. Write `is_reachable(host)` that tries DNS then a socket connect.
3. Write a script that resolves a list of hostnames and reports which fail.
4. Use `dns.getfqdn()` to print the local machine's FQDN.
5. Reverse-lookup an IP address and print the hostname.


## 32.12 DNS module — deep reference

DNS maps domain names to IP addresses (and vice versa).

### DNS record types

| Type | Meaning | Example |
|------|---------|---------|
| A | IPv4 address | example.com → 93.184.216.34 |
| AAAA | IPv6 address | example.com → 2606:2800:220:1:248:1893:25c8:1946 |
| CNAME | canonical name (alias) | www.example.com → example.com |
| MX | mail server | example.com → mail.example.com |
| TXT | arbitrary text | SPF, DKIM, verification |
| NS | name server | example.com → ns1.example.com |
| PTR | reverse (IP → name) | 93.184.216.34 → example.com |

### Resolution process

1. Check local /etc/hosts.
2. Check OS cache / DNS cache.
3. Query configured resolver (ISP, 1.1.1.1, 8.8.8.8).
4. Resolver queries root → TLD → authoritative servers.
5. Result returned and cached (TTL seconds).

### Usage

```zen
import dns
# forward lookup
let ips = dns.resolve("example.com")
print(ips)   # ["93.184.216.34", ...]

# specific record type
let mx = dns.resolve("example.com", "MX")
print(mx)

# reverse lookup
let names = dns.reverse("93.184.216.34")
print(names)
```

### Common DNS issues

| Issue | Cause | Fix |
|-------|-------|-----|
| NXDOMAIN | domain doesn't exist | check spelling |
| SERVFAIL | server failure | try another resolver |
| timeout | no response | check network/firewall |
| stale cache | old record cached | flush cache / lower TTL |

### Exercises

1. Resolve example.com to IP and print it.
2. Look up MX records for a domain.
3. Look up TXT records (SPF/DKIM).
4. Perform a reverse lookup on an IP.
5. Explain the difference between A and AAAA records.
6. Describe what happens when you type a URL in a browser.
7. Research public DNS resolvers (1.1.1.1, 8.8.8.8, 9.9.9.9).
8. Explain DNS caching and TTL.
9. Write a small tool resolving a list of domains.
10. Describe DNS hijacking / spoofing (conceptually, ethical).



## 32.12 Worked example — DNS resolution

```zen
import dns

func show(host) {
    print("A records for " + host + ":")
    let ips = dns.resolve(host, "A")
    for ip in ips { print("  " + ip) }

    print("MX records for " + host + ":")
    let mx = dns.resolve(host, "MX")
    for m in mx { print("  " + str(m)) }

    print("reverse for " + ips[0] + ":")
    let names = dns.reverse(ips[0])
    for n in names { print("  " + n) }
}

show("example.com")
```

Sample output:
```
A records for example.com:
  93.184.216.34
MX records for example.com:
  ...
reverse for 93.184.216.34:
  ...
```

## 32.13 Troubleshooting DNS

| Symptom | Cause | Fix |
|---------|-------|-----|
| NXDOMAIN | domain doesn't exist | check spelling |
| SERVFAIL | resolver failure | try another resolver (1.1.1.1) |
| timeout | no response | check network/firewall |
| stale data | cached old record | wait for TTL / flush cache |

## 32.14 Exercises

1. Resolve example.com to its A record and print it.
2. Look up MX records for a domain.
3. Look up TXT records (SPF/DKIM).
4. Reverse-lookup an IP to its name.
5. Explain A vs AAAA records.
