# Data Utility Modules

Zen has a powerful collection of built-in data processing modules. Since they are compiled natively, they execute quickly and do not require external runtime packages.

---

## JSON Module (`json`)

The `json` module provides standard JSON parsing and encoding.

### Methods
* `json.parse(str)` (or `json.decode`): Decodes a JSON string into a Zen List or Dictionary.
* `json.encode(val)` (or `json.stringify`): Serializes a Zen value (string, number, list, dict, null) into a JSON string.
* `json.load(path)`: Reads a JSON file from disk and parses it.
* `json.save(path, val)`: Encodes a value and writes it to a JSON file.

### Examples
```zen
let text = '{"name": "Grace", "languages": ["Cobol", "Fortran"], "active": true}'
let obj = json.parse(text)

print obj.name               // "Grace"
print obj.languages[0]       // "Cobol"

let back_to_text = json.encode(obj)
print back_to_text

// stringify is an alias for encode
let s = json.stringify({x: 42, y: "hello"})
print s                      // {"x":42,"y":"hello"}

// load and save files
let data = json.load("config.json")
json.save("output.json", data)
```

---

## CSV Module (`csv`)

Reads and writes RFC-4180 compliant CSV files with full support for double-quoted fields, commas, and embedded newlines.

### Methods
* `csv.parse(csv_string)`: Parses a CSV string and returns a List of Lists of strings.
* `csv.read(file_path)`: Reads a CSV file from disk and parses it.
* `csv.encode(list_of_lists, headers?)`: Encodes a list of lists into a CSV string (optionally prepend a list of headers).
* `csv.write(file_path, list_of_lists, headers?)`: Encodes and writes a CSV file to disk.

### Examples
```zen
let csv_data = [
    ["Name", "ID", "City"],
    ["Ada Lovelace", "1", "London"],
    ["Linus Torvalds", "2", "Portland"]
]
csv.write("people.csv", csv_data)

let parsed = csv.read("people.csv")
print parsed[1][0]  // "Ada Lovelace"
```

---

## Regular Expressions Module (`re`)

Natively powered by the high-performance Rust `regex` crate.

### Methods
* `re.match(pattern, text)` (or `re.matches`): Returns `true` if the text matches the regular expression pattern.
* `re.search(pattern, text)`: Returns `true` if any part of the text matches.
* `re.find(pattern, text)` (or `re.findall`): Returns a List of all matched substrings.
* `re.split(pattern, text)`: Splits text by the matches of the pattern.
* `re.replace(pattern, text, replacement)` (or `re.sub`): Replaces matches in text with the replacement string.

### Examples
```zen
let text = "My phone number is 555-123-4567."
let pattern = "\\d{3}-\\d{3}-\\d{4}"

if re.search(pattern, text) {
    print "Found phone number!"
}

let numbers = re.find("\\d+", text)
print numbers  // ["555", "123", "4567"]

let redacted = re.replace(pattern, text, "[REDACTED]")
print redacted  // "My phone number is [REDACTED]."
```

---

## Random Module (`random`)

Generates pseudo-random numbers and elements.

### Methods
* `random.random()`: Floating-point number between `0.0` and `1.0`.
* `random.randint(min, max)`: Integer between `min` and `max` (inclusive).
* `random.randrange(start, stop)`: Random element from the specified range.
* `random.uniform(min, max)`: Float between `min` and `max`.
* `random.choice(list)`: Selects a random element from a list.
* `random.choices(list, k)`: Selects `k` random elements (with replacement).
* `random.sample(list, k)`: Selects `k` unique elements (without replacement).
* `random.shuffle(list)`: Shuffles a list in-place and returns it.
* `random.hex(n_bytes)`: Random hex string of the given byte length.
* `random.seed(n)`: Seeds the random number generator.

### Examples
```zen
print random.randint(1, 10)       // e.g. 7
let card = random.choice(["Hearts", "Diamonds", "Clubs", "Spades"])
print card

let uuid_like = random.hex(16)
print uuid_like
```

---

## Encoding Modules (`base64` and `base32`)

Provides fast string encoding and decoding.

### Base64 Methods
* `base64.encode(text)`: Base64 encodes text.
* `base64.decode(b64_string)`: Decodes a base64 string.
* `base64.url_encode(text)`: URL-safe base64 encoding.
* `base64.url_decode(b64_string)`: URL-safe base64 decoding.

### Base32 Methods
* `base32.encode(text)`: RFC 4648 Base32 encodes text (no padding).
* `base32.decode(b32_string)`: Decodes a Base32 string.

