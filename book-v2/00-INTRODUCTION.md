# Zen for Dummies: Complete Edition

*The Definitive Guide to the Zen Programming Language*

## Table of Contents

This comprehensive guide is organized into detailed sections covering every aspect of Zen.

### Part 1: Fundamentals
1. [Getting Started](fundamentals/01-getting-started.md) - Installation, first program, REPL
2. [Data Types](fundamentals/02-data-types.md) - Primitives, collections, type system
3. [Operators and Expressions](fundamentals/03-operators.md) - Arithmetic, logical, bitwise
4. [Control Flow](fundamentals/04-control-flow.md) - Conditionals, loops, branching
5. [Functions](fundamentals/05-functions.md) - Defining, calling, parameters, returns
6. [Lambdas and Closures](fundamentals/06-lambdas-closures.md) - Anonymous functions, higher-order functions

### Part 2: Core Concepts
7. [Strings Deep Dive](core/07-strings.md) - Manipulation, formatting, methods
8. [Lists and Arrays](core/08-lists.md) - Operations, methods, iteration
9. [Dictionaries and Maps](core/09-dictionaries.md) - Key-value pairs, methods
10. [Modules and Imports](core/10-modules-imports.md) - Creating, importing, module system
11. [Error Handling](core/11-error-handling.md) - Try/catch, throw, debugging

### Part 3: Object-Oriented & Advanced Programming
12. [Classes and Objects](advanced/12-classes-objects.md) - OOP, inheritance, polymorphism
13. [File I/O](advanced/13-file-io.md) - Reading, writing, `with` statement
14. [Memory and References](advanced/14-memory-references.md) - Values, references, ownership

### Part 4: Standard Library Modules (Detailed)
15. [math Module](modules/15-math.md) - Mathematical functions and constants
16. [random Module](modules/16-random.md) - Random numbers and selection
17. [string Module](modules/17-string.md) - String utilities
18. [collections Module](modules/18-collections.md) - Advanced data structures
19. [json Module](modules/19-json.md) - JSON parsing and serialization
20. [datetime/time Module](modules/20-time-datetime.md) - Time and date operations
21. [pathlib & os Module](modules/21-path-os.md) - File system operations
22. [re (Regex) Module](modules/22-regex.md) - Pattern matching
23. [base64 & binascii](modules/23-base64-binascii.md) - Encoding/decoding
24. [hashlib Module](modules/24-hashlib.md) - Hashing algorithms
25. [crypto Module - Complete Guide](modules/25-crypto-complete.md) - Cryptography, encryption, JWT, X.509

### Part 5: Networking & Protocols
26. [Networking Fundamentals](networking/26-networking-fundamentals.md) - Sockets, TCP/IP basics
27. [socket Module](networking/27-socket-module.md) - Low-level socket programming
28. [HTTP Protocol](networking/28-http-protocol.md) - HTTP/1.1, requests, headers
29. [requests Module](networking/29-requests-module.md) - High-level HTTP client
30. [http Server Module](networking/30-http-server.md) - Building HTTP servers
31. [HTTPS & SSL/TLS Certificates](networking/31-https-ssl-certs.md) - Secure connections, certificates
32. [DNS Module](networking/32-dns.md) - Domain name resolution
33. [url Module](networking/33-url.md) - URL parsing and manipulation
34. [Raw Sockets](networking/34-raw-sockets.md) - Packet-level networking
34b. [Networking Deep Dive](networking/68-networking-deep-dive.md) - Full OSI packet trace + socket labs

### Part 6: Concurrency & Threading
35. [Threading Fundamentals](concurrency/35-threading-fundamentals.md) - Threads, spawning, execution
36. [Channels](concurrency/36-channels.md) - Inter-thread communication
37. [Mutexes & Locks](concurrency/37-mutexes-locks.md) - Synchronization primitives
38. [Thread Safety & Patterns](concurrency/38-thread-safety-patterns.md) - Best practices

### Part 7: FFI, System & Interoperability
39. [FFI Fundamentals](ffi/39-ffi-fundamentals.md) - Foreign Function Interface basics
40. [FFI in Practice](ffi/40-ffi-practice.md) - Calling C functions, handles
41. [sys Module](system/41-sys-module.md) - System information and operations
42. [pymport - Python Interoperability](interop/42-pymport-python-interoperability.md) - Using Python from Zen (setup, sync, examples)

### Part 8: Practical Projects
43. [CLI Tools](projects/43-cli-tools.md) - Building command-line applications
44. [Web Scraping & Automation](projects/44-web-automation.md) - Bots, scrapers, automation
45. [REST API Client](projects/45-rest-api-client.md) - API integration projects
46. [File Processing Tools](projects/46-file-processing.md) - Data processing pipelines
47. [Network Security Tools](projects/47-security-tools.md) - Ethical hacking utilities
48. [Capstone Projects](projects/48-capstone.md) - Full applications

### Part 10: Cookbook — Complete Recipes
55. [Strings & Text](cookbook/55-cookbook-strings-text.md) - case, slugify, acronym, email, wrap, cipher
56. [Files & Data](cookbook/56-cookbook-files-data.md) - read/write, CSV, JSON, find files, hashing
57. [Networking & Web](cookbook/57-cookbook-networking-web.md) - HTTP, sockets, DNS, URL, server
58. [Concurrency & System](cookbook/58-cookbook-concurrency-system.md) - threads, channels, mutexes, sys
59. [Crypto & Security](cookbook/59-cookbook-crypto-security.md) - hash, encrypt, sign, JWT, passwords, TOTP

