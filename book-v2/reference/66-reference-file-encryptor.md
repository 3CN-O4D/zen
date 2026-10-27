# Chapter 66: Reference Implementation — File Encryptor (AES-GCM)

A small file encryption tool using AES-GCM authenticated encryption. It reads a
file, encrypts it with a user-supplied passphrase (derived via PBKDF2/Argon2id),
and writes an encrypted output file with a header storing the nonce and salt.
It demonstrates the `crypto` module, file I/O, bytes handling, and CLI flags.

## 66.1 The program (`encrypt.z`)

```zen
import crypto
import sys
import os
import pathlib

# ── Format (binary) ─────────────────────────────────────────────────────────
# <magic: 4 bytes "ZENC"><salt: 16 bytes><nonce: 12 bytes><ciphertext: ...>
let MAGIC = bytes([0x5A, 0x45, 0x4E, 0x43])  # "ZENC"
let SALT_LEN = 16
let NONCE_LEN = 12

func derive_key(passphrase, salt) {
    return crypto.pbkdf2(passphrase, salt, 100000, 32)
}

func encrypt_file(input_path, output_path, passphrase) {
    let salt = crypto.bytes(SALT_LEN)   # random salt
    let nonce = crypto.bytes(NONCE_LEN) # random nonce
    let key = derive_key(passphrase, salt)

    let plaintext = read_all_bytes(input_path)
    let ciphertext = crypto.encrypt_aes_gcm(key, plaintext, nonce)

    with open(output_path, "wb") as f {
        f.write(MAGIC)
        f.write(salt)
        f.write(nonce)
        f.write(ciphertext)
    }
    print("encrypted " + input_path + " -> " + output_path)
}

func decrypt_file(input_path, output_path, passphrase) {
    with open(input_path, "rb") as f {
        let magic = f.read(4)
        if magic == nil or len(magic) != 4 or magic != MAGIC {
            throw "not a ZENC file (bad magic)"
        }
        let salt = f.read(SALT_LEN)
        let nonce = f.read(NONCE_LEN)
        let ciphertext = f.read()   # rest of file
        let key = derive_key(passphrase, salt)
        let plaintext = crypto.decrypt_aes_gcm(key, ciphertext, nonce)
        write_all_bytes(output_path, plaintext)
    }
    print("decrypted " + input_path + " -> " + output_path)
}

func read_all_bytes(path) {
    with open(path, "rb") as f {
        let parts = []
        while true {
            let chunk = f.read(8192)
            if chunk == nil or len(chunk) == 0 { break }
            parts.push(chunk)
        }
        return parts.join("")   # conceptual byte join
    }
}

func write_all_bytes(path, data) {
    with open(path, "wb") as f { f.write(data) }
}

func main(argv) {
    if len(argv) < 4 {
        print("Usage:")
        print("  zen run encrypt.z enc <input> <output> <passphrase>")
        print("  zen run encrypt.z dec <input> <output> <passphrase>")
        return 1
    }
    let mode = argv[1]
    let input_path = argv[2]
    let output_path = argv[3]
    let passphrase = argv[4]

    try {
        if mode == "enc" {
            encrypt_file(input_path, output_path, passphrase)
        } elif mode == "dec" {
            decrypt_file(input_path, output_path, passphrase)
        } else {
            print("unknown mode: " + mode + " (use enc or dec)")
            return 1
        }
    } catch as e {
        print("error: " + e.msg)
        return 1
    }
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 66.2 Example session

```
$ zen run encrypt.z enc secret.txt secret.enc hunter2
encrypted secret.txt -> secret.enc
$ zen run encrypt.z dec secret.enc secret.dec hunter2
decrypted secret.enc -> secret.dec
$ diff secret.txt secret.dec
(no output — files identical)
```

Using a wrong passphrase produces a decryption error (AES-GCM authentication
fails):
```
$ zen run encrypt.z dec secret.enc secret.dec wrongpass
error: authentication failed (or corrupt ciphertext)
```

## 66.3 Design notes

- **PBKDF2** with 100,000 iterations derives a 256-bit key from the passphrase
  and a random salt, making brute-force attacks expensive.
- **AES-GCM** provides both confidentiality and authenticity; tampering with
  the ciphertext is detected on decrypt.
- **Random salt and nonce** per file ensure the same passphrase + plaintext
  produces different ciphertext each time.
- The header format (`MAGIC || salt || nonce || ciphertext`) is self-describing.

## 66.4 Security notes

- Never hardcode passphrases in scripts; read them from the environment or a
  secure prompt.
- PBKDF2 iteration count (100,000) should be tuned to your hardware; higher is
  slower but more resistant to brute force.
- This tool encrypts file contents, not filenames or metadata.

## 66.4 Exercises

1. Add `--iterations N` to configure PBKDF2 cost.
2. Add Argon2id option (`crypto.argon2id`) as an alternative KDF.
3. Add streaming (chunked) encryption for large files instead of reading all
   into memory.
4. Add `--salt` to reuse a deterministic salt (for testing only).
5. Add integrity manifest: store SHA-256 of original file in header.
