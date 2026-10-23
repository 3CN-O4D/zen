# Chapter 31: HTTPS & SSL/TLS Certificates

HTTPS (HyperText Transfer Protocol Secure) is HTTP layered over TLS/SSL encryption. This chapter covers how to work with secure connections, certificates, and certificate validation in Zen.

## What is HTTPS?

HTTPS uses TLS (Transport Layer Security) to encrypt communication between client and server. It provides:

- **Confidentiality** - Data is encrypted, can't be read by eavesdroppers
- **Integrity** - Data can't be modified in transit
- **Authentication** - Verifies the server's identity via certificates

## HTTP vs HTTPS

| Aspect | HTTP | HTTPS |
|--------|------|-------|
| Port | 80 | 443 |
| Encryption | None | TLS/SSL encrypted |
| Security | Vulnerable | Secure |
| URL Scheme | `http://` | `https://` |
| Certificate | Not required | Required |

## How TLS Works (Simplified)

1. **Client Hello** - Client sends supported TLS versions and cipher suites
2. **Server Hello** - Server responds with chosen version and certificate
3. **Certificate Verification** - Client verifies server's certificate
4. **Key Exchange** - Establish shared encryption keys
5. **Encrypted Communication** - All data encrypted with session keys

## Making HTTPS Requests

The `requests` module handles HTTPS automatically:

```zen
import requests

# HTTPS request - certificate verification is enabled by default
let resp = requests.get("https://api.github.com/users/octocat")
print(resp.status_code)
print(resp.text)
```

### Disabling Certificate Verification (Not Recommended)

Only do this for testing/development:

```zen
import requests

# WARNING: Insecure - only for testing
let resp = requests.get("https://self-signed.badssl.com/", verify=false)
print(resp.status_code)
```

### Using Custom CA Certificates

```zen
import requests

let resp = requests.get("https://internal.company.com/api",
    verify="/path/to/custom-ca-cert.pem"
)
print(resp.status_code)
```

## Understanding SSL Certificates

### Certificate Components

- **Subject** - Who the certificate is issued to (domain name)
- **Issuer** - Certificate Authority (CA) that issued it
- **Valid From/To** - Validity period
- **Public Key** - Used for encryption/verification
- **Signature** - CA's digital signature
- **Serial Number** - Unique identifier

### Certificate Types

| Type | Description | Use Case |
|------|-------------|----------|
| DV (Domain Validated) | Basic validation of domain ownership | Personal sites, blogs |
| OV (Organization Validated) | Validates organization identity | Business websites |
| EV (Extended Validation) | Strict validation process | Banks, e-commerce |
| Wildcard | Covers subdomains (*.example.com) | Multiple subdomains |
| Self-Signed | Signed by yourself, not trusted by browsers | Development, internal use |

## Working with Certificates in Zen

### Inspecting Certificates

```zen
import crypto.x509
import fs

let cert_pem = fs.read("server.crt")
let cert = crypto.x509.load_cert(cert_pem)

print("Subject:", cert.subject)
print("Issuer:", cert.issuer)
print("Serial:", cert.serial_number)
print("Valid From:", cert.not_before)
print("Valid Until:", cert.not_after)
print("Signature Algorithm:", cert.signature_algorithm)
```

### Checking Certificate Expiration

```zen
import crypto.x509
import time

func check_cert_expiry(cert_path) {
    let cert_pem = fs.read(cert_path)
    let cert = crypto.x509.load_cert(cert_pem)
    
    let now = time.time()
    let expiry = cert.not_after_timestamp
    
    if expiry < now {
        print("Certificate EXPIRED!")
        return false
    }
    
    let days_left = (expiry - now) / 86400
    print("Days until expiry: " + int(days_left))
    
    if days_left < 30 {
        print("WARNING: Certificate expiring soon!")
    }
    
    return true
}

check_cert_expiry("server.crt")
```

### Verifying Certificate Chain

```zen
import crypto.x509

func verify_chain(server_cert_path, ca_cert_path) {
    let server_cert = crypto.x509.load_cert(fs.read(server_cert_path))
    let ca_cert = crypto.x509.load_cert(fs.read(ca_cert_path))
    
    try {
        let valid = crypto.x509.verify(server_cert, ca_cert)
        if valid {
            print("Certificate chain is valid")
        } else {
            print("Certificate chain is invalid")
        }
        return valid
    } catch (e) {
        print("Verification error: " + e)
        return false
    }
}
```

