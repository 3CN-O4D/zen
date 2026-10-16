# report — findings to reports

Collects `{severity, title, detail, evidence, remediation}` findings (as
produced by `tlscheck.assess`) and renders Markdown or HTML reports with a
severity summary. Pure offline, pure Zen.

```zen
import report
import tlscheck

let r = report.create("Q3 external audit")
r.extend(tlscheck.from_file("site.pem", "example.com"))
r.add("info", "scope", "3 hosts reviewed", "", "")
r.save("/tmp/audit.md")
```

## Functions

| Function | Description |
|----------|-------------|
| `create(title)` | New empty report builder. |
| `add(severity, title, detail, evidence?, remediation?)` | Appends one finding; unknown severities throw. |
| `extend(items)` | Appends a findings list (e.g. from `tlscheck`). |
| `summary()` | `{total, critical, high, medium, low, info}` counts. |
| `markdown()` | Full report as Markdown, critical-first. |
| `html()` | Full report as an HTML table (content escaped). |
| `save(path, format?)` | Writes `"markdown"` (default) or `"html"` to disk. |

Only test systems you own or are authorized to audit.
