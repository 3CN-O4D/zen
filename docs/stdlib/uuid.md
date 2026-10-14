# uuid — unique identifiers

UUIDs in the spirit of Python's `uuid`. UUIDs print as dicts with `hex`,
`int`, `str`, `urn`, `bytes`, `version`, and `variant` fields.

```zen
import uuid

print uuid.uuid4().str
# 10a71b89-6b7e-4952-a580-8c88e0e59c0a
```

## Functions

| Function | Description |
|----------|-------------|
| `uuid4()` | Random (v4) UUID. |
| `uuid1(node, clock_seq)` | Time-based (v1) UUID. |
| `uuid3(namespace, name)` | MD5-based (v3) UUID. |
| `uuid5(namespace, name)` | SHA-1-based (v5) UUID. |
| `UUID(other)` | Normalizes/coerces a UUID value. |
| `uuid1_fields()` | Field breakdown of a v1 UUID. |

## Constants

`NAMESPACE_DNS`, `NAMESPACE_URL`, `NAMESPACE_OID`, `NAMESPACE_X500`.

## Example

```zen
import uuid

# Deterministic: same namespace + name, same UUID.
print uuid.uuid3(uuid.NAMESPACE_DNS, "example.com").str
print uuid.uuid5(uuid.NAMESPACE_DNS, "example.com").str
```
