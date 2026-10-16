# Security auditing with Zen

Zen ships an offline-first auditing toolkit: recon primitives, certificate
inspection, scan-state diffing, TLS sweeps, low-profile helpers, a module
framework, and report builders. Everything composes through one shape —
`{severity, title, detail, evidence, remediation}` findings.

> **Scope rule:** only test systems you own or are authorized to audit.
> Every offensive-capable module carries this line, and `audit` modules
> additionally refuse to run against targets outside their declared scope.

## The pipeline

```
recon → assess → diff/sweep → report
```

1. **Recon (offline):** `hashid` identifies hash strings, `codec` chains
   text transforms, `fingerprint` guesses products from banners you already
   collected (`nmap.service`, `socket.recv_text`, `Server` headers).
2. **Assess:** `x509` parses certificates; `tlscheck` grades them (expiry,
   weak signatures, short keys, hostname mismatch); `tls.fetch` grabs live
   certificates for the same chain.
3. **Track:** `scandiff` diffs snapshots (opened/closed/changed) with sqlite
   state; `tlssweep` sweeps many hosts without aborting on dead ones.
4. **Report:** `report.create(...).extend(findings).save(path)` renders
   Markdown or HTML, critical-first.
5. **Frame:** `audit.create(...)` scopes named checks to declared targets.

## Module map

| Module | Kind | Needs network |
|--------|------|---------------|
| `hashid`, `codec`, `fingerprint` | stdlib (pure Zen) | never |
| `x509` | native (offline parse) | never |
| `tlscheck`, `report`, `scandiff`, `opsec`, `audit` | stdlib (pure Zen) | never |
| `tls`, `tlssweep` | native fetch / stdlib sweep | yes (target hosts) |

## Worked example: TLS posture review

```zen
import report
import tls
import tlscheck
import tlssweep
import x509

let r = tlssweep.sweep_report("Q3 external", ["web1", "web2:8443"], null)
r.save("/tmp/q3-tls.md")
print r.summary()
```

## Explicitly out of scope

Zen does not ship payload templates, exploit databases, or credential-spray
lists: point at dedicated tooling for those, and keep every audit inside
written authorization.
