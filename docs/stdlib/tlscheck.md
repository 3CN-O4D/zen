# tlscheck — certificate findings

Turns `x509.parse_pem` output into report-ready findings: expiry grading,
weak signature algorithms, short RSA keys, hostname mismatch, and
self-signed leaves. Offline — it never opens a connection.

```zen
import tlscheck

let findings = tlscheck.from_file("site.pem", "example.com")
```

## Functions

| Function | Description |
|----------|-------------|
| `assess(cert, host?)` | Grades a parsed cert dict; `host` enables the SAN/CN check. |
| `from_pem(pem, host?)` | Parses PEM, then assesses. |
| `from_file(path, host?)` | Reads PEM from disk, then assesses. |

## Grading

| Condition | Severity |
|-----------|----------|
| Expired | critical |
| Expires within 30 days | high |
| Expires within 90 days | medium |
| md5/sha1 signatures | high |
| RSA below 2048 bits | high |
| Host not in SAN/CN | medium |
| Self-signed non-CA leaf | low |

Findings plug directly into `report.create(...).extend(...)`.

Only test systems you own or are authorized to audit.
