# tlssweep — multi-host TLS posture sweeps

Fetches and grades TLS certificates across many hosts, composing the
`tls.fetch` → `x509.parse_pem` → `tlscheck.assess` chain with the `report`
builder. A sweep never aborts on one bad host: unreachable hosts and
handshake failures become `info` findings instead of errors.

```zen
import tlssweep

let r = tlssweep.sweep_report("Q3 sweep", ["web1", "web2:8443"], null)
r.save("/tmp/sweep.md")
```

## Functions

| Function | Description |
|----------|-------------|
| `sweep_host(target, opts?)` | `{target, ok, findings}` for one `"host"` / `"host:port"`. |
| `sweep(targets, opts?)` | One result per target, in order. |
| `sweep_report(title, targets, opts?)` | Findings folded into a `report` builder. |

`opts`: `insecure` (lab-only, default `false`), `timeout_ms` (default 10000).

> **Imports:** the entry file must import every std module whose functions
> execute — nested imports do not re-export names. Import `report`, `tls`,
> `tlscheck`, `tlssweep`, and `x509` together (see troubleshooting).

Only test systems you own or are authorized to audit.