## Creating Self-Signed Certificates (for Development)

While Zen might not have built-in cert generation, you can use OpenSSL via shell or pymport:

```zen
import subprocess

func generate_self_signed(domain) {
    let cmd = "openssl req -x509 -newkey rsa:2048 -keyout key.pem -out cert.pem -days 365 -nodes -subj '/CN=" + domain + "'"
    subprocess.run(cmd, shell=true)
    print("Generated cert.pem and key.pem")
}

generate_self_signed("localhost")
```

Or with Python:

```zen
pymport OpenSSL.crypto

# Generate self-signed cert using pyOpenSSL
```

## HTTPS Server with Certificates

```zen
import http

func handler(req) {
    return {
        "status": 200,
        "headers": {"Content-Type": "text/html"},
        "body": "<h1>Secure HTTPS Server</h1>"
    }
}

# Serve over HTTPS with cert and key
http.serve_tls("0.0.0.0", 443, handler, {
    "cert": "server.crt",
    "key": "server.key"
})
```

## Mutual TLS (mTLS)

mTLS requires both client and server to present certificates:

```zen
import requests

let resp = requests.get("https://secure-api.example.com/data",
    cert=("client.crt", "client.key"),
    verify="ca.crt"
)
print(resp.text)
```

## Certificate Pinning

Pin certificates to prevent MITM attacks:

```zen
import requests
import crypto.hash

func pinned_request(url, expected_fingerprint) {
    let resp = requests.get(url, verify=true)
    # Extract and verify certificate fingerprint
    let fingerprint = get_cert_fingerprint(resp)
    if fingerprint != expected_fingerprint {
        throw "Certificate pinning failed!"
    }
    return resp
}

# SHA256 fingerprint of expected certificate
let fp = "AA:BB:CC:..."
pinned_request("https://example.com", fp)
```

## Common SSL/TLS Issues

### Certificate Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `CERTIFICATE_VERIFY_FAILED` | Self-signed or untrusted CA | Add CA to trust store or use `verify=false` for testing |
| `HOSTNAME_MISMATCH` | Cert doesn't match domain | Use correct domain or get proper cert |
| `CERT_HAS_EXPIRED` | Cert past expiration date | Renew certificate |
| `SELF_SIGNED_CERT_IN_CHAIN` | Self-signed in chain | Trust the CA or disable verification |

### Debugging SSL Issues

```zen
import requests

try {
    let resp = requests.get("https://problematic.site", verify=true)
} catch (e) {
    print("Error: " + e)
    if e.contains("CERTIFICATE_VERIFY_FAILED") {
        print("Certificate verification failed - check CA bundle")
    } elif e.contains("hostname") {
        print("Hostname mismatch")
    }
}
```

## Let's Encrypt & Free Certificates

For production, use Let's Encrypt for free certificates. You can automate with certbot:

```bash
# Get certificate
certbot certonly --standalone -d example.com
```

Certificates will be in `/etc/letsencrypt/live/example.com/`

## Security Best Practices

1. **Always use HTTPS in production** - No HTTP for sensitive data
2. **Verify certificates** - Never disable verification in production
3. **Use modern TLS versions** - TLS 1.2 or 1.3 only
4. **Keep certificates updated** - Monitor expiration
5. **Use strong ciphers** - Let libraries choose secure defaults
6. **Implement HSTS** - Force HTTPS with HTTP Strict Transport Security
7. **Use certificate pinning** - For high-security apps
8. **Protect private keys** - Restrict file permissions

## HSTS Example

```zen
func handler(req) {
    return {
        "status": 200,
        "headers": {
            "Strict-Transport-Security": "max-age=31536000; includeSubDomains"
        },
        "body": "Hello"
    }
}
```

## Chapter Summary

- HTTPS = HTTP + TLS encryption
- Certificates authenticate servers and enable encryption
- `requests` verifies certificates by default
- Use `crypto.x509` to inspect certificates
- Self-signed certs only for development
- Use Let's Encrypt for free production certs
- Always verify certificates in production
- Consider mTLS for mutual authentication

**Next:** Chapter 32 - DNS Module
