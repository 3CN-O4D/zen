# Modules Overview

Complete guide to Zen's module system — how modules work, available modules, and import patterns.

## How Modules Work

Zen provides modules as global dictionaries — **no import statement is required** for built-in modules. They are available directly:

```
fs.read("file.txt")
http.get("https://example.com")
crypto.sha256("data")
json.parse('{"key": "value"}')
```

### Two access styles

Most modules support both dot-notation and flat function names:

```
// Module style (recommended)
json.parse('{"a": 1}')

// Flat style (legacy alias)
json_parse('{"a": 1}')

// Both return the same result
```

---

## All Available Modules

### Core Modules

| Module | Description | Example |
|--------|-------------|---------|
| `json` | JSON encode/decode | `json.parse("{}")` |
| `fs` | Filesystem operations | `fs.read("file.txt")` |
| `http` | HTTP client | `http.get("https://...")` |
| `re` | Regular expressions | `re.search("\\d+", "abc123")` |
| `crypto` | Cryptographic hashes | `crypto.sha256("data")` |
| `datetime` | Date/time operations | `datetime.now()` |
| `time` | Time functions | `time.now()` |
| `math` | Math functions | `math.sqrt(144)` |
| `random` | Random numbers | `random.randint(1, 100)` |
| `os` | OS info and process | `os.platform()` |
| `base64` | Base64 encoding | `base64.encode("hello")` |
| `base32` | Base32 encoding | `base32.encode("hello")` |
| `uuid` | UUID generation | `uuid.uuid4()` |
| `color` | ANSI color helpers | `color.red("error")` |
| `csv` | CSV processing | `csv.read("data.csv")` |
| `decimal` | Decimal arithmetic | `decimal.Decimal("3.14")` |
| `statistics` | Statistical functions | `statistics.mean([1,2,3])` |
| `threading` | Background execution | `threading.start(fn)` |

### Network Modules

| Module | Description | Example |
|--------|-------------|---------|
| `socket` | Low-level TCP sockets | `socket.open("host", 80)` |
| `ftp` | FTP client | `ftp.connect("host")` |
| `smtp` | SMTP email client | `smtp.connect("host")` |
| `pop3` | POP3 email client | `pop3.connect("host")` |
| `imap` | IMAP email client | `imap.connect("host")` |
| `telnet` | Telnet client | `telnet.connect("host")` |
| `dns` | DNS resolver | `dns.resolve("example.com")` |
| `ssh` | SSH/SCP wrapper | `ssh.run("host", "ls")` |
| `scapy` | Packet crafting | `scapy.ip(dst="1.1.1.1")` |

### Python-backed Modules

| Module | Description | Example |
|--------|-------------|---------|
| `errors` | Error class hierarchy | `errors.define("MyErr", "Error")` |
| `string` | String helpers | `string.upper("hello")` |
| `hashlib` | Crypto hashing | `hashlib.sha256("data")` |
| `struct` | Binary pack/unpack | `struct.pack("i", 42)` |
| `shutil` | High-level file ops | `shutil.copy(src, dst)` |
| `pathlib` | Path manipulation | `pathlib.Path("file.txt")` |
| `glob` | File pattern matching | `glob.glob("*.txt")` |
| `urllib` | URL handling | `urllib.parse.quote("hello")` |
| `collections` | Data structures | `collections.Counter([1,1,2])` |
| `itertools` | Iterators | `itertools.chain(a, b)` |
| `tempfile` | Temporary files | `tempfile.mkdtemp()` |
| `binascii` | Binary/ASCII | `binascii.hexlify(data)` |
| `subprocess` | External commands | `subprocess.run(["ls"])` |
| `cryptography` | Fernet encryption | `cryptography.fernet.generate_key()` |
| `emoji` | Emoji lookup | `emoji.smiley` |
| `net` | Network info | `net.online()` |
| `storage` | localStorage | `storage.get("key")` |
| `cookies` | Browser cookies | `cookies.get("session")` |

### Special Modules

| Module | Description | Example |
|--------|-------------|---------|
| `wa` | WhatsApp client | `wa.connect("auth_dir")` |

