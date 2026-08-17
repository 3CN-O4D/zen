# HTTP Module (`http`)

HTTP client.

```zen
let resp = http.get("https://api.github.com")
print resp.status             // HTTP status code
print resp.body               // response body string
resp.json()                   // parse body as JSON

http.post("https://example.com/api", "payload")
http.put("https://example.com/api", "payload")
http.del("https://example.com/item/1")
http.head("https://example.com")
http.patch("https://example.com/item/1", "field=new")
```
