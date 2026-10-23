# Chapter 28: HTTP Protocol

HTTP (HyperText Transfer Protocol) is the foundation of data communication on the web. This chapter explains HTTP in detail and how to work with it in Zen.

## What is HTTP?

HTTP is an application-layer protocol for transmitting hypermedia documents. It's:
- **Stateless**: Each request is independent
- **Client-Server**: Client requests, server responds
- **Text-based**: Human-readable protocol (though can carry binary)
- **Request-Response**: Follows this pattern

## HTTP Versions

| Version | Status | Notes |
|---------|--------|-------|
| HTTP/1.0 | Legacy | Simple, one request per connection |
| HTTP/1.1 | Widely used | Persistent connections, chunked transfer |
| HTTP/2 | Modern | Multiplexing, binary, server push |
| HTTP/3 | New | Uses QUIC over UDP |

## HTTP Methods

HTTP methods (verbs) indicate the action to perform:

| Method | Purpose | Idempotent | Safe | Body |
|--------|---------|------------|------|------|
| GET | Retrieve resource | Yes | Yes | Usually no |
| POST | Create resource / submit data | No | No | Yes |
| PUT | Update/replace resource (full) | Yes | No | Yes |
| PATCH | Partial update | No | No | Yes |
| DELETE | Remove resource | Yes | No | Usually no |
| HEAD | Same as GET but no body | Yes | Yes | No |
| OPTIONS | Get supported methods | Yes | Yes | No |

### Examples

```zen
# GET - retrieve data
GET /users/123 HTTP/1.1
Host: api.example.com

# POST - create data
POST /users HTTP/1.1
Host: api.example.com
Content-Type: application/json

{"name": "Alice", "email": "alice@example.com"}

# PUT - replace entire resource
PUT /users/123 HTTP/1.1
Host: api.example.com
Content-Type: application/json

{"name": "Alice Smith", "email": "alice@example.com", "age": 30}

# DELETE - remove resource
DELETE /users/123 HTTP/1.1
Host: api.example.com
```

## HTTP Status Codes

Status codes indicate the result of the request:

### 1xx Informational
- 100 Continue
- 101 Switching Protocols

### 2xx Success
- 200 OK - Request succeeded
- 201 Created - Resource created
- 204 No Content - Success but no body

### 3xx Redirection
- 301 Moved Permanently
- 302 Found (temporary redirect)
- 304 Not Modified - Use cached version

### 4xx Client Errors
- 400 Bad Request - Invalid syntax
- 401 Unauthorized - Authentication required
- 403 Forbidden - Not allowed
- 404 Not Found - Resource doesn't exist
- 405 Method Not Allowed
- 422 Unprocessable Entity
- 429 Too Many Requests

### 5xx Server Errors
- 500 Internal Server Error
- 502 Bad Gateway
- 503 Service Unavailable
- 504 Gateway Timeout

## HTTP Headers

Headers provide additional information about the request/response.

### Common Request Headers
```http
Host: example.com
User-Agent: Zen/3.0
Accept: text/html,application/json
Content-Type: application/json
Authorization: Bearer token123
Cookie: session=abc123
```

### Common Response Headers
```http
Content-Type: text/html; charset=utf-8
Content-Length: 1234
Cache-Control: max-age=3600
Set-Cookie: session=xyz789
Server: nginx
Access-Control-Allow-Origin: *
```

## Making HTTP Requests with Zen's `requests` Module

The easiest way to make HTTP requests:

```zen
import requests

# GET request
let resp = requests.get("https://httpbin.org/get")
print("Status:", resp.status_code)
print("Body:", resp.text)
```

### GET with Query Parameters

```zen
import requests

let resp = requests.get("https://httpbin.org/get", 
    params={"name": "Alice", "age": "30"}
)
print(resp.text)
```

### POST with JSON

```zen
import requests
import json

let data = {"username": "alice", "password": "secret"}
let resp = requests.post("https://httpbin.org/post",
    json=data,
    headers={"User-Agent": "Zen-App/1.0"}
)
print(resp.status_code)
let body = json.parse(resp.text)
print(body.json)
```

### POST with Form Data

```zen
import requests

let resp = requests.post("https://httpbin.org/post",
    data={"key1": "value1", "key2": "value2"}
)
print(resp.text)
```

### Custom Headers

