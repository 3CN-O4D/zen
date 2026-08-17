# Zen Language Reference

Zen is a dynamic, interpreted scripting language with Python-inspired ergonomics
and JavaScript-style object and closure features. This document is the complete
reference for the native Rust runtime. Everything described here runs without a
Python interpreter.

## Programs, comments, whitespace

Zen is newline-delimited. Statements end at a newline; a trailing `;` is
optional and accepted. Blocks are delimited by `{` `}` braces.

```zen
// line comment
# also a comment
/* block comment */

let x = 1   // statement ends at newline
let y = 2;

{
    let z = 3
    print x + y + z   // 6
}
```

## Variables

Declare with `let` (mutable) or `const` (immutable).

```zen
let name = "Ada"
name = "Grace"          // ok: let is mutable

const PI = 3.14159
PI = 3.0                // error: cannot assign to constant: PI
```

> Built-in names (native functions and modules) are locked: `str = "x"` is an
> error. Declaring `let str = "x"` shadows the built-in and unlocks the name.

## Values and types

| Type     | Examples                            |
|----------|-------------------------------------|
| number   | `42`, `3.14`, `-7`, `1e6`, `0x1F`   |
| string   | `"hello"`, `'world'`                |
| bool     | `true`, `false`                     |
| null     | `null`                              |
| list     | `[1, 2, 3]`                         |
| dict     | `{ "name": "Ada", "age": 36 }`      |
| function | named functions, lambdas, natives   |
| object   | class instances                     |
| socket   | network socket                      |

```zen
type(42)          // "number"
type("hi")        // "string"
type([1])         // "list"
type({a: 1})      // "dict"
type(null)        // "null"
type(new Person()) // "object"
typeof x          // same, operator form
```

## Operators

Arithmetic:

```zen
1 + 2        // 3
5 - 3        // 2
4 * 3        // 12
10 / 4       // 2.5
10 % 3       // 1   modulo
2 ** 10      // 1024  power
```

Comparison and equality:

```zen
1 == 1        // true   loose equality
1 === "1"     // false  strict equality (type + value)
1 != 2        // true
1 !== "1"     // true   strict inequality
2 < 3         // true
3 <= 3        // true
4 > 2         // true
2 >= 2        // true
```

Logical (with short-circuiting):

```zen
true and false   // false
true or false    // true
not true         // false
null ?? "default"  // "default"  (nullish: right when left is null)
5 ?? "default"     // 5
```

Bitwise:

```zen
5 & 3        // 1
5 | 3        // 7
5 ^ 3        // 6
~5           // -6
1 << 4       // 16
16 >> 2      // 4
```

Assignment and compound assignment:

```zen
let n = 10
n += 5       // 15
n -= 2       // 13
n *= 2       // 26
n /= 2       // 13
n %= 3       // 1
n++          // post-increment
++n          // pre-increment
n--          // post-decrement
```

Index and member assignment:

```zen
let list = [1, 2, 3]
list[0] = 99          // [99, 2, 3]

let d = { a: 1 }
d.a = 2               // {a: 2}
d["b"] = 3            // {a: 2, b: 3}
```

Membership and `is`:

```zen
"ell" in "hello"      // true
2 in [1, 2, 3]        // true
"a" in {a: 1}         // true
[1, 2] is "list"      // type check
```

## Strings

```zen
let s = "hello world"
s.len()                   // 11  (also s.length())
s.upper()                 // "HELLO WORLD"
s.lower()                 // "hello world"
s.trim()                  // "hello world"
s.strip()                 // same as trim()
s.trimStart()             // "hello world "
s.trimEnd()               // " hello world"
s.split(" ")              // ["hello", "world"]
s.replace("world", "zen") // "hello zen"
s.contains("ell")         // true
s.startsWith("he")        // true
s.endsWith("ld")          // true
s.indexOf("o")            // 4  (or -1)
s.slice(0, 5)             // "hello"   (also substring/substr)
s.repeat(2)               // "hello worldhello world"
s.toList()                // ["h","e",...]
s.toUpperCase()           // same as upper()
```