### Examples
```zen
let encoded = base64.encode("hello world")
print encoded  // "aGVsbG8gd29ybGQ="

let decoded = base64.decode(encoded)
print decoded  // "hello world"
```

---

## UUID Module (`uuid`)

Generates standard universally unique identifiers (UUIDs).

### Methods
* `uuid.uuid4()` (or `uuid.v4()`): Generates a random version 4 UUID string.
* `uuid.uuid1()` (or `uuid.v1()`): Generates a time-based version 1 UUID string.
* `uuid.uuid3(namespace, name)` (or `uuid.v3()`): MD5 hash-based version 3 UUID string.
* `uuid.uuid5(namespace, name)` (or `uuid.v5()`): SHA-1 hash-based version 5 UUID string.

### Constants
* `uuid.NAMESPACE_DNS`
* `uuid.NAMESPACE_URL`
* `uuid.NAMESPACE_OID`
* `uuid.NAMESPACE_X500`

### Examples
```zen
print uuid.uuid4()  // e.g. "a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d"
print uuid.v4()     // same, short alias
let ns_url = uuid.NAMESPACE_URL
print uuid.uuid5(ns_url, "https://github.com/3CN-O4D/zen")
```

---

## Decimal Module (`decimal`)

Symmetric to Python's decimal library, providing fixed-point and floating-point decimal arithmetic with customizable precision.

### Methods & Values
* `decimal.Decimal(value)`: Converts a number or string into a Decimal dictionary wrapper.
* `decimal.getcontext()`: Returns a dictionary of current context options: `prec` (precision) and `rounding`.
* `decimal.localcontext()`: Temporarily enters a local execution context block.

### Constants
* `decimal.ROUND_HALF_UP`
* `decimal.ROUND_HALF_EVEN`
* `decimal.ROUND_DOWN`
* `decimal.ROUND_UP`
* `decimal.ROUND_CEILING`
* `decimal.ROUND_FLOOR`
* `decimal.ROUND_HALF_DOWN`

---

## Terminal Color Module (`color`)

Provides cross-platform ANSI colors and text styles to decorate CLI output.

### Formatting Methods
Methods wrap the input string with ANSI codes and append a reset.
* Text styles: `color.bold(s)`, `color.dim(s)`, `color.italic(s)`, `color.underline(s)`, `color.blink(s)`, `color.reverse(s)`, `color.hidden(s)`, `color.strike(s)`
* Basic colors: `color.black(s)`, `color.red(s)`, `color.green(s)`, `color.yellow(s)`, `color.blue(s)`, `color.magenta(s)`, `color.cyan(s)`, `color.white(s)`
* Backgrounds: `color.bg_black(s)`, `color.bg_red(s)`, etc.
* Bright variants: `color.bright_black(s)`, `color.bright_red(s)`, etc.
* Custom colors:
  * `color.rgb(r, g, b, s)`: Foreground RGB.
  * `color.bg_rgb(r, g, b, s)`: Background RGB.
  * `color.hex("#RRGGBB", s)`: Hex foreground.
* Clean utility:
  * `color.strip(s)`: Removes all ANSI escape codes from a string.

### Examples
```zen
print color.red(color.bold("CRITICAL ERROR"))
print color.hex("#4EC9B0", "Polished mint green output")
```

---

## Statistics Module (`statistics`)

Provides basic statistical analysis routines over lists of numbers.

### Methods
* `statistics.sum(list)`: Returns the sum.
* `statistics.mean(list)`: Returns the average.
* `statistics.median(list)`: Returns the middle value.
* `statistics.mode(list)`: Returns the most frequent value.
* `statistics.stdev(list)`: Returns the sample standard deviation.
* `statistics.variance(list)`: Returns the sample variance.

### Examples
```zen
let scores = [85, 90, 78, 92, 85, 100]
print statistics.mean(scores)    // 88.333
print statistics.stdev(scores)   // 7.865
```

---

## Threading Module (`threading`)

Spawns OS threads to run background workers parallel to the main interpreter thread.

### Methods
* `threading.start(worker_function)`: Spawns a parallel background thread that executes the user function. Returns a thread handle dictionary containing `name` and `daemon: true`.

### Examples
```zen
function background_worker() {
    let i = 0
    while i < 3 {
        print "working..."
        wait 500  // sleep 500ms
        i += 1
    }
}

threading.start(background_worker)
wait 2000  // wait 2 seconds to let the background thread finish
print "main thread done"
```
