# Chapter 85: Reference Implementation — Base64 Encode/Decode Tool

Encodes a file to base64 or decodes a base64 string/file back to bytes. Supports
standard and URL-safe base64. Demonstrates the `base64` module, binary file I/O,
and CLI flags.

## 85.1 The program (`b64.z`)

```zen
import base64
import sys

func read_all_bytes(path) {
    with open(path, "rb") as f {
        let parts = []
        while true {
            let chunk = f.read(8192)
            if chunk == nil or len(chunk) == 0 { break }
            parts.push(chunk)
        }
        return parts.join("")
    }
}

func write_all_bytes(path, data) {
    with open(path, "wb") as f { f.write(data) }
}

func main(argv) {
    if len(argv) < 2 {
        print("Usage:")
        print("  zen run b64.z encode <file> [urlsafe]      # encode file to base64")
        print("  zen run b64.z decode <string> [urlsafe]    # decode base64 string to bytes")
        print("  zen run b64.z decode-file <file> [urlsafe] # decode base64 file")
        return 1
    }
    let mode = argv[1]
    let urlsafe = len(argv) >= 3 and argv[2] == "urlsafe"

    if mode == "encode" {
        if len(argv) < 3 {
            print("usage: b64.z encode <file> [urlsafe]")
            return 1
        }
        let data = read_all_bytes(argv[2])
        let enc = if urlsafe { base64.url_encode(data) } else { base64.b64encode(data) }
        print(enc)
    } elif mode == "decode" {
        if len(argv) < 3 {
            print("usage: b64.z decode <string> [urlsafe]")
            return 1
        }
        let dec = if urlsafe { base64.url_decode(argv[2]) } else { base64.b64decode(argv[2]) }
        print(dec)
    } elif mode == "decode-file" {
        if len(argv) < 3 {
            print("usage: b64.z decode-file <file> [urlsafe]")
            return 1
        }
        let text = read_all(argv[2])   # read as text
        let dec = if urlsafe { base64.url_decode(text) } else { base64.b64decode(text) }
        write_all_bytes("decoded.out", dec)
        print("decoded -> decoded.out")
    } else {
        print("unknown mode: " + mode)
        return 1
    }
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 85.2 Example session

```
$ zen run b64.z encode secret.png
iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAY... (long base64 string)

$ zen run b64.z encode secret.png urlsafe
iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAY...-urlsafe-variant

$ zen run b64.z decode "aGVsbG8gd29ybGQ="
hello world

$ zen run b64.z decode-file message.b64
decoded -> decoded.out
```

## 85.3 Design notes

- **Standard vs URL-safe:** `urlsafe` uses `-` and `_` instead of `+` and `/`
  so the result is safe in URLs and filenames.
- **Binary-safe:** files are read/written in binary mode (`rb`/`wb`), so any
  file type (images, executables) works.
- **`decode-file`** writes the decoded bytes to `decoded.out`.

## 85.4 Exercises

1. Add `--output <file>` to `encode` so the result goes to a file.
2. Add `--wrap N` to break encoded output into lines of N characters.
3. Add `--ignore-garbage` to skip non-base64 characters when decoding.
4. Add a `--check` mode that verifies a string is valid base64.
5. Support reading the string to decode from stdin.
