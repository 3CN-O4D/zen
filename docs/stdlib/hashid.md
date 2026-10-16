# hashid — hash identification

Offline recon helper in the spirit of hashID: given a hash string, returns
candidate algorithms with a confidence level. Ambiguous lengths (md5 vs ntlm,
sha256 vs sha3-256) return every candidate honestly — never silently picks one.

```zen
import hashid

print hashid.likely("900150983cd24fb0d6963f7d28e17f72")  # md5
```

## Functions

| Function | Description |
|----------|-------------|
| `identify(s)` | List of `{name, bits, encoding, confidence, reason}` candidates. |
| `likely(s)` | Top candidate name, or `null` when nothing matches. |

## Covered formats

Hex digests by length (32/40/56/64/96/128 chars), plus modular-crypt
formats: `bcrypt` (`$2a$/$2b$/$2x$/$2y$`), `md5crypt` (`$1$`), `apr1`
(`$apr1$`), `sha256crypt` (`$5$`), `sha512crypt` (`$6$`), `argon2`.

Only test systems you own or are authorized to audit.
