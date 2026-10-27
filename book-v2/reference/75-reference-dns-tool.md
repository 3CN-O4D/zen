# Chapter 75: Reference Implementation — DNS Lookup Tool

A small DNS client that resolves a domain to its A, AAAA, MX, TXT, and NS
records, and performs reverse lookups. Demonstrates the `dns` module, record
types, and CLI argument handling.

## 75.1 The program (`dnslookup.z`)

```zen
import dns
import sys

func show_record(label, values) {
    print("  " + label + ":")
    if values == nil or len(values) == 0 {
        print("    (none)")
    } else {
        for v in values { print("    " + str(v)) }
    }
}

func lookup(domain) {
    print("DNS lookup for " + domain)
    print("========================")
    show_record("A", dns.resolve(domain, "A"))
    show_record("AAAA", dns.resolve(domain, "AAAA"))
    show_record("CNAME", dns.resolve(domain, "CNAME"))
    show_record("MX", dns.resolve(domain, "MX"))
    show_record("TXT", dns.resolve(domain, "TXT"))
    show_record("NS", dns.resolve(domain, "NS"))
}

func reverse_lookup(ip) {
    print("Reverse lookup for " + ip)
    print("========================")
    show_record("PTR", dns.reverse(ip))
}

func main(argv) {
    if len(argv) < 2 {
        print("Usage:")
        print("  zen run dnslookup.z <domain>        # forward lookup")
        print("  zen run dnslookup.z --reverse <ip>  # reverse lookup")
        return 1
    }
    if argv[1] == "--reverse" and len(argv) >= 3 {
        reverse_lookup(argv[2])
    } else {
        lookup(argv[1])
    }
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 75.2 Example session

```
$ zen run dnslookup.z example.com
DNS lookup for example.com
========================
  A:
    93.184.216.34
  AAAA:
    2606:2800:220:1:248:1893:25c8:1946
  CNAME:
    (none)
  MX:
    0 example.com
  TXT:
    v=spf1 -all
  NS:
    a.iana-servers.net
    b.iana-servers.net

$ zen run dnslookup.z --reverse 93.184.216.34
Reverse lookup for 93.184.216.34
========================
  PTR:
    example.com
```

## 75.3 Design notes

- Record types: A (IPv4), AAAA (IPv6), CNAME (alias), MX (mail server), TXT
  (arbitrary text, e.g. SPF), NS (name server), PTR (reverse IP→name).
- `dns.resolve` returns a list; `dns.reverse` returns the PTR name(s).
- Missing record types return an empty list (printed as "(none)").

## 75.4 Exercises

1. Add `--type <TYPE>` to query a single record type.
2. Add `--server <IP>` to use a specific DNS resolver (e.g., 1.1.1.1).
3. Add `--timeout N` for resolution timeouts.
4. Batch mode: read domains from a file, one per line, resolve all.
5. Check SPF/DKIM TXT records and print whether they look valid.
