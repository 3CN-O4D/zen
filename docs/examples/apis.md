# API Examples

## Basic GET Request

```
let resp = http.get("https://api.github.com/repos/3CN-O4D/zen")
if resp.ok {
    let data = resp.json()
    print "Repo: " + data["full_name"]
    print "Stars: " + data["stargazers_count"]
    print "Description: " + data["description"]
}
```

## POST with JSON

```
let resp = http.post("https://httpbin.org/post",
    json={"name": "Zen", "version": "0.1.0"})

print resp.status    // 200
print resp.json()    // echoed data
```

## Custom Headers

```
let resp = http.get("https://api.github.com/user",
    headers={"Authorization": "Bearer ghp_xxxxxxxxxxxx"})

if resp.ok {
    let user = resp.json()
    print "Logged in as: " + user["login"]
}
```

## Fetch and Process

```
function fetch_users() {
    let resp = http.get("https://api.github.com/users")
    if resp.ok {
        return resp.json()
    }
    return []
}

let users = fetch_users()
for user in users {
    print user["login"] + " - " + user["html_url"]
}
```

## Pagination

```
let page = 1
let all_items = []

while true {
    let resp = http.get("https://api.example.com/items?page=" + page)
    let items = resp.json()

    if items.len == 0 {
        break
    }

    all_items = [...all_items, ...items]
    page = page + 1
}

print "Total items: " + all_items.len
```

## Error Handling

```
try {
    let resp = http.get("https://api.example.com/data")
    if !resp.ok {
        throw "HTTP error: " + resp.status
    }
    let data = resp.json()
    print data
} catch err {
    print "Request failed: " + err
}
```

## WhatsApp API Integration

```
load wa
conn = wa.connect()

// Fetch data from API
let resp = http.get("https://api.example.com/notifications")
let notifications = resp.json()

// Send via WhatsApp
for notif in notifications {
    conn.send(notif["jid"], notif["message"])
    print "Sent to: " + notif["jid"]
}
```
