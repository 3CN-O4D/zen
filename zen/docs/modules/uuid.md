# UUID Module (`uuid`)

Generate UUIDs.

```zen
uuid.uuid4()                  // random UUID v4
uuid.v4()                     // alias
uuid.uuid1()                  // time-based v1
uuid.v1()                     // alias
uuid.uuid3(uuid.NAMESPACE_DNS, "example.com")
uuid.v3(uuid.NAMESPACE_DNS, "example.com")
uuid.uuid5(uuid.NAMESPACE_URL, "https://example.com")
uuid.v5(uuid.NAMESPACE_URL, "https://example.com")
```
Constants: `NAMESPACE_DNS`, `NAMESPACE_URL`, `NAMESPACE_OID`, `NAMESPACE_X500`.
