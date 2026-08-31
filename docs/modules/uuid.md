# uuid — UUID generation

The `uuid` module provides tools for generating Universally Unique Identifiers (UUIDs). It is available globally as `uuid`.

```zen
# Generate a random UUID (Version 4)
var id = uuid.v4()
print(id)  # e.g., 550e8400-e29b-41d4-a716-446655440000

# v4() is an alias for uuid4()
print(uuid.uuid4())
```

## Functions

| Function | Description |
|----------|-------------|
| `v4()` / `uuid4()` | Generates a random UUID (RFC 4122). |
| `v1()` / `uuid1()` | Generates a UUID based on host ID and time. |
| `v3(ns, name)` | Generates a MD5-hashed UUID based on a namespace and name. |
| `v5(ns, name)` | Generates a SHA1-hashed UUID based on a namespace and name. |

## Namespaces for v3 and v5
Commonly used namespaces are provided as constants:

- `uuid.NAMESPACE_DNS`
- `uuid.NAMESPACE_URL`
- `uuid.NAMESPACE_OID`
- `uuid.NAMESPACE_X500`

## Examples

### Deterministic UUIDs with v5
Use `v5` when you need the same UUID for the same input string.

```zen
var ns = uuid.NAMESPACE_URL
var id = uuid.v5(ns, "https://example.com")
print(id) # Always the same for this URL
```

## See Also
- [random](random.md) — For general random strings.
- [hashlib](hashlib.md) — For SHA/MD5 hashing.
