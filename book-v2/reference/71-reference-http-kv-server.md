# Chapter 71: Reference Implementation — HTTP Key-Value REST Server

A small HTTP key-value store with a REST interface: `GET /keys/<name>`,
`PUT /keys/<name>` with a JSON body, `DELETE /keys/<name>`, and `GET /keys`
to list all keys. Demonstrates `http.server`, JSON, routing, mutexes, and
CLI flags.

## 71.1 The program (`httpkv.z`)

```zen
import http.server
import json
import mutexes
import sys

var store = #{}
let store_mu = mutexes.Mutex()

func kv_get(name) {
    store_mu.lock()
    let v = store[name]
    store_mu.unlock()
    return v
}

func kv_set(name, value) {
    store_mu.lock()
    store[name] = value
    store_mu.unlock()
}

func kv_del(name) {
    store_mu.lock()
    let existed = name in store
    if existed {
        let new_store = #{}
        for k in store { if k != name { new_store[k] = store[k] } }
        store = new_store
    }
    store_mu.unlock()
    return existed
}

func kv_list() {
    store_mu.lock()
    let ks = []
    for k in store { ks.push(k) }
    store_mu.unlock()
    return ks
}

# JSON helpers
func json_response(code, obj) {
    let body = json.stringify(obj)
    return #(code, #{"Content-Type": "application/json"}, body)
}

func text_response(code, body) {
    return #(code, #{"Content-Type": "text/plain"}, body)
}

# Route handler
func handle(req) {
    let path = req.path ?? "/"
    let method = req.method ?? "GET"

    # GET /keys  -> list all keys
    if method == "GET" and path == "/keys" {
        return json_response(200, #{keys: kv_list()})
    }

    # GET /keys/<name> / PUT /keys/<name> / DELETE /keys/<name>
    let m = /^\/keys\/(.+)$/.match(path)   # conceptual regex match
    if m != nil {
        let name = m[1]
        if method == "GET" {
            let v = kv_get(name)
            if v == nil { return json_response(404, #{error: "not found"}) }
            return json_response(200, #{name: name, value: v})
        } elif method == "PUT" {
            let body = req.body ?? ""
            let parsed = nil
            try { parsed = json.parse(body) } catch as e {}
            if parsed == nil { return json_response(400, #{error: "invalid json"}) }
            kv_set(name, parsed)
            return json_response(200, #{ok: true})
        } elif method == "DELETE" {
            let existed = kv_del(name)
            if existed { return json_response(200, #{ok: true}) }
            else { return json_response(404, #{error: "not found"}) }
        }
    }

    return text_response(404, "not found")
}

func main(argv) {
    let port = argv[1] ?? "8080"
    let server = http.server.Server(port=int(port), handler=handle)
    print("http-kv server on http://localhost:" + str(port))
    server.start()
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 71.2 Example session

Terminal 1 (server):
```
$ zen run httpkv.z 8080
http-kv server on http://localhost:8080
```

Terminal 2 (client, curl):
```bash
# list keys (empty)
$ curl http://localhost:8080/keys
{"keys":[]}

# create a key
$ curl -X PUT http://localhost:8080/keys/foo -H 'Content-Type: application/json' -d '"bar"'
{"ok":true}

# read a key
$ curl http://localhost:8080/keys/foo
{"name":"foo","value":"bar"}

# read missing key
$ curl http://localhost:8080/keys/missing
{"error":"not found"}

# list keys again
$ curl http://localhost:8080/keys
{"keys":["foo"]}

# delete a key
$ curl -X DELETE http://localhost:8080/keys/foo
{"ok":true}
$ curl http://localhost:8080/keys
{"keys":[]}
```

## 71.3 Design notes

- **Mutex-guarded store:** concurrent HTTP handlers share the dict safely.
- **JSON bodies:** `PUT` expects a JSON value as the request body; responses
  are JSON with `Content-Type: application/json`.
- **Routing:** crude path-prefix matching (`/keys` vs `/keys/<name>`); real
  apps use a proper router (see chapter 30).
- **Status codes:** 200 success, 400 invalid JSON, 404 not found.

## 71.4 Exercises

1. Add `POST /keys` to create a key only if it doesn't exist (return 409 on
   conflict).
2. Add value expiration (TTL) with lazy cleanup on read.
3. Add persistence: snapshot to disk periodically and load at startup (see ch 62).
4. Add basic auth: require an `Authorization` header for writes.
5. Add content negotiation: return plain text when `Accept: text/plain`.
