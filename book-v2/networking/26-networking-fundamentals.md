# Chapter 26: Networking Fundamentals

Before diving into Zen's networking modules, it's important to understand the basics of computer networking, protocols, and how they work.

## What is Networking?

Networking allows computers to communicate with each other over a network (like the internet). In Zen, you can build clients, servers, and work with various protocols.

## The OSI Model (Simplified)

Understanding layers helps when working with networking:

| Layer | Name | What it does | Examples |
|-------|------|--------------|----------|
| 7 | Application | User-facing protocols | HTTP, HTTPS, DNS, FTP |
| 6 | Presentation | Data formatting/encryption | SSL/TLS, JSON |
| 5 | Session | Managing connections | Sessions, cookies |
| 4 | Transport | Reliable delivery | TCP, UDP |
| 3 | Network | Routing between networks | IP (IPv4/IPv6) |
| 2 | Data Link | Local network delivery | Ethernet, WiFi |
| 1 | Physical | Hardware transmission | Cables, radio |

## Key Networking Concepts

### IP Addresses

Unique identifiers for devices on a network:
- **IPv4**: `192.168.1.1`, `8.8.8.8` (32-bit)
- **IPv6**: `2001:4860:4860::8888` (128-bit)
- **localhost/loopback**: `127.0.0.1`

### Ports

Ports identify specific services on a device (0-65535):
- **Well-known ports** (0-1023): HTTP(80), HTTPS(443), SSH(22), FTP(21)
- **Registered ports** (1024-49151): Application-specific
- **Dynamic/private** (49152-65535): Temporary use

### Sockets

A socket is an endpoint for communication - combination of IP address + port.

### TCP vs UDP

| Feature | TCP | UDP |
|---------|-----|-----|
| Connection | Connection-oriented | Connectionless |
| Reliability | Guaranteed delivery | No guarantee |
| Ordering | Ordered delivery | No ordering |
| Speed | Slower (overhead) | Faster |
| Use cases | HTTP, HTTPS, FTP, Email | DNS, Streaming, Gaming |

## Common Protocols

### HTTP (HyperText Transfer Protocol)
- Port 80
- Request/Response model
- Stateless
- Used for web pages and APIs

### HTTPS (HTTP Secure)
- Port 443
- HTTP over TLS/SSL encryption
- Secure communication

### DNS (Domain Name System)
- Port 53
- Translates domain names to IP addresses
- `example.com` → `93.184.216.34`

### FTP (File Transfer Protocol)
- Ports 20, 21
- File transfers

## Client-Server Model

Most network applications follow this model:
- **Server**: Listens for connections, provides services
- **Client**: Connects to server, requests services

```zen
# Conceptually
# Server waits for connections on port 8080
# Client connects to server:8080 and requests data
```

## URLs (Uniform Resource Locators)

Structure: `scheme://host:port/path?query#fragment`

Example: `https://example.com:443/products?id=123#details`
- scheme: `https`
- host: `example.com`
- port: `443`
- path: `/products`
- query: `id=123`
- fragment: `details`

## Zen Networking Modules

Zen provides several modules for networking:

| Module | Purpose | Level |
|--------|---------|-------|
| `socket` | Low-level BSD sockets | Very low |
| `http` | HTTP server | High-level |
| `requests` | HTTP client | High-level |
| `dns` | DNS lookups | Mid-level |
| `url` | URL parsing | Utility |
| `raw_socket` | Raw packet access | Very low |

## Network Programming Workflow

### For a Client (e.g., HTTP request)
1. Resolve hostname to IP (DNS)
2. Create socket
3. Connect to server
4. Send request
5. Receive response
6. Close connection

### For a Server
1. Create socket
2. Bind to IP and port
3. Listen for connections
4. Accept connections
5. Handle requests
6. Send responses
7. Close

## Security Considerations

When doing network programming:
- **Validate input** - Never trust network data
- **Use HTTPS** - Encrypt sensitive data
- **Check certificates** - Verify SSL/TLS certs
- **Handle errors** - Network can fail
- **Rate limiting** - Be respectful to servers
- **Firewalls** - Consider network restrictions

## Chapter Summary

- Networking enables inter-computer communication
- TCP is reliable, UDP is fast but unreliable
- Ports identify services, IPs identify hosts
- HTTP/HTTPS for web, DNS for name resolution
- Zen has socket, http, requests, dns modules
- Understand client-server model
- Security is crucial in network programming

**Next:** Chapter 27 - socket Module
