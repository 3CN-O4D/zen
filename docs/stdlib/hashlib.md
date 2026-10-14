# hashlib — hashing

Hashing in the spirit of Python's `hashlib`. Constructors return hasher
objects; feed them with `update` and read `digest` / `hexdigest`.

```zen
import hashlib

let h = hashlib.sha256("abc")
print h.hexdigest()  # ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
```

## Constructors

`md5`, `sha1`, `sha224`, `sha256`, `sha384`, `sha512`, `sha3_256`,
`sha3_512`, `blake2b`, `blake2s` — each takes optional initial `data`.
`new_(name, data)` selects by name; `algorithms_guaranteed` and
`algorithms_available` list names; `pbkdf2_hmac(hash_name, password, salt,
iterations, dklen)` derives keys.

## Hasher methods

| Method | Description |
|--------|-------------|
| `update(data)` | Feeds more bytes. |
| `digest()` | Raw digest bytes. |
| `hexdigest()` | Lowercase hex digest. |
| `copy()` | Independent clone of the hasher state. |

## Example

```zen
import hashlib

let h = hashlib.new_("sha256", "ab")
h.update("c")
print h.hexdigest() == hashlib.sha256("abc").hexdigest()  # true
```
