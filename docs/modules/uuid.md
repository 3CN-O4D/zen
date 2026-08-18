# UUID Module (`uuid`)

Generate UUIDs (v1, v3, v4, v5).

```zen
uuid.uuid4()                  // random UUID v4
uuid.v4()                     // alias

uuid.uuid1()                  // time-based v1
uuid.v1()                     // alias

uuid.uuid3(uuid.NAMESPACE_DNS, "example.com")
uuid.v3(uuid.NAMESPACE_DNS, "example.com")

uuid.uuid5(uuid.NAMESPACE_URL, "https://example.com")
uuid.v5(uuid.NAMESPACE_URL, "https://example.com")

// Constants
uuid.NAMESPACE_DNS
uuid.NAMESPACE_URL
uuid.NAMESPACE_OID
uuid.NAMESPACE_X500
```
