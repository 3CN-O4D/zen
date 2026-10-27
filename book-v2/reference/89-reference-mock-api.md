# Chapter 89: Reference Implementation — JSON API Mock Server

An HTTP mock server that returns canned JSON responses based on the request
path and method. Useful for front-end development and testing without a real
backend. Demonstrates `http.server`, JSON, routing, and a response registry.

## 89.1 The program (`mock.z`)

```zen
import http.server
import json
import sys

# Routes: method + path -> #{status, body, headers}
var routes = #{}

func mock(method, path, status, body, headers) {
    let key = method + " " + path
    routes[key] = #{status: status, body: body, headers: headers ?? #{}}
}

func handle(req) {
    let method = req.method ?? "GET"
    let path = req.path ?? "/"
    let key = method + " " + path

    if key in routes {
        let r = routes[key]
        let body = json.stringify(r["body"])
        let h = #{"Content-Type": "application/json"}
        # merge custom headers
        for k in r["headers"] { h[k] = r["headers"][k] }
        return #(r["status"], h, body)
    }

    return #(404, #{"Content-Type": "application/json"},
             json.stringify(#{error: "not found"}))
}

func main(argv) {
    # register mock endpoints
    mock("GET", "/api/users", 200,
         #[#{id: 1, name: "Ada"}, #{id: 2, name: "Bob"}], #{})
    mock("GET", "/api/users/1", 200, #{id: 1, name: "Ada", role: "admin"}, #{})
    mock("POST", "/api/login", 200, #{token: "abc123", user: "Ada"}, #{})
    mock("GET", "/api/status", 200, #{status: "ok", uptime: 3600}, #{})
    mock("GET", "/api/error", 500, #{error: "internal error"}, #{})

    let port = argv[1] ?? "9000"
    let server = http.server.Server(port=int(port), handler=handle)
    print("mock API server on http://localhost:" + str(port))
    print("endpoints: GET /api/users, GET /api/users/1, POST /api/login, GET /api/status, GET /api/error")
    server.start()
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 89.2 Example session

```
$ zen run mock.z 9000
mock API server on http://localhost:9000
endpoints: GET /api/users, ...
```

```bash
$ curl http://localhost:9000/api/users
[{"id":1,"name":"Ada"},{"id":2,"name":"Bob"}]

$ curl http://localhost:9000/api/users/1
{"id":1,"name":"Ada","role":"admin"}

$ curl -X POST http://localhost:9000/api/login -d '{"user":"Ada","pass":"x"}'
{"token":"abc123","user":"Ada"}

$ curl -i http://localhost:9000/api/missing
HTTP/1.1 404 Not Found
{"error":"not found"}

$ curl http://localhost:9000/api/error
{"error":"internal error"}   # status 500
```

## 89.3 Design notes

- **Response registry:** `mock()` registers a method+path → response mapping.
- **JSON bodies:** all responses are JSON with `Content-Type: application/json`.
- **Extensible:** add query-parameter matching, request-body inspection, or
  response delays to simulate latency.

## 89.4 Exercises

1. Add query-parameter matching: `GET /api/users?role=admin` returns only admins.
2. Add response delay: simulate latency with `time.sleep`.
3. Add request logging: log each request's method, path, and status.
4. Add `PUT`/`DELETE` endpoints for a mock CRUD API.
5. Load routes from a JSON config file instead of hardcoding.
