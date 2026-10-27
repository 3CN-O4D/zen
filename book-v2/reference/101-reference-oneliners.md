# Chapter 101: Reference — Useful Zen One-Liners

Practical, copy-pasteable one-liners for common tasks. Run with `zen -e '...'` or
save to a file and run with `zen run`.

## 101.1 System & files

```zen
# Print Zen version
zen --version

# List files in current directory (Zen one-liner)
zen -e 'import os; print(os.listdir("."))'

# Print current working directory
zen -e 'import os; print(os.getcwd())'

# Read and print a file
zen -e 'print(open("README.md").read())'

# Count lines in a file
zen -e 'print(len(open("file.txt").read().splitlines()))'

# Create a directory
zen -e 'import os; os.makedirs("output")'

# Check if a file exists
zen -e 'import pathlib; print(pathlib.Path("file.txt").exists())'
```

## 101.2 Numbers & math

```zen
# Compute 2^10
zen -e 'print(2 ** 10)'          # 1024

# Round a float
zen -e 'import math; print(math.round(3.14159))'   # 3

# Check if a number is prime
zen -e 'func p(n){if n<2{return false}var i=2;while i*i<=n{if n%i==0{return false}i+=1}return true};print(p(17))'   # true

# Fibonacci
zen -e 'func f(n){if n<2{return n}return f(n-1)+f(n-2)};print(f(10))'   # 55

# Generate a random number 1-100
zen -e 'import random; print(random.randint(1,100))'
```

## 101.3 Strings & text

```zen
# Uppercase a string
zen -e 'print("hello zen".upper())'   # HELLO ZEN

# Reverse a string
zen -e 'let s="zen";print(s[len(s)-1::-1])'   # nez   (if slicing step supported)

# Count words in a string
zen -e 'print(len("the quick brown fox".split(" ")))'   # 4

# Replace text
zen -e 'print("hello world".replace("world","Zen"))'   # hello Zen

# Split a CSV line
zen -e 'print("a,b,c".split(","))'   # ["a","b","c"]

# Slugify
zen -e 'let t="Hello World!";let o="";for c in t.lower(){if c.isalnum() or c==" "{o=o+c}};print(o.replace(" ","-"))'   # hello-world
```

## 101.4 JSON

```zen
# Parse JSON
zen -e 'import json;print(json.parse("{\"a\":1}")["a"])'   # 1

# Serialize to JSON
zen -e 'import json;print(json.stringify(#{a:1,b:2}))'   # {"a":1,"b":2}

# Pretty-print JSON
zen -e 'import json;print(json.pretty(#{a:1,b:[2,3]}))'
```

## 101.5 HTTP & networking

```zen
# Fetch a URL
zen -e 'import requests;let r=requests.get("https://example.com",timeout=5000);print(r.status_code)'

# Resolve a domain
zen -e 'import dns;print(dns.resolve("example.com","A"))'

# Check if a port is open
zen -e 'import socket;let s=socket.socket(socket.AF_INET,socket.SOCK_STREAM);s.settimeout(2000);print(s.connect(("example.com",443))==nil)'
```

## 101.6 Crypto & security

```zen
# SHA-256 of a string
zen -e 'import hashlib;print(hashlib.sha256("hello"))'

# Generate a secure token
zen -e 'import crypto;print(crypto.secure_token())'

# Hash a password (argon2id)
zen -e 'import crypto;print(crypto.argon2id_hash("hunter2"))'

# Generate a TOTP token
zen -e 'import crypto;print(crypto.totp("JBSWY3DPEHPK3PXP"))'
```

## 101.7 Date & time

```zen
# Current epoch seconds
zen -e 'import time;print(time.time())'

# Format current date
zen -e 'import datetime;let n=datetime.now();print(datetime.strftime("%Y-%m-%d %H:%M:%S",n))'

# Sleep for 1 second
zen -e 'import time;time.sleep(1000);print("awake")'
```

## 101.8 Concurrency (one-liner concepts)

```zen
# Run two things in parallel
zen -e 'import threading;threading.Thread(fn(){print("a")}).start();threading.Thread(fn(){print("b")}).start()'

# Sleep and print
zen -e 'import time;time.sleep(1000);print("done after 1s")'
```

## 101.9 Exercises

1. Write a one-liner that reads a file, uppercases it, and prints it.
2. Write a one-liner that fetches a URL and prints only the status code.
3. Write a one-liner that lists all `.z` files in the current directory.
4. Combine two one-liners into a small script saved to a file.
