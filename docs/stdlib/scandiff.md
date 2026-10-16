# scandiff — scan-result diffing

Continuous-monitoring story for `nmap` scans: record snapshots, then diff
them — new/closed ports, changed banners. `diff` is pure and offline;
`record`/`restore`/`compare` persist snapshots in sqlite (in-memory or file).

```zen
import scandiff

let d = scandiff.diff(last_week, today)
print d["opened"]   # [{host, port, state, service}, ...]
```

## Functions

| Function | Description |
|----------|-------------|
| `diff(old_scan, new_scan)` | `{opened, closed, changed}` between two scans. Changed entries carry `before`/`after` port dicts. |
| `record(h, label, scan)` | Stores a snapshot under `label` in an open sqlite handle; returns the port count. |
| `restore(h, label)` | Rebuilds a scan dict from a stored snapshot; throws on unknown labels. |
| `compare(h, old_label, new_label)` | `restore` both labels, then `diff`. |

Scan shape: `{host, ports: [{port, state, service, banner}]}`.

Only test systems you own or are authorized to audit.
