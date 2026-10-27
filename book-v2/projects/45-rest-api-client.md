# Chapter 45: REST API Client

This chapter shows how to build a REST API client in Zen using `requests`, `json`, and `argparser`. It covers authentication, pagination, rate limiting, error handling, and building a reusable client class.

## 45.1 Basic GET request

```zen
import requests
import json

let resp = requests.get("https://api.example.com/users/1")
if resp.ok {
    let user = resp.json()
    print(user["name"])
} else {
    print("HTTP " + str(resp.status_code))
}
```

## 45.2 POST with JSON body

```zen
import requests
import json

let body = json.dumps({"name": "Zen", "version": 3})
let resp = requests.post("https://example.com/api/users", body, {
    "headers": {"Content-Type": "application/json"}
})
print(resp.status_code)
print(resp.text)
```

## 45.3 Reusable API client

```zen
import requests
import json

class ApiClient {
    let base_url
    let headers
    let session

    func init(base_url, headers = {}) {
        this.base_url = base_url
        this.headers = headers
        this.session = requests.Session()
        this.session.headers = headers
    }

    func get(path, params = {}) {
        let url = this.base_url + path
        let resp = requests.sess_get(this.session, url, {"params": params})
        return resp
    }

    func post(path, data) {
        let url = this.base_url + path
        let body = json.dumps(data)
        let resp = requests.sess_post(this.session, url, {
            "json": body,
            "headers": {"Content-Type": "application/json"}
        })
        return resp
    }

    func put(path, data) {
        let url = this.base_url + path
        let body = json.dumps(data)
        let resp = requests.sess_put(this.session, url, {
            "json": body,
            "headers": {"Content-Type": "application/json"}
        })
        return resp
    }

    func delete(path) {
        let url = this.base_url + path
        let resp = requests.sess_delete(this.session, url)
        return resp
    }

    func close() {
        requests.session_close(this.session)
    }
}
```

### Usage

```zen
let client = new ApiClient("https://api.example.com", {
    "Authorization": "Bearer token123",
    "User-Agent": "ZenClient/1.0"
})

let resp = client.get("/users/1")
if resp.ok {
    print(resp.json()["name"])
}

let created = client.post("/users", {"name": "New User"})
print(created.status_code)

client.close()
```

## 45.4 Authentication

### Bearer token

```zen
let client = new ApiClient("https://api.example.com", {
    "Authorization": "Bearer " + token
})
```

### Basic auth

```zen
import requests

let resp = requests.get("https://example.com/api", {
    "auth": ["user", "pass"]
})
```

### API key header

```zen
let client = new ApiClient("https://api.example.com", {
    "X-API-Key": "your-api-key"
})
```

## 45.5 Pagination

Many APIs paginate results. Here's a pattern for following `next` links:

```zen
import requests

func get_all_pages(url) {
    let all_items = []
    let next_url = url
    while next_url != nil {
        let resp = requests.get(next_url)
        if !resp.ok {
            break
        }
        let data = resp.json()
        for item in data["items"] {
            all_items.push(item)
        }
        next_url = data["next"]  // null when no more pages
    }
    return all_items
}

let items = get_all_pages("https://api.example.com/items?page=1")
print(len(items))
```

### Offset-based pagination

```zen
func get_all_offset(base_url, page_size = 50) {
    let all_items = []
    let offset = 0
    while true {
        let resp = requests.get(base_url, {
            "params": {"offset": offset, "limit": page_size}
        })
        if !resp.ok {
            break
        }
        let data = resp.json()
        let items = data["items"]
        for item in items {
            all_items.push(item)
        }
        if len(items) < page_size {
            break
        }
        offset = offset + page_size
    }
    return all_items
}
```

## 45.6 Rate limiting

Respect `RateLimit` headers and retry on 429:

```zen
import requests
import time

func get_with_retry(url, max_retries = 3) {
    let attempt = 0
    while attempt < max_retries {
        let resp = requests.get(url)
        if resp.ok {
            return resp
        }
        if resp.status_code == 429 {
            let retry_after = resp.headers["Retry-After"]
            let wait = retry_after != nil ? int(retry_after) : 1
            print("Rate limited, waiting " + str(wait) + "s")
            time.sleep(wait)
            attempt = attempt + 1
        } else {
            break
        }
    }
    return nil
}

let resp = get_with_retry("https://api.example.com/data")
if resp != nil {
    print(resp.text)
}
```

## 45.7 Error handling

```zen
import requests

func api_get(client, path) {
    let resp = client.get(path)
    if resp.status_code == 404 {
        return nil  // not found
    }
    if !resp.ok {
        throw "API error: " + str(resp.status_code) + " " + resp.reason
    }
    return resp.json()
}

try {
    let user = api_get(client, "/users/999")
    if user == nil {
        print("User not found")
    } else {
        print(user["name"])
    }
} catch as e {
    print("API call failed: " + e.msg)
}
```

## 45.8 Complete CLI example