## Lists

```zen
let xs = [1, 2, 3]
xs.push(4)                // [1, 2, 3, 4]
xs.pop()                  // 4
xs.first()                // 1
xs.last()                 // 3
xs.contains(2)            // true
xs.join("-")              // "1-2-3"
xs.reverse()              // [3, 2, 1]
xs.sort()                 // sorted copy
xs.skip(1)                // [2, 3]
xs.concat([4, 5])         // [1, 2, 3, 4, 5]
xs.sum()                  // 6
xs.unique()               // deduplicated copy
xs.shift()                // 1 (removes first)
xs.unshift(0)             // [0, 1, 2, 3]
xs.length()               // 3

// higher-order methods with lambdas
[1, 2, 3].map(fn (x) => x * 2)                // [2, 4, 6]
[1, 2, 3, 4].filter(fn (x) => x % 2 == 0)     // [2, 4]
[1, 2, 3].each(fn (x) => print x)             // prints 1 2 3
```

Negative indexing and ranges:

```zen
let xs = [10, 20, 30]
xs[-1]        // 30
xs[0..2]      // [10, 20]
2..5          // [2, 3, 4]
5..1          // [5, 4, 3, 2]
```

## Dictionaries

```zen
let d = { name: "Ada", age: 36 }
d.name                      // "Ada"
d["age"]                    // 36
d.age = 37                  // set
d.keys()                    // ["name", "age"]
d.values()                  // ["Ada", 37]
d.get("name")               // "Ada"
d.get("missing", "default") // "default"
d.has("age")                // true
d.set("city", "London")     // returns new dict with the entry
d.length()                  // 2
```

Keys are strings; bare identifiers are shorthand for string keys.

## Control flow

```zen
if x > 10 {
    print "big"
} else if x > 0 {
    print "positive"
} else {
    print "small"
}
```

```zen
let i = 0
while i < 5 {
    print i
    i += 1
}
```

`for ... in` iterates lists, strings, ranges, and dict keys:

```zen
for item in [1, 2, 3] { print item }
for ch in "abc" { print ch }
for i in 0..10 { print i }
for key in { a: 1, b: 2 } { print key }
```

`break` exits a loop, `continue` skips to the next iteration.

```zen
switch day {
    case "mon":
        print "start of week"
        break
    case "fri":
        print "almost there"
        break
    default:
        print "meh"
}
```

```zen
try {
    risky_call()
} catch (e) {
    print "caught: " + e
} finally {
    print "always runs"
}
```

`catch` and `except` are synonyms. A catch may name a type, bind the error
with `as` or `(var)`, both, or neither; typed catches also match subclasses:

```zen
try {
    risky_call()
} except ZeroDivisionError as e {
    print "division: " + e
} catch ArithmeticError as e {
    print "arithmetic: " + e
} catch as e {
    print "anything else: " + e
}
```

The built-in error types live in the `errors` module (`errors.Error`,
`errors.ValueError`, `errors.TypeError`, `errors.MathError`,
`errors.KeyboardInterrupt`, ...). Every error type is a class, so you can
construct it with `new` and build your own by subclassing:

```zen
class MoneyError extends errors.Error { }
throw new MoneyError("overdraft")
throw new errors.ValueError("bad input")
```

#### errors.define()

Define custom error classes without writing a class:

```zen
errors.define("MoneyError", "Error", "not enough money")
errors.define("InsufficientFundsError", "MoneyError", "insufficient funds")

throw new InsufficientFundsError("balance too low")
```

Arguments: `(name, parent?, message)`. Parent defaults to `"Error"`.

`throw` raises an error:

```zen
function check(n) {
    if n < 0 {
        throw new errors.ValueError("must be non-negative")
    }
    return n
}
```

Uncaught errors print a Python-style traceback with the file, line, column,
and the error's type and message.

