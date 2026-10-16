# fingerprint — service/banner fingerprinting

Turns banners you already collected (`nmap.service`, `socket.recv_text`,
HTTP `Server` headers) into product/version guesses. Matching is deliberately
conservative: unknown input yields `{product: "unknown"}` rather than a
fabricated version.

```zen
import fingerprint

print fingerprint.service("SSH-2.0-OpenSSH_9.6p1 Ubuntu-1")
# {product: openssh, version: 9.6p1, confidence: high, ...}
```

## Functions

| Function | Description |
|----------|-------------|
| `service(banner)` | `{product, version, confidence, reason}` guess for SSH/FTP/SMTP/HTTP banners. |
| `http_server(value)` | Alias for `service`, for `Server` header values. |

Covered products: OpenSSH/Dropbear-style SSH tokens, vsftpd, ProFTPD,
Pure-FTPd, FileZilla Server, Postfix, Exim, Sendmail, Exchange greetings,
Apache, nginx, IIS, lighttpd, OpenResty, gunicorn.

Only test systems you own or are authorized to audit.
