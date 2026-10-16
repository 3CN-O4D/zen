# tls — live certificate fetching

The `tls` module fetches the leaf certificate from a live `host:port` over
TLS and returns it as PEM text, ready for `x509.parse_pem`. It is available
globally as `tls`.

The handshake verifies normally: `host` drives both SNI and hostname
verification. Pass `{insecure: true}` only for lab hosts with self-signed
certificates — it disables verification the way curl's `-k` does.

```zen
# Fetch, then hand to the M1 inspection chain
var pem = tls.fetch("example.com", 443)
var cert = x509.parse_pem(pem)
print(cert["not_after"])
```

## Functions

| Function | Description |
|----------|-------------|
| `fetch(host, port?, opts?)` | TLS handshake with `host:port` (default 443); returns the leaf certificate as PEM. Throws on DNS, TCP, timeout, or TLS failures. |

`opts` keys: `timeout_ms` (default 10000, connect and I/O), `insecure`
(default `false` — disables verification for lab hosts only).

Only test systems you own or are authorized to audit.

## See Also

- [x509](x509.md) — Parsing the fetched PEM.
- `tlscheck` (Standard Library) — Expiry/weakness findings.
