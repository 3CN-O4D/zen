# opsec — low-profile scanning helpers

Jittered delays, nmap-style timing templates, and rotating benign request
headers. These reduce scan noise against systems you are authorized to audit;
they are not evasion tooling for anyone else's systems.

```zen
import opsec

opsec.pause(opsec.timing_delay(2))   # polite template + jitter
let h = opsec.request_headers()      # rotated browser-like headers
```

## Functions

| Function | Description |
|----------|-------------|
| `timing_delay(template)` | Ms per probe for nmap-style `-T0..-T5` (unknown → normal 250). |
| `jitter(base_ms, pct?)` | `base_ms` randomized by ±`pct`% (default 25); deterministic at `pct=0`. |
| `pause(base_ms, pct?)` | Sleeps the jittered delay; returns `true`. |
| `user_agent()` | Rotates common browser user-agents. |
| `request_headers()` | Benign `User-Agent` / `Accept` / `Accept-Language` dict. |

Only test systems you own or are authorized to audit.
