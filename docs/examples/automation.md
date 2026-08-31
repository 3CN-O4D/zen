# Automation Examples

These examples demonstrate how to use Zen's `browser` module for common automation tasks.

## Login Automation

```zen
# 1. Launch and navigate
browser.launch({headless: true})
browser.navigate("https://the-internet.herokuapp.com/login")

# 2. Fill credentials and click login
browser.fill("#username", "tomsmith")
browser.fill("#password", "SuperSecretPassword!")
browser.click(".radius")

# 3. Wait for success message
browser.wait_for(".flash.success")

# 4. Verify login state
if browser.page_text().contains("secure area") {
    print("LOGIN SUCCESSFUL")
}

browser.close()
```

## Form Fill

```zen
browser.launch()
browser.navigate("https://example.com/form")

# Filling various fields
browser.fill("#name", "John Doe")
browser.fill("#email", "john@example.com")
browser.fill("#message", "Hello from Zen!")

# Click submit and wait
browser.click("button[type='submit']")
browser.wait_for(".success-message")

print("Form submitted successfully")
browser.close()
```

## Multi-Step Workflow

```zen
func login(username, password) {
    browser.navigate("https://example.com/login")
    browser.fill("#username", username)
    browser.fill("#password", password)
    browser.click("button[type='submit']")
    browser.wait_for(".dashboard")
    return true
}

func search(query) {
    browser.fill("#search", query)
    browser.click("button[type='submit']")
    browser.wait_for(".results")
    return browser.query(".result-item") # returns all matching texts
}

# Execute workflow
browser.launch()
login("admin", "secret")
var results = search("automation")
print("Found ${results.len} results")
browser.close()
```

## Screenshot Workflow

```zen
browser.launch()

# Homepage
browser.navigate("https://example.com")
browser.screenshot("homepage.png")

# About page
browser.click("a[href='/about']")
browser.wait_for(".content")
browser.screenshot("about.png")

# Contact page
browser.click("a[href='/contact']")
browser.wait_for("form")
browser.screenshot("contact.png")

print("Screenshots saved!")
browser.close()
```

## See Also
- [Browser Overview](../browser/overview.md) — The complete CDP module reference.
- [Interacting](../browser/interacting.md) — More on clicks, fills, and form handling.
- [Screenshots](../browser/screenshots.md) — Advanced screenshot options.