## Functions

Named functions (`function` and `func` are synonyms):

```zen
function add(a, b) {
    return a + b
}
print add(2, 3)   // 5
```

Functions are first-class values:

```zen
let f = add
print f(1, 2)          // 3

function twice(f, x) {
    return f(f(x))
}
print twice(lambda n: n + 1, 5)   // 7
```

Lambdas use `lambda` (or `fn`), with either a colon body or a block body:

```zen
let square = lambda x: x * x
print square(4)          // 16

let add = lambda (a, b): a + b
print add(1, 2)          // 3

let clamp = lambda (v, lo, hi) {
    if v < lo { return lo }
    if v > hi { return hi }
    return v
}
```

Recursion:

```zen
function fib(n) {
    if n < 2 { return n }
    return fib(n - 1) + fib(n - 2)
}
print fib(10)   // 55
```

Spread in list and dict literals (not in call arguments):

```zen
let xs = [1, 2]
let merged = [0, ...xs, 3]     // [0, 1, 2, 3]
let o = { a: 1 }
let cfg = { ...o, b: 2 }       // {a: 1, b: 2}
```

## Classes and objects

Classes have a constructor (`init`), mutable fields (`self.x`), and methods.
Single inheritance is supported with `extends`.

```zen
class Animal {
    function init(name) {
        self.name = name
    }
    function speak() {
        return self.name + " makes a sound"
    }
}

class Dog extends Animal {
    function speak() {
        return self.name + " barks"
    }
}

let a = new Animal("cat")
print a.speak()       // cat makes a sound
print a.name          // cat
a.name = "big cat"

let d = new Dog("rex")
print d.speak()       // rex barks
```

Methods declared inside the class body are the only declarations allowed there.

## Modules

### Built-in module dicts

Modules are available as global dicts without any import: `json`, `fs`, `re`,
`random`, `math`, `time`, `os`, `base64`, `base32`, `crypto`, `cryptography`,
`datetime`, `uuid`, `color`, `csv`, `http`, `decimal`, `threading`,
`statistics`, and `browser`. Each is documented in its own page under
[Modules](../modules/README.md).

```zen
let data = json.parse('{"a": 1}')
print math.sqrt(16)          // 4
print fs.read("notes.txt")
print http.get("https://api.example.com/data").json()
```

### import / from / include

`import` and `from` load `.z` files by path or package name. `include` and
`load` are synonyms for `import`. `as` creates an alias.

```zen
import "greetings.z"          // run everything in greetings.z
from "greetings" import greet // bring in just `greet`
import "greetings" as g       // now call g.greet()
import greetings              // bare name: finds greetings.z
```

#### Dotted (package) imports

Dotted names like `import pkg.sub.mod` are supported. The loader resolves each
segment, looking for the file in `~/.zen/modules/` (installed packages) and
then in the current directory:

```zen
import mylib                        // loads mylib.z or mylib/main.z
import mylib.utils                  // loads mylib/utils.z
from mylib.utils import add         // bring in just `add`
from mylib.utils import add as a    // aliased import
```

#### Absolute path imports

Absolute paths are supported for loading files outside the current directory:

```zen
import /usr/local/lib/helpers.z
```

#### Resolver order

The loader checks in order:

1. `~/.zen/modules/<path>.z` — installed packages
2. `./<path>.z` — current directory
3. `./<path>/main.z` — package main entry

#### zen package manager (PM)

```bash
zen pm init mymodule               # create zen.json + main.z
zen pm install user/repo           # from GitHub
zen pm install https://...         # from URL
zen pm install ./local-module      # from local directory
zen pm install helpers.z           # from single file
zen pm list                        # list installed modules
zen pm remove helpers              # remove a module
```

### native declarations

`native function <name>(...)` binds a name to a built-in native routine. It is
used by the bundled std modules to expose native functions as Zen values.

```zen
native function fs_read(path)
let read_file = fs_read
```