### Part 11: Reference Implementations (complete programs)
60. [Static Site Generator](reference/60-reference-static-site-generator.md) - Markdown to HTML site builder
61. [Multi-Room Chat Server](reference/61-reference-chat-server.md) - concurrent TCP chat with rooms
62. [Persistent Key-Value Store](reference/62-reference-keyvalue-store.md) - TCP store with JSON snapshots
63. [TCP Port Scanner](reference/63-reference-port-scanner.md) - educational scanner with timeouts
64. [Log Analyzer](reference/64-reference-log-analyzer.md) - level counts, errors, reports
65. [HTTP Uptime Monitor](reference/65-reference-http-monitor.md) - endpoint checks with response times
66. [File Encryptor (AES-GCM)](reference/66-reference-file-encryptor.md) - PBKDF2 + authenticated encryption
67. [Website Link Checker](reference/67-reference-link-checker.md) - fetch, extract, verify links
69. [Task Queue with Worker Pool](reference/69-reference-task-queue.md) - channels, threads, mutexes
70. [Password Generator & Strength Checker](reference/70-reference-password-tool.md) - crypto, argon2id
71. [HTTP Key-Value REST Server](reference/71-reference-http-kv-server.md) - http.server, JSON, routing
72. [File Backup / Synchronizer](reference/72-reference-file-backup.md) - content-addressed copying
73. [CSV to JSON Converter](reference/73-reference-csv-json.md) - tabular data conversion
74. [Task Scheduler (cron-like)](reference/74-reference-scheduler.md) - interval-based background tasks
75. [DNS Lookup Tool](reference/75-reference-dns-tool.md) - A/AAAA/MX/TXT/NS/PTR records
76. [TCP Chat Client](reference/76-reference-chat-client.md) - concurrent socket client
77. [Static File HTTP Server](reference/77-reference-static-file-server.md) - serve files with MIME types
78. [grep Tool](reference/78-reference-grep-tool.md) - regex pattern search in files
79. [Test Runner](reference/79-reference-test-runner.md) - minimal test framework
80. [URL Shortener](reference/80-reference-url-shortener.md) - secure random codes + HTTP redirect
81. [Tail -f (File Watcher)](reference/81-reference-tail-follow.md) - watch files for appended lines
82. [HTTP Load Tester](reference/82-reference-http-load-tester.md) - concurrent load + latency stats
83. [Host Availability Monitor](reference/83-reference-host-monitor.md) - concurrent TCP port checks
84. [JSON Pretty Printer](reference/84-reference-json-pretty.md) - validate, pretty-print, minify
85. [Base64 Tool](reference/85-reference-base64-tool.md) - encode/decode files and strings
86. [Word Frequency Analyzer](reference/86-reference-word-frequency.md) - top-N word counts
87. [Env Template Expander](reference/87-reference-env-template.md) - ${VAR} from environment
88. [File Deduplicator](reference/88-reference-file-dedup.md) - content-addressed duplicate detection
89. [JSON API Mock Server](reference/89-reference-mock-api.md) - canned HTTP responses
90. [Settings Manager](reference/90-reference-settings.md) - INI-style config read/write
91. [Log Rotator](reference/91-reference-log-rotator.md) - size-based log rotation
92. [HTTP Response Cache (TTL)](reference/92-reference-http-cache.md) - cached origin fetches
93. [Batch File Renamer](reference/93-reference-batch-rename.md) - bulk rename with dry-run
94. [Static Blog Engine](reference/94-reference-static-blog.md) - Markdown posts + sorted index
95. [Markdown Slides Generator](reference/95-reference-slides.md) - single-file HTML slideshow

### Part 9: Runtime Internals
50. [Zen VM and Runtime Internals](advanced/50-vm-runtime-internals.md) - interpreter, JIT, GC, memory
51. [Type System In Depth](advanced/51-type-system-in-depth.md) - types, coercion, equality
52. [Bytecode Reference](advanced/52-bytecode-reference.md) - bytecode opcodes and execution
53. [Garbage Collection](advanced/53-garbage-collection.md) - memory management and cycles
54. [Cranelift JIT Compiler](advanced/54-jit-compiler.md) - JIT, inline caches, OSR, deopt

### Appendices
- [Appendix A: Built-in Functions](appendix/a-builtins.md)
- [Appendix B: Error Codes Reference](appendix/b-error-codes.md)
- [Appendix C: Zen CLI Reference](appendix/c-cli-reference.md)
- [Appendix D: Standard Library Index](appendix/d-stdlib-index.md)
- [Appendix E: Quick Reference Card](appendix/e-quick-reference.md)
- [Appendix F: Complete Language Reference](appendix/f-complete-reference.md)
- [Appendix G: Exercise Solutions](appendix/g-exercise-solutions.md)
- [Appendix H: Troubleshooting & FAQ](appendix/h-troubleshooting.md)
- [Appendix I: Platform Differences](appendix/i-platform-differences.md)
- [Appendix J: Standard Library Complete API Reference](appendix/j-stdlib-api-reference.md)

## About This Book

This book is written for complete beginners but goes deep into every aspect of Zen. Each module gets its own detailed chapter with theory, examples, common pitfalls, and exercises.
