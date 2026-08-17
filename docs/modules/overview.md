# Modules Overview

Zen ships with a comprehensive set of modules for file system, HTTP, cryptography, and more.

## Module System

Modules are accessed via dot notation:

```
fs.read("file.txt")
http.get("https://example.com")
re.search("\\d+", "abc123")
```

Some modules also have flat aliases:

```
json_parse('{"a": 1}')    // flat function
json.parse('{"a": 1}')    // module method
```

## Available Modules

| Module | Description |
|--------|-------------|
| [fs](fs.md) | File system operations |
| [http](http.md) | HTTP requests |
| [re](re.md) | Regular expressions |
| [json](json.md) | JSON parsing and serialization |
| [crypto](crypto.md) | Hashing and encryption |
| [datetime](datetime.md) | Date and time operations |
| [threading](threading.md) | Concurrent execution |
| [base64](#base64) | Base64 encoding |
| [base32](#base32) | Base32 encoding |
| [uuid](#uuid) | UUID generation |
| [statistics](#statistics) | Statistical functions |
| [decimal](#decimal) | Decimal arithmetic |
| [emoji](#emoji) | Emoji lookup |
| [csv](#csv) | CSV processing |
| [net](#net) | Network info |
| [storage](#storage) | localStorage |
| [cookies](#cookies) | Browser cookies |
| [cryptography](#cryptography) | Fernet encryption |
| [whatsapp](whatsapp.md) | WhatsApp client |

## Base64

```
base64.encode("hello")          // "aGVsbG8="
base64.decode("aGVsbG8=")       // "hello"
base64.url_encode("hello")      // URL-safe variant
base64.url_decode("aGVsbG8=")   // URL-safe variant
```

## Base32

```
base32.encode("hello")          // "NBSWY3DP"
base32.decode("NBSWY3DP")       // "hello"
```

## UUID

```
uuid.uuid4()                // random UUID: "550e8400-..."
uuid.uuid1()                // time-based UUID
uuid.uuid3("dns", "name")   // MD5 namespace UUID
uuid.uuid5("url", "name")   // SHA1 namespace UUID

uuid.NAMESPACE_DNS   // "dns"
uuid.NAMESPACE_URL   // "url"
```

## Statistics

```
statistics.mean([1,2,3,4,5])      // 3
statistics.median([1,2,3,4,5])    // 3
statistics.mode([1,1,2,3])        // 1
statistics.stdev([1,2,3,4,5])     // standard deviation
statistics.variance([1,2,3,4,5])  // variance
statistics.sum([1,2,3])           // 6
statistics.min(3,1,4,1,5)         // 1
statistics.max(3,1,4,1,5)         // 5
```

## Decimal

```
decimal.Decimal("3.14")                    // create a Decimal
decimal.getcontext()                        // {prec: 28, rounding: "ROUND_HALF_EVEN", ...}
decimal.setcontext({prec: 10})              // set precision
decimal.ROUND_HALF_UP                       // rounding mode constant
decimal.ROUND_HALF_EVEN
decimal.ROUND_DOWN
decimal.ROUND_UP
```

## Emoji

Access emojis by name, Unicode code point, or text emoticon:

```
emoji.smiley              // 😃
emoji.heart               // ❤️
emoji.hut                 // 🏡
emoji.fire                // 🔥
emoji.poop                // 💩
emoji.thumbsup            // 👍
emoji.hundred             // 💯
emoji.ok_hand             // 👌
emoji.flag_us             // 🇺🇸
emoji.flag_uk             // 🇬🇧
emoji.dog                 // 🐶
emoji.cat                 // 🐱
emoji.cake                // 🍰
emoji.rocket              // 🚀
emoji.pizza               // 🍕

emoji.by_name("smiley")            // 😃
emoji.by_code("1f600")             // 😀
emoji.codes(":D :) :(")            // 😄 🙂 🙁

emoji.names()                      // list all emoji names
emoji.search("heart")              // search by keyword
emoji.show()                       // print all emojis
emoji.show("heart")                // print "heart" emojis
```

## CSV

```
csv.read("data.csv")                  // → list of rows
csv.write("out.csv", rows)            // write rows
csv.parse("a,b,c\n1,2,3")             // parse CSV string
csv.encode(rows)                      // → CSV string
```

Flat aliases: `csv_read`, `csv_write`, `csv_parse`, `csv_encode`.

## Net

```
net.online()              // is browser online?
net.cookies()             // document.cookie string
net.url()                 // current URL
```

## Storage (localStorage)

```
storage.get("key")           // value or null
storage.set("key", "value")  // set item
storage.remove("key")        // remove item
storage.clear()              // clear all
storage.all()                // list of {key, value}
```

## Cookies

```
cookies.all()                // list of {name, value}
cookies.get("session_id")    // value or null
cookies.set("key", "val")    // set cookie
cookies.clear()              // clear all cookies
```

## Cryptography (Fernet)

```
cryptography.fernet.generate_key()                        // generate a random key
let key = cryptography.fernet.generate_key()
let token = cryptography.fernet.encrypt(key, "secret data")
cryptography.fernet.decrypt(key, token)                    // "secret data"
```