## Destructuring

Lists and dicts can be unpacked:

```zen
const [C, D] = [1, 2]
const { E } = { E: 5 }
let [a, b] = [1, 2]
let { x, y } = { x: 1, y: 2 }
```

## Built-in globals

| Function | Description |
|----------|-------------|
| `print` | print a value, newline appended |
| `input` | read a line from stdin |
| `len(x)` | length of string/list/dict |
| `str(x)` | convert to string |
| `int(x)` | truncate to integer |
| `float(x)` | convert to float |
| `bool(x)` | convert to bool |
| `list(x)` | convert to list |
| `abs(x)`, `min(...)`, `max(...)` | numeric helpers |
| `round(x)` | round to nearest |
| `trunc(x)` | truncate |
| `hex(x)` | hex string |
| `range(end)` / `range(start, end)` | inclusive range list |
| `type(x)` / `typeof` | type name |
| `exit(code)` | terminate |
| `sleep(sec)`, `wait(ms)` | pause |

## std browser prelude

The bundled `std/browser.z` is loaded automatically as a prelude, so legacy
scripts that call `go`, `click`, `fill`, `text`, `attr`, `wait_for`, `shot`,
`title`, `url`, `page`, and `browser` work unchanged. See
[Browser automation](../modules/browser.md).

## Standard Modules

All modules below are available as globals (no import needed).

### string

| Function | Description |
|----------|-------------|
| `string.upper(s)` | uppercase |
| `string.lower(s)` | lowercase |
| `string.title(s)` | title case |
| `string.capitalize(s)` | capitalize first char |
| `string.swapcase(s)` | swap case |
| `string.strip(s)` | trim whitespace |
| `string.lstrip(s)` / `string.rstrip(s)` | left/right trim |
| `string.split(s, sep)` | split by separator |
| `string.splitlines(s)` | split by newlines |
| `string.join(sep, list)` | join list with separator |
| `string.replace(s, old, new)` | replace substring |
| `string.count(s, sub)` | count occurrences |
| `string.find(s, sub)` | index of first occurrence (-1 if not found) |
| `string.rfind(s, sub)` | index of last occurrence |
| `string.startswith(s, prefix)` | starts with prefix |
| `string.endswith(s, suffix)` | ends with suffix |
| `string.contains(s, sub)` | contains substring |
| `string.ljust(s, width, fill)` | left justify |
| `string.rjust(s, width, fill)` | right justify |
| `string.center(s, width, fill)` | center |
| `string.zfill(s, width)` | zero-pad |
| `string.repeat(s, n)` | repeat string |
| `string.isdigit(s)` | all chars are digits |
| `string.isalpha(s)` | all chars are alphabetic |
| `string.isalnum(s)` | all chars are alphanumeric |
| `string.isspace(s)` | all chars are whitespace |
| `string.islower(s)` | all chars are lowercase |
| `string.isupper(s)` | all chars are uppercase |
| `string.digits` | "0123456789" |
| `string.ascii_letters` | "abc...XYZ" |
| `string.ascii_lowercase` | "abc...xyz" |
| `string.ascii_uppercase` | "ABC...XYZ" |
| `string.punctuation` | !"#$%&'()*+,-./... |
| `string.whitespace` | space, tab, newline... |
| `string.printable` | all printable characters |

### hashlib

| Function | Description |
|----------|-------------|
| `hashlib.sha256(data)` | SHA-256 hex digest |
| `hashlib.md5(data)` | MD5 hex digest |
| `hashlib.sha1(data)` | SHA-1 hex digest |
| `hashlib.create(algo, data)` | returns `{hexdigest, name}` |
| `hashlib.algorithms_available` | list of supported algorithms |

### struct

| Function | Description |
|----------|-------------|
| `struct.pack(fmt, values...)` | pack values to binary string |
| `struct.unpack(fmt, data)` | unpack binary string to list |
| `struct.calcsize(fmt)` | size in bytes for format |