```zen
import requests
import json
import argparser
import fs

let parser = argparser.ArgumentParser("api-client")
parser.add_argument("url", help="API URL")
parser.add_argument("--method", "-m", default="GET", help="HTTP method")
parser.add_argument("--data", "-d", help="JSON body")
parser.add_argument("--header", "-H", action="append", help="Header (key:value)")
let args = parser.parse_args()

let headers = {}
if args.header != nil {
    for h in args.header {
        let parts = h.split(":", 1)
        if len(parts) == 2 {
            headers[parts[0]] = parts[1]
        }
    }
}

let body = args.data != nil ? args.data : nil

let resp = requests.get(args.url, {"headers": headers})
// ... extend for other methods

print(resp.status_code)
print(resp.text)
```

## 45.9 Common pitfalls

- **Always check `resp.ok`.** A non-200 response still returns a response object.
- **JSON bodies must be strings.** Use `json.dumps()` to serialize.
- **Sessions persist cookies and headers.** Close them with `requests.session_close()`.
- **Rate limits.** Respect `Retry-After` and back off on 429.
- **Timeouts.** Always pass a `timeout` option to avoid hanging.

## 45.10 Quick reference

```zen
import requests
import json

// GET
let resp = requests.get("https://api.example.com/items", {
    "params": {"page": 1},
    "headers": {"Authorization": "Bearer token"}
})

// POST JSON
let resp = requests.post("https://example.com/api", {
    "json": json.dumps({"name": "Zen"}),
    "headers": {"Content-Type": "application/json"}
})

// Session
let sess = requests.Session()
sess.headers = {"Authorization": "Bearer token"}
let resp = requests.sess_get(sess, "https://example.com/api")
requests.session_close(sess)

// Status
resp.status_code
resp.ok
resp.text
resp.json()
resp.headers
```

## 45.11 Exercises

1. Write a REST client for a public API (e.g., JSONPlaceholder) that lists posts.
2. Add pagination support to fetch all pages.
3. Add retry logic with exponential backoff.
4. Write a CLI that takes a URL and method and prints the response.
5. Build a small config client that reads/writes JSON config via REST.


## Extended Project Ideas and Implementation Notes

### Design checklist
- Requirements and scope
- Error handling
- Logging
- Configuration
- Testing
- Documentation
- Security considerations

### Exercises
1. Extend the project with new features
2. Add tests
3. Improve error messages
4. Add help/usage
5. Make it configurable
6. Package for distribution


## 45.12 Full tutorial — REST API client

### Goal
A small client for a JSON REST API with GET/POST/PUT/DELETE and auth.

### Implementation (`api.z`)

```zen
import requests
import json

class ApiClient {
    var base_url = ""
    var token = nil

    func init(base_url, token = nil) {
        self.base_url = base_url
        self.token = token
    }

    func headers() {
        let h = #{}
        h["Content-Type"] = "application/json"
        if self.token != nil { h["Authorization"] = "Bearer " + self.token }
        return h
    }

    func request(method, path, body = nil) {
        let url = self.base_url + path
        try {
            let r = nil
            if method == "GET" { r = requests.get(url, headers=self.headers(), timeout=10) }
            elif method == "POST" { r = requests.post(url, headers=self.headers(), json=body, timeout=10) }
            elif method == "PUT" { r = requests.put(url, headers=self.headers(), json=body, timeout=10) }
            elif method == "DELETE" { r = requests.delete(url, headers=self.headers(), timeout=10) }
            else { throw "unsupported method: " + method }
            r.raise_for_status()
            let ct = r.headers["Content-Type"] ?? ""
            if ct.contains("json") { return r.json() }
            return r.text
        } catch as e {
            throw "API request failed (" + method + " " + path + "): " + e.msg
        }
    }

    func get(path) { return self.request("GET", path) }
    func post(path, body) { return self.request("POST", path, body) }
    func put(path, body) { return self.request("PUT", path, body) }
    func delete(path) { return self.request("DELETE", path) }
}

func main(argv) {
    let client = new ApiClient("https://api.example.com", "my-token")
    try {
        let users = client.get("/users")
        print("users: " + json.stringify(users))
        let created = client.post("/users", #{name: "Zen User"})
        print("created: " + json.stringify(created))
    } catch as e {
        print(e.msg)
        return 1
    }
    return 0
}

if __name__ == "__main__" { sys.exit(main(sys.argv)) }
```

### Key design points
- `base_url` + path composition.
- Bearer token auth header.
- Central error handling with context (method + path).
- Content-type check before `r.json()`.
- Timeout on every request (never hang forever).

## 45.13 REST client checklist

1. **Base URL** stored once; paths appended.
2. **Auth header** (Bearer/API key) added to every request.
3. **Timeouts** on all requests.
4. **`raise_for_status`** or manual status checks.
5. **Parse JSON only when Content-Type is JSON.**
6. **Wrap errors** with method + path context.
7. **Retry** transient failures (optional).
8. **Pagination** support (optional: follow `Link` header or `page` param).

## 45.14 Exercises

1. Add `patch` method to the client.
2. Add automatic retry (3 attempts) on 5xx errors.
3. Add pagination: follow `next` URL until none.
4. Add query-parameter support to `get`.
5. Add file upload (`files=` param) support.
6. Add a `--verbose` flag printing each request URL.
7. Implement OAuth2 bearer token refresh (conceptually).
8. Write tests against a local mock server.
9. Add rate-limit handling (respect `Retry-After`).
10. Turn the client into a reusable module `api_client.z`.