```zen
import requests

let resp = requests.get("https://httpbin.org/headers",
    headers={
        "Authorization": "Bearer mytoken",
        "X-Custom-Header": "hello"
    }
)
print(resp.text)
```

### Handling Response

```zen
import requests
import json

let resp = requests.get("https://httpbin.org/status/200")

print("Status Code:", resp.status_code)
print("Headers:", resp.headers)
print("Text:", resp.text)
print("URL:", resp.url)

# If JSON response
if resp.headers["Content-Type"].contains("json") {
    let data = resp.json()
    print(data)
}
```

## Building an HTTP Server

Use the `http` module to create servers:

```zen
import http
import json

func handle_request(req) {
    let path = req.path
    let method = req.method
    
    if path == "/" && method == "GET" {
        return {
            "status": 200,
            "headers": {"Content-Type": "text/html"},
            "body": "<h1>Welcome to Zen HTTP Server</h1>"
        }
    }
    
    if path == "/api/data" && method == "GET" {
        let data = {"message": "Hello", "count": 42}
        return {
            "status": 200,
            "headers": {"Content-Type": "application/json"},
            "body": json.stringify(data)
        }
    }
    
    if path == "/api/echo" && method == "POST" {
        return {
            "status": 200,
            "headers": {"Content-Type": "application/json"},
            "body": json.stringify({"echo": req.body})
        }
    }
    
    return {
        "status": 404,
        "headers": {"Content-Type": "text/html"},
        "body": "<h1>404 Not Found</h1>"
    }
}

print("Server starting on http://localhost:8080")
http.serve("0.0.0.0", 8080, handle_request)
```

## RESTful API Design

```zen
import http
import json

let users = [
    {"id": 1, "name": "Alice"},
    {"id": 2, "name": "Bob"}
]

func api_handler(req) {
    if req.method == "GET" && req.path == "/api/users" {
        return {
            "status": 200,
            "headers": {"Content-Type": "application/json"},
            "body": json.stringify(users)
        }
    }
    
    if req.method == "GET" && req.path.starts_with("/api/users/") {
        let id = int(req.path.split("/")[3])
        for user in users {
            if user.id == id {
                return {
                    "status": 200,
                    "headers": {"Content-Type": "application/json"},
                    "body": json.stringify(user)
                }
            }
        }
        return {"status": 404, "body": json.stringify({"error": "Not found"})}
    }
    
    return {"status": 404, "body": "Not Found"}
}

http.serve("0.0.0.0", 3000, api_handler)
```

## HTTP Cookies

```zen
import http

func handler(req) {
    # Read cookies
    let cookies = req.cookies
    if cookies.contains("session") {
        return {"status": 200, "body": "Welcome back!"}
    }
    
    # Set cookie in response
    return {
        "status": 200,
        "headers": {
            "Set-Cookie": "session=abc123; Path=/; HttpOnly"
        },
        "body": "Cookie set!"
    }
}
```

## CORS (Cross-Origin Resource Sharing)

```zen
func handler(req) {
    if req.method == "OPTIONS" {
        return {
            "status": 200,
            "headers": {
                "Access-Control-Allow-Origin": "*",
                "Access-Control-Allow-Methods": "GET, POST, PUT, DELETE",
                "Access-Control-Allow-Headers": "Content-Type, Authorization"
            },
            "body": ""
        }
    }
    
    return {
        "status": 200,
        "headers": {
            "Access-Control-Allow-Origin": "*",
            "Content-Type": "application/json"
        },
        "body": json.stringify({"data": "hello"})
    }
}
```

## Best Practices

1. **Use appropriate methods** - GET for reads, POST for creates, etc.
2. **Handle errors gracefully** - Check status codes
3. **Set proper headers** - Content-Type, User-Agent
4. **Use HTTPS** - For sensitive data
5. **Validate input** - On both client and server
6. **Use timeouts** - Don't wait forever
7. **Respect rate limits** - Don't spam servers
8. **Follow REST conventions** - For APIs

## Chapter Summary

- HTTP is stateless request-response protocol
- Methods: GET, POST, PUT, PATCH, DELETE
- Status codes indicate success/failure
- Headers carry metadata
- Use `requests` module for clients, `http` module for servers
- Handle JSON with `json` module
- Follow RESTful patterns for APIs

**Next:** Chapter 29 - requests Module
