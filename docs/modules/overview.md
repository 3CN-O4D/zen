# Modules Overview

Zen ships with a large set of **native modules** — dicts that are registered
as **global variables on startup**, so you can use them without any `import`:

```zen
print(math.sqrt(144))          # 12
fs.write("hello.txt", "hi")
json.parse('{"a": 1}')
```

`import` is only needed for the Zen standard library (`logging`, `requests`,
`sys`, `argparser`) and your own `.z` modules. See
[imports.md](../imports.md).

## Every module is a dict

A module is just a `dict` whose keys are its functions (and sometimes
constants/factories):

```zen
print(typeof math)        # dict
print(math.keys())        # [sqrt, sin, cos, pi, e, ...]
print(math.sqrt(9))       # 3
```

`help(math)` also prints the module's member list.

## All available modules

All of the following are global dicts — no import required.

### Core

| Module | Description | Example |
|--------|-------------|---------|
| `math` | Math constants & functions | `math.sqrt(144)` |
| `fs` | Filesystem operations | `fs.read("file.txt")` |
| `http` | HTTP client | `http.get("https://...")` |
| `re` | Regular expressions | `re.findall("\\d+", "abc123")` |
| `json` | JSON encode/decode | `json.parse('{"a": 1}')` |
| `random` | Random numbers | `random.randint(1, 100)` |
| `time` | Time functions | `time.now()` |
| `datetime` | Date/time objects | `datetime.now()` |
| `os` | OS info & processes | `os.platform()` |
| `string` | String helpers & constants | `string.upper("hi")` |
| `errors` | Error classes & typed catch | `errors.define("MyErr")` |
| `decimal` | Arbitrary-precision decimals | `decimal.Decimal("3.14")` |
| `statistics` | Statistical functions | `statistics.mean([1,2,3])` |
| `threading` | Background execution | `threading.start(fn)` |

### Encoding & crypto

| Module | Description | Example |
|--------|-------------|---------|
| `base64` | Base64 encode/decode | `base64.encode("hello")` |
| `base32` | Base32 encode/decode | `base32.encode("hello")` |
| `binascii` | Binary/ASCII conversion | `binascii.hexlify("abc")` |
| `crypto` | Cryptographic hashes (sha256, md5, ...) | `crypto.sha256("data")` |
| `cryptography` | Fernet symmetric encryption | `cryptography.fernet.encrypt(...)` |
| `hashlib` | Hashing (sha256, md5, sha1, ...) | `hashlib.sha256("data")` |
| `uuid` | UUID generation | `uuid.uuid4()` |

### Files & paths

| Module | Description | Example |
|--------|-------------|---------|
| `pathlib` | Path manipulation | `pathlib.join("a", "b")` |
| `shutil` | High-level file ops | `shutil.copy(src, dst)` |
| `glob` | Pattern matching | `glob.glob("*.txt")` |
| `tempfile` | Temporary files/dirs | `tempfile.mkdtemp()` |
| `subprocess` | External commands | `subprocess.run(["ls"])` |
| `struct` | Binary pack/unpack | `struct.pack("i", 42)` |

### Data structures & streams

| Module | Description | Example |
|--------|-------------|---------|
| `csv` | CSV parse/write | `csv.parse("a,b")` |
| `collections` | Counter, chain, flatten | `collections.Counter([1,1,2])` |
| `itertools` | Iterators | `itertools.chain(a, b)` |
| `urllib` | URL handling | `urllib.parse("https://...")` |
| `color` | ANSI color helpers | `color.red("error")` |

### Networking

| Module | Description | Example |
|--------|-------------|---------|
| `socket` | TCP/UDP sockets | `socket.open("host", 80)` |
| `ftp` | FTP client | `ftp.connect("host")` |
| `smtp` | SMTP email client | `smtp.connect("host")` |
| `pop3` | POP3 email client | `pop3.connect("host")` |
| `imap` | IMAP email client | `imap.connect("host")` |
| `telnet` | Telnet client | `telnet.connect("host")` |
| `dns` | DNS resolver | `dns.resolve("example.com")` |
| `ssh` | SSH/SCP wrapper | `ssh.run("host", "ls")` |

### System / security / automation

| Module | Description | Example |
|--------|-------------|---------|
| `browser` | Browser automation via CDP | `browser.open(...)` |
| `wa` | WhatsApp automation | `wa.sendText("...")` |
| `bluetooth` | Bluetooth via bluetoothctl | `bluetooth.scan()` |
| `wifi` | WiFi via nmcli | `wifi.status()` |
| `crunch` | Password wordlist generator | `crunch.generate(...)` |
| `scapy` | Packet crafting/sniffing | `scapy.build(...)` |

These mesh the native runtime into useful categories. The definitive list is
available in the REPL with `:help modules`.

## Import patterns (for std/ and your modules)

```zen
import utils                    # bind module + expose names
import utils as u               # alias
from utils import add, sub      # specific names
from utils import add as a      # aliased items
from utils import *             # all public names

import logging, sys             # one statement, multiple modules
```

### Local file modules

```zen
# lib/math.z
func square(x) { return x * x }

# main.z  (same directory)
import math
print(math.square(5))           # 25
```

`include "path.z"` executes a file and injects its names directly:

```zen
include "utils.z"
print greet("World")            # names in global scope
```

## Common patterns

### Load a config

```zen
var config = json.parse(fs.read("config.json"))
print(config.host)
print(config.port)
```

### HTTP + JSON

```zen
var resp = http.get("https://api.github.com/repos/rust-lang/rust")
var data = resp.json()
print(data.name)
```

### CSV pipeline

```zen
var rows = csv.parse(fs.read("data.csv"))
for row in rows { print(row) }
```

### Time-based logic

```zen
var now = datetime.now()
print(datetime.weekday(now))
```

## Common mistakes

- **Importing a non-existent module** → `module not found`. Only registered
  natives, `std/*.z`, and local files are resolvable.
- **Forgetting modules are dicts** — `fs.read` is `fs` (a dict) indexed by
  `"read"`. Missing methods surface as `dictionary has no method: X`.
- **Including the same file twice** — `include` re-executes the file (no
  caching), which can cause redefinition errors in large projects.

## See also

- [imports.md](../imports.md) — import, from, include, load, native func
- Per-module references: [fs](fs.md), [http](http.md), [json](json.md),
  [re](re.md), [math](math.md), [datetime](datetime.md), ...