Format chars: `b/B` (i8/u8), `h/H` (i16/u16), `i/I` (i32/u32), `q/Q` (i64/u64), `f` (f32), `d` (f64), `s` (string), `x` (pad byte), `?` (bool). Prefix: `>` (big-endian), `<` (little-endian).

### subprocess

| Function | Description |
|----------|-------------|
| `subprocess.run(cmd, cwd?)` | returns `{ok, code, stdout, stderr}` |
| `subprocess.call(cmd)` | returns exit code |
| `subprocess.check_output(cmd)` | returns stdout or throws on error |

### collections

| Function | Description |
|----------|-------------|
| `collections.Counter(list)` | counts of each element |
| `collections.chain(a, b, ...)` | concatenate lists |
| `collections.flatten(nested)` | recursive flatten |

### itertools

| Function | Description |
|----------|-------------|
| `itertools.range(start, end?, step?)` | numeric range |
| `itertools.enumerate(list)` | `[[0, a], [1, b], ...]` |
| `itertools.zip(a, b)` | paired elements |
| `itertools.chain(a, b, ...)` | concatenate |
| `itertools.product(a, b)` | cartesian product |
| `itertools.combinations(list, r)` | r-element combinations |
| `itertools.permutations(list, r?)` | r-element permutations |
| `itertools.accumulate(list)` | running sum |
| `itertools.take(n, list)` | first n elements |
| `itertools.drop(n, list)` | skip first n elements |
| `itertools.repeat(val, n)` | repeat value n times |

### pathlib

| Function | Description |
|----------|-------------|
| `pathlib.join(parts...)` | join path components |
| `pathlib.name(path)` | filename |
| `pathlib.parent(path)` | parent directory |
| `pathlib.stem(path)` | filename without extension |
| `pathlib.suffix(path)` | extension (with dot) |
| `pathlib.suffixes(path)` | list of extensions |
| `pathlib.is_absolute(path)` | is absolute |
| `pathlib.resolve(path)` | canonicalize |
| `pathlib.absolute(path)` | make absolute |
| `pathlib.exists(path)` | file/dir exists |
| `pathlib.is_file(path)` | is a regular file |
| `pathlib.is_dir(path)` | is a directory |
| `pathlib.glob(pattern)` | glob match |
| `pathlib.touch(path)` | create/touch file |
| `pathlib.mkdir(path, parents?)` | create directory |
| `pathlib.rmdir(path)` | remove directory |
| `pathlib.unlink(path)` | delete file |
| `pathlib.rename(src, dst)` | rename |
| `pathlib.read_text(path)` | read file to string |
| `pathlib.write_text(path, data)` | write string to file |

### shutil

| Function | Description |
|----------|-------------|
| `shutil.copy(src, dst)` | copy file |
| `shutil.copy2(src, dst)` | copy with metadata |
| `shutil.move(src, dst)` | move/rename |
| `shutil.rmtree(path)` | recursive delete |
| `shutil.copytree(src, dst)` | recursive copy |
| `shutil.which(name)` | find executable in PATH |
| `shutil.disk_usage(path)` | returns `{total, used, free}` |

### urllib

| Function | Description |
|----------|-------------|
| `urllib.urlopen(url)` | HTTP GET (returns response dict) |
| `urllib.parse(url)` | parse URL into `{scheme, host, port, path, query}` |
| `urllib.parse_qs(query)` | parse query string to dict |
| `urllib.quote(s)` | percent-encode |
| `urllib.unquote(s)` | percent-decode |
| `urllib.urlencode(dict)` | encode dict to query string |

### tempfile

| Function | Description |
|----------|-------------|
| `tempdir()` | system temp directory |
| `tempfile.mkdtemp(prefix?)` | create temp dir |
| `tempfile.mkstemp(prefix?)` | create temp file |

### binascii

