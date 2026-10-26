# Chapter 32: dns Module

The `dns` module provides DNS resolution: forward lookups (name → address),
reverse lookups (address → name), and hostname information.

## 32.1 Resolve a hostname

```zen
import dns

let addrs = dns.resolve("example.com")
for addr in addrs {
    print(addr)
}
# 93.184.216.34
# 2606:2800:220:1:248:1893:25c8:1946
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

print(is_resolvable("example.com"))  # true
print(is_resolvable("nonexistent.invalid"))  # false
```

### Get the first IPv4 address

```zen
import dns

func first_ipv4(host) {
    let addrs = dns.resolve(host)
    for addr in addrs {
        if addr.contains(".") {  # IPv4 has dots
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
    # ... use socket ...
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
