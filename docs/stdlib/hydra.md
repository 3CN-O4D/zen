# hydra — login testing

Brute-force login testing in the spirit of THC-Hydra (for authorized
audits of your own systems): SSH, FTP, HTTP, Telnet, and SMTP.

```zen
import hydra

print hydra.test("ssh", "127.0.0.1", "root", "toor")
```

## Functions

| Function | Description |
|----------|-------------|
| `ssh(host, user, pass)` | Single SSH login attempt. |
| `ftp(host, user, pass)` | Single FTP login attempt. |
| `http(host, user, pass, urlpath)` | Single HTTP-auth attempt. |
| `telnet(host, user, pass)` | Single Telnet login attempt. |
| `smtp(host, user, pass)` | Single SMTP login attempt. |
| `test(proto, host, user, pass)` | Dispatches by protocol name. |
| `run(proto, host, user, passwords)` | Tries a password list; returns the hit. |
| `run_file(proto, host, user, path)` | Tries passwords from a file. |

Only test systems you own or are authorized to audit.
