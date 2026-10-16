# audit — authorized-audit modules

Metasploit-shaped scaffolding for audit modules: named checks run against
an explicit scope, results come back as report-ready findings, and anything
outside the declared scope refuses to run. Detection and reporting only —
there is no exploitation support, by design.

```zen
import audit
import report

let m = audit.create("lab-net", "quarterly review")
m.set_scope(["web1", "db1"])
m.add_check("tls", fn(t) { return tlscheck.from_file(t + ".pem", t)[0] })
let r = report.create("lab-net audit")
r.extend(m.run_checks("web1"))
```

## Functions

| Function | Description |
|----------|-------------|
| `create(name, description?)` | New audit module with empty scope and checks. |
| `set_scope(targets)` | Replaces the scope list; returns its size. |
| `scope()` | Copy of the current scope list. |
| `add_check(name, fn)` | Registers `fn(target)` → finding dict or `null`. |
| `in_scope(target)` | Scope membership test. |
| `run_checks(target)` | Runs every check; throws when `target` is outside the scope. A throwing check becomes an `info` finding — checks never abort each other. |

Only test systems you own or are authorized to audit.