| Function | Description |
|----------|-------------|
| `binascii.hexlify(data)` | bytes to hex string |
| `binascii.unhexlify(hex)` | hex string to bytes |
| `binascii.b2a_base64(data)` | bytes to base64 |
| `binascii.a2b_base64(data)` | base64 to bytes |

### glob

| Function | Description |
|----------|-------------|
| `glob.glob(pattern)` | match files by pattern |

## Protocol Modules

### ftp

| Function | Description |
|----------|-------------|
| `ftp.connect(host, port?)` | connect to FTP server |
| `ftp.login(session, user, pass)` | authenticate |
| `ftp.pwd(session)` | current directory |
| `ftp.list(session, path?)` | LIST output |
| `ftp.nlist(session, path?)` | names only |
| `ftp.cwd(session, dir)` | change directory |
| `ftp.retr(session, file)` | download file |
| `ftp.stor(session, file, data)` | upload file |
| `ftp.dele(session, file)` | delete file |
| `ftp.mkdir(session, dir)` | create directory |
| `ftp.rmdir(session, dir)` | remove directory |
| `ftp.rename(session, from, to)` | rename |
| `ftp.quit(session)` | disconnect |

### smtp

| Function | Description |
|----------|-------------|
| `smtp.connect(host, port?)` | connect |
| `smtp.login(session, user, pass)` | authenticate |
| `smtp.sendmail(session, from, to, msg)` | send email |
| `smtp.message(from, to, subject, body)` | build MIME message |
| `smtp.quit(session)` | disconnect |

### pop3

| Function | Description |
|----------|-------------|
| `pop3.connect(host, user, pass, port?)` | connect + login |
| `pop3.stat(session)` | `{count, size}` |
| `pop3.list(session)` | message list |
| `pop3.retr(session, id)` | retrieve message |
| `pop3.dele(session, id)` | delete message |
| `pop3.quit(session)` | disconnect |

### imap

| Function | Description |
|----------|-------------|
| `imap.connect(host, user, pass, port?)` | connect + login |
| `imap.select(session, mailbox)` | select mailbox |
| `imap.search(session, criteria)` | search (e.g. "ALL") |
| `imap.fetch(session, id)` | `{flags, body}` |
| `imap.list(session)` | list mailboxes |
| `imap.logout(session)` | disconnect |

### telnet

| Function | Description |
|----------|-------------|
| `telnet.connect(host, port?)` | connect |
| `telnet.write(session, data)` | send data |
| `telnet.read(session, size?)` | read bytes |
| `telnet.read_until(session, marker)` | read until marker |
| `telnet.close(session)` | disconnect |

### dns

| Function | Description |
|----------|-------------|
| `dns.resolve(name)` | resolve to IP list |
| `dns.query(name, type?)` | query records (A, AAAA, MX, TXT, NS, CNAME) |

### ssh

| Function | Description |
|----------|-------------|
| `ssh.available()` | true if ssh binary exists |
| `ssh.run(opts, command)` | run remote command (opts: `{host, user?, port?, key?}`) |
| `ssh.upload(opts, local, remote)` | upload via scp |
| `ssh.download(opts, remote, local)` | download via scp |

### scapy

| Function | Description |
|----------|-------------|
| `scapy.ip(src, dst, proto?, ttl?, payload?)` | build IP layer |
| `scapy.tcp(sport, dport, payload?)` | build TCP layer |
| `scapy.udp(sport, dport, payload?)` | build UDP layer |
| `scapy.icmp(type?, code?, payload?)` | build ICMP layer |
| `scapy.raw(data)` | build raw data layer |
| `scapy.build(layer)` | serialize to bytes |
| `scapy.parse(bytes)` | parse bytes to layers |
| `scapy.send(layer)` | send raw packet (requires root) |
| `scapy.sniff(count?, timeout?)` | sniff packets (requires root) |
| `scapy.checksum(data)` | internet checksum |
| `scapy.ip_to_int(ip)` | convert IP string to integer |
| `scapy.int_to_ip(int)` | convert integer to IP string |