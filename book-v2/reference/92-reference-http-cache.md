# Chapter 92: Reference Implementation — HTTP Response Cache (with TTL)

A small HTTP reverse-proxy cache: it forwards GET requests to an origin server,
caches the response with a TTL, and serves subsequent requests from cache
within the TTL. Demonstrates `requests`, `time`, `mutexes`, dict caching, and
conditional forwarding.

## 92.1 The program (`cache.z`)

```zen
import requests
import time
import mutexes
import sys

var cache = #{}
let cache_mu = mutexes.Mutex()
let TTL_SEC = 60

func cache_get(key) {
    cache_mu.lock()
    let entry = cache[key]
    cache_mu.unlock()
    if entry == nil { return nil }
    let now = time.time()
    if now - entry["stored_at"] > TTL_SEC {
        # expired
        cache_mu.lock()
        if cache[key] is entry { cache[key] = nil }   # remove if still same
        cache_mu.unlock()
        return nil
    }
    return entry["body"]
}

func cache_set(key, body) {
    cache_mu.lock()
    cache[key] = #{body: body, stored_at: time.time()}
    cache_mu.unlock()
}

func fetch(url) {
    let body = cache_get(url)
    if body != nil {
        return #{body: body, from_cache: true}
    }
    try {
        let r = requests.get(url, timeout=5000)
        cache_set(url, r.text)
        return #{body: r.text, from_cache: false, status: r.status_code}
    } catch as e {
        return #{body: nil, error: e.msg}
    }
}

func main(argv) {
    if len(argv) < 2 {
        print("Usage: zen run cache.z <url>")
        print("  Example: zen run cache.z https://example.com")
        return 1
    }
    let url = argv[1]

    print("first fetch (from origin):")
    let r1 = fetch(url)
    if r1["body"] == nil { print("error: " + str(r1["error"])); return 1 }
    print("  from_cache: " + str(r1["from_cache"]) + "  len: " + str(len(r1["body"])))

    print("second fetch (from cache):")
    let r2 = fetch(url)
    print("  from_cache: " + str(r2["from_cache"]) + "  len: " + str(len(r2["body"])))
    print("  match: " + str(r1["body"] == r2["body"]))

    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 92.2 Example session

```
$ zen run cache.z https://example.com
first fetch (from origin):
  from_cache: false  len: 1256
second fetch (from cache):
  from_cache: true  len: 1256
  match: true
```

## 92.3 Design notes

- **TTL-based expiry:** entries older than `TTL_SEC` are treated as stale and
  refetched from origin.
- **Mutex-guarded cache:** the dict is shared; reads and writes go through
  `cache_mu`.
- **Cache hit/miss:** `from_cache` tells you whether the result came from the
  cache or the origin.
- **In-memory only:** cache is lost on restart; for persistence, serialize to
  disk (see chapter 62/72).

## 92.4 Exercises

1. Add `--ttl N` to configure the TTL in seconds.
2. Add `--max-entries N` to cap the cache size (evict oldest when full).
3. Add cache statistics: hits, misses, evictions.
4. Add `POST`/`PUT` support with cache-by-request-body-hash.
5. Add cache invalidation: `DELETE /cache/<url>` (if wrapped in an HTTP server).
