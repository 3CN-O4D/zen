# API and Networking Examples

Zen can interact with REST APIs and perform low-level networking tasks.

## REST API Client

Using the `http` module to interact with a JSON API.

```zen
# 1. GET request with JSON parsing
var resp = http.get("https://api.github.com/repos/rust-lang/rust")
if resp.ok {
    var data = resp.json()
    print("Project: ${data.full_name}")
    print("Description: ${data.description}")
    print("Stars: ${data.stargazers_count}")
}

# 2. Authenticated POST request
var token = os.getenv("GITHUB_TOKEN")
var payload = json.stringify({
    title: "New Issue from Zen",
    body: "This issue was created using Zen automation."
})

var res = http.post("https://api.github.com/repos/user/repo/issues", {
    headers: {
        "Authorization": "token " + token,
        "Accept": "application/vnd.github.v3+json"
    },
    json: payload
})

if res.ok {
    print("Issue created successfully!")
}
```

## Concurrent API Requests

Using the `threading` module to fetch data in parallel.

```zen
var results = []

func fetch_status(url) {
    try {
        var r = http.get(url, {timeout: 5000})
        results = results.push("${url}: ${r.status}")
    } catch as e {
        results = results.push("${url}: FAILED")
    }
}

var sites = [
    "https://google.com",
    "https://github.com",
    "https://bing.com"
]

var threads = []
for site in sites {
    threads = threads.push(threading.start(fn() { fetch_status(site) }))
}

# Wait for all checks to complete
for t in threads {
    threading.join(t)
}

for res in results {
    print(res)
}
```

## Low-Level Port Scanner

Using the `socket` module to check for open ports.

```zen
var target = "127.0.0.1"
var ports = [22, 80, 443, 3000, 8080]

print("Scanning ${target}...")

for port in ports {
    try {
        var s = socket.open(target, port)
        print("PORT ${port}: OPEN")
        s.close()
    } catch as e {
        # Connection refused or timeout
        print("PORT ${port}: CLOSED")
    }
}
```

## See Also
- [http Module](../modules/http.md) — Full HTTP client reference.
- [json Module](../modules/json.md) — Working with JSON data.
- [socket Module](../modules/socket.md) — Raw TCP/UDP networking.
- [threading Module](../modules/threading.md) — Concurrent execution.
