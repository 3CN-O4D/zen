# Chapter 24: hashlib Module

The `hashlib` module provides cryptographic hash functions: MD5, SHA-1,
SHA-224/256/384/512, SHA-3, and BLAKE2/3.

> WARNING:️ Warning: MD5 and SHA-1 are cryptographically broken. Use them only for
> legacy compatibility or non-security purposes (e.g. checksums). For
> security-sensitive work, use SHA-256 or better.

## 24.1 Hash functions

| Function | Description | Example |
|----------|-------------|---------|
| `md5(data)` | MD5 digest | `hashlib.md5("hello")` |
| `sha1(data)` | SHA-1 digest | `hashlib.sha1("hello")` |
| `sha224(data)` | SHA-224 digest | `hashlib.sha224("hello")` |
| `sha256(data)` | SHA-256 digest | `hashlib.sha256("hello")` |
| `sha384(data)` | SHA-384 digest | `hashlib.sha384("hello")` |
| `sha512(data)` | SHA-512 digest | `hashlib.sha512("hello")` |
| `sha3_256(data)` | SHA3-256 digest | `hashlib.sha3_256("hello")` |
| `sha3_512(data)` | SHA3-512 digest | `hashlib.sha3_512("hello")` |
| `blake2b(data)` | BLAKE2b digest | `hashlib.blake2b("hello")` |
| `blake2s(data)` | BLAKE2s digest | `hashlib.blake2s("hello")` |

All return lowercase hex strings.

```zen
import hashlib

print(hashlib.md5("hello"))       # 5d41402abc4b2a76b9719d911017c592
print(hashlib.sha256("hello"))    # 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
print(hashlib.sha3_256("hello"))  # SHA3-256 hex
print(hashlib.blake2b("hello"))   # BLAKE2b hex
```

## 24.2 File hashing

```zen
import hashlib
import fs

func hash_file(path) {
    let data = fs.read_binary(path)
    return hashlib.sha256(data)
}

let digest = hash_file("document.pdf")
print(digest)
```

## 24.3 Verifying file integrity

```zen
import hashlib
import fs

let expected = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
let actual = hashlib.sha256(fs.read_binary("file.txt"))

if actual == expected {
    print("file intact")
} else {
    print("file corrupted or tampered")
}
```

## 24.4 Common pitfalls

- **Hashes are one-way.** You cannot recover the original data from a hash.
- **MD5/SHA-1 are broken** for collision resistance. Don't use them for
  signatures or security.
- **Hashes return hex strings**, not raw bytes. Use `binascii.unhexlify` if
  you need raw bytes.
- **Empty input has a defined hash.** `hashlib.sha256("")` is a well-known
  value.

## 24.5 Quick reference

```zen
import hashlib

hashlib.md5("hello")        # md5 hex
hashlib.sha1("hello")       # sha1 hex
hashlib.sha256("hello")     # sha256 hex
hashlib.sha384("hello")     # sha384 hex
hashlib.sha512("hello")     # sha512 hex
hashlib.sha3_256("hello")   # sha3-256 hex
hashlib.sha3_512("hello")   # sha3-512 hex
hashlib.blake2b("hello")    # blake2b hex
hashlib.blake2s("hello")    # blake2s hex
```

## 24.6 Exercises

1. Write `file_fingerprint(path)` that returns the SHA-256 of a file.
2. Write `verify_checksum(path, expected)` that compares a file's hash.
3. Write `find_duplicates(files)` that groups files by identical hash.
4. Write a script that hashes all files in a directory and writes a manifest.
5. Compare `hashlib.md5` vs `hashlib.sha256` output lengths.
