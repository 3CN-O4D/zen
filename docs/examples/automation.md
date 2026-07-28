# Automation Examples

## Login Automation

```
go "https://the-internet.herokuapp.com/login"
find("#username").fill("tomsmith")
find("#password").fill("SuperSecretPassword!")
click(".radius")
wait_for(".flash.success")

if "secure area" in page.text {
    print "LOGIN SUCCESSFUL"
}
```

## Form Fill

```
go "https://example.com/form"
fill("#name", "John Doe")
fill("#email", "john@example.com")
fill("#message", "Hello from Zen!")
select("#country", "US")
check("#terms")
click("button[type='submit']")
wait_for(".success-message")
```

## Multi-Step Workflow

```
function login(username, password) {
    go "https://example.com/login"
    fill("#username", username)
    fill("#password", password)
    click("button[type='submit']")
    wait_for(".dashboard")
    return true
}

function search(query) {
    fill("#search", query)
    click("button[type='submit']")
    wait_for(".results")
    return find_all(".result-item")
}

// Execute workflow
login("admin", "secret")
let results = search("automation")
print "Found " + results.count + " results"
```

## Data Entry

```
go "https://example.com/entry"

let entries = [
    {name: "Alice", email: "alice@example.com"},
    {name: "Bob", email: "bob@example.com"},
    {name: "Charlie", email: "charlie@example.com"}
]

for entry in entries {
    fill("#name", entry["name"])
    fill("#email", entry["email"])
    click("button[type='submit']")
    wait_for(".success")
    print "Added: " + entry["name"]
}
```

## Screenshot Workflow

```
go "https://example.com"
shot "homepage.png"

click("about")
wait_for(".content")
shot "about.png"

click("contact")
wait_for("form")
shot "contact.png"

print "Screenshots saved!"
```

## Monitor Changes

```
function check_page() {
    go "https://example.com/status"
    let status = find(".status").text
    return status
}

let last_status = check_page()
print "Initial status: " + last_status

while true {
    wait 60000  // check every minute
    let current = check_page()
    if current != last_status {
        print "STATUS CHANGED: " + current
        last_status = current
    }
}
```
