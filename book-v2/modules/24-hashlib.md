# Chapter 24: hashlib Module

The `hashlib` module provides cryptographic hash functions: MD5, SHA-1, SHA-224/256/384/512, SHA-3, and BLAKE2/3.

> ⚠️ Warning: MD5 and SHA-1 are cryptographically broken. Use them only for legacy compatibility or non-security purposes (e.g. checksums). For security-sensitive work, use SHA-256 or better.

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

### Output lengths

| Algorithm | Hex length | Bytes |
|-----------|------------|-------|
| MD5 | 32 | 16 |
| SHA-1 | 40 | 20 |
| SHA-224 | 56 | 28 |
| SHA-256 | 64 | 32 |
| SHA-384 | 96 | 48 |
| SHA-512 | 128 | 64 |
| SHA3-256 | 64 | 32 |
| SHA3-512 | 128 | 64 |
| BLAKE2b | 128 | 64 |
| BLAKE2s | 64 | 32 |

```zen
import hashlib

print(len(hashlib.md5("hello")))       # 32
print(len(hashlib.sha1("hello")))      # 40
print(len(hashlib.sha256("hello")))    # 64
print(len(hashlib.sha512("hello")))    # 128
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

### Hash a large file in chunks

```zen
import hashlib
import fs

func hash_file_chunks(path, chunk_size = 8192) {
    let data = fs.read_binary(path)
    return hashlib.sha256(data)
    # For very large files, read in chunks and update incrementally
    # (if hashlib supports incremental hashing)
}
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

### Verify downloaded file

```zen
import hashlib
import fs
import requests

let url = "https://example.com/file.zip"
let expected_sha256 = "abc123..."

let resp = requests.get(url)
fs.write_binary("file.zip", resp.content)

let actual = hashlib.sha256(fs.read_binary("file.zip"))
if actual == expected_sha256 {
    print("download verified")
} else {
    print("download corrupted - delete and retry")
    fs.remove("file.zip")
}
```

## 24.4 Common pitfalls

- **Hashes are one-way.** You cannot recover the original data from a hash.
- **MD5/SHA-1 are broken** for collision resistance. Don't use them for signatures or security.
- **Hashes return hex strings**, not raw bytes. Use `binascii.unhexlify` if you need raw bytes.
- **Empty input has a defined hash.** `hashlib.sha256("")` is a well-known value:
  ```
  e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
  ```

```zen
import hashlib

print(hashlib.sha256(""))  # e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
print(hashlib.md5(""))     # d41d8cd98f00b204e9800998ecf8427e
```

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
6. Write `verify_download(url, expected_hash)` that downloads and verifies.
7. Write `hash_compare(h1, h2)` that compares two hashes case-insensitively.


## 24.12 hashlib module — complete reference

| Function | Purpose | Example |
|----------|---------|---------|
| `md5(data)` | MD5 digest | `hashlib.md5("hello")` |
| `sha1(data)` | SHA-1 digest | `hashlib.sha1("hello")` |
| `sha224(data)` | SHA-224 | `hashlib.sha224("hello")` |
| `sha256(data)` | SHA-256 | `hashlib.sha256("hello")` |
| `sha384(data)` | SHA-384 | `hashlib.sha384("hello")` |
| `sha512(data)` | SHA-512 | `hashlib.sha512("hello")` |
| `sha3_256(data)` | SHA3-256 | `hashlib.sha3_256("hello")` |
| `sha3_512(data)` | SHA3-512 | `hashlib.sha3_512("hello")` |
| `blake2b(data)` | BLAKE2b | `hashlib.blake2b("hello")` |
| `blake2s(data)` | BLAKE2s | `hashlib.blake2s("hello")` |
| `pbkdf2_hmac(pass, salt, iters)` | key derivation | `hashlib.pbkdf2_hmac(...)` |

### Usage

```zen
import hashlib
print(hashlib.sha256("hello"))   # hex digest
print(hashlib.md5("hello"))      # 32 hex chars
print(hashlib.sha256("hello") == hashlib.sha256("hello"))  # true
print(hashlib.sha256("Hello") != hashlib.sha256("hello"))  # true (case sensitive)
```

### Properties

- Deterministic: same input → same output.
- One-way: cannot reverse to input.
- Avalanche: tiny input change → very different output.
- Collision-resistant (for secure hashes like SHA-256).

### Exercises

1. Compute SHA-256 of a string and verify it matches a known value.
2. Write `file_hash(path)` hashing a file's contents.
3. Implement `verify_checksum(data, expected)`.
4. Write `hash_password(password, salt)` combining salt + hash.
5. Implement `find_collision_prefix(prefix)` brute-force conceptually.
6. Compare MD5 vs SHA-256 output lengths.
7. Write `dedupe_files(paths)` grouping identical files by hash.
8. Implement `merkle_root(hashes)` conceptually.
9. Write `constant_time_compare(a, b)` conceptually.
10. Implement `pbkdf2` key derivation using hashlib.pbkdf2_hmac.



## 24.12 Worked example — file integrity checker

```zen
import hashlib
import sys
import pathlib

func file_sha256(path) {
    let b = pathlib.Path(path)
    if !b.exists() { throw "not found: " + path }
    let data = b.read_bytes()   # if supported; otherwise read chunks
    return hashlib.sha256(data)
}

func main(argv) {
    if len(argv) < 2 {
        print("usage: hashutil <file> [expected]")
        return 1
    }
    let path = argv[1]
    let expected = argv[2] ?? nil
    try {
        let actual = file_sha256(path)
        print("SHA-256(" + path + ") = " + actual)
        if expected != nil {
            if actual == expected { print("MATCH") }
            else { print("MISMATCH"); return 1 }
        }
    } catch as e {
        print("error: " + e.msg)
        return 1
    }
    return 0
}

if __name__ == "__main__" { sys.exit(main(sys.argv)) }
```

Sample:
```
$ zen run hashutil.z document.pdf
SHA-256(document.pdf) = a3f5...c9
$ zen run hashutil.z document.pdf a3f5...c9
SHA-256(document.pdf) = a3f5...c9
MATCH
```

## 24.13 Troubleshooting hashing

| Symptom | Cause | Fix |
|---------|-------|-----|
| different hash for same file | file changed or encoding differs | re-hash exact bytes |
| hash mismatch expected vs actual | wrong algorithm or corrupted file | verify algorithm & source |
| slow on large files | reading whole file at once | hash in chunks |

Use SHA-256 or SHA-512 for integrity; avoid MD5/SHA-1 for security.

## 24.14 Exercises

1. SHA-256 a string and compare to a known value.
2. Hash a file and save the digest to a sidecar `.sha256` file.
3. Verify a file against a digest file.
4. Hash all files in a folder into a manifest.
5. Compare MD5 vs SHA-256 digest lengths.