---

## Import Patterns

### Import a module (namespaced)

```
import utils
print utils.add(2, 3)
```

### Import with alias

```
import utils as u
print u.add(2, 3)
```

### From-import (specific items)

```
from utils import add, subtract
print add(2, 3)
```

### From-import with alias

```
from utils import add as a, subtract as s
print a(2, 3)
```

### Star import (all items)

```
from utils import *
// All non-underscore items are imported
```

### Importing file modules

```
// In lib/math.z:
function square(x) {
    return x * x
}

// In main.z:
import math
print math.square(5)    // 25
```

### Importing dotted modules

```
// In pkg/utils.z:
function helper() { return 42 }

// In main.z:
import pkg.utils
print pkg.utils.helper()
```

---

## Module Resolution

### How Zen finds modules

1. **Built-in modules** — checked first (json, fs, http, etc.)
2. **Stdlib factories** — lazy-loaded Python-backed modules
3. **File resolution** — looks for `<name>.z` or `<name>/main.z`
4. **Dotted paths** — `pkg.sub` resolves to `pkg/sub.z`

### Module search order

```
// For `import utils`:
// 1. Check if "utils" is a built-in module
// 2. Check if "utils" is a dict already in scope
// 3. Look for utils.z in the current directory
// 4. Look for utils/main.z
// 5. Look in lib/ directory
```

---

## Include vs Import

### `include` — inline code injection

```
// utils.z
function greet(name) {
    return "Hello, " + name + "!
}

// main.z
include "utils.z"
print greet("World")    // functions are in global scope
```

### `import` — namespaced module

```
// utils.z
function greet(name) {
    return "Hello, " + name + "!
}

// main.z
import utils
print utils.greet("World")    // namespaced access
```

### When to use each

| Feature | `include` | `import` |
|---------|-----------|----------|
| Scope | Global | Namespaced |
| Conflicts | Possible | Avoided |
| Re-includes | Executes again | Cached |
| Use case | Small shared files | Libraries, larger codebases |

---

## Common Module Patterns

### Loading config

```
let config = json.load("config.json")
print config.host
print config.port
```

### File processing pipeline

```
let raw = fs.read("data.csv")
let lines = raw.split("\n")
let headers = lines[0].split(",")

for line in lines[1:] {
    let values = line.split(",")
    let record = {}
    for i, header in enumerate(headers) {
        record[header] = values[i]
    }
    // process record
}
```

### HTTP + JSON

```
let resp = http.get("https://api.github.com/repos/rust-lang/rust")
let data = resp.json()
print data.name        // rust
print data.stars       // star count
```

### Time-based logic

```
let now = datetime.now()
let hour = datetime.hour()

if hour >= 9 and hour < 17 {
    print "Business hours"
} else {
    print "After hours"
}
```

---

## Pro Tips

1. **Built-in modules need no import.** Use `fs.read()`, `http.get()` directly.
2. **Use `import` over `include`.** Better for avoiding name conflicts.
3. **Use `from X import Y` for specific items.** Keeps the namespace clean.
4. **Check `fs.exists()` before `fs.read()`.** Prevents file-not-found errors.
5. **Use `try/catch` with modules.** Module functions can fail (network, file I/O).

---

## Common Mistakes

### Importing a non-existent module

```
// WRONG — module doesn't exist
import nonexistent
// Error: module not found

// Check available modules
// Run .help modules in the shell
```

### Forgetting that modules are dicts

```
// All module access is dict access:
fs.read("file.txt")    // fs is a dict, "read" is a key
http.get("url")         // http is a dict, "get" is a key
```

### Including the same file twice

```
// include executes the file again — can cause redefinitions
include "utils.z"
include "utils.z"    // executes again — may cause issues
```

---

## See Also

- [fs Module](fs.md) — Filesystem operations
- [http Module](http.md) — HTTP requests
- [json Module](json.md) — JSON handling
- [crypto Module](crypto.md) — Cryptography
- [re Module](re.md) — Regular expressions
- [datetime Module](datetime.md) — Date/time operations
