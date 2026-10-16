# Zen — a fast native scripting language

[![release](https://img.shields.io/github/v/release/3CN-O4D/zen)](https://github.com/3CN-O4D/zen/releases)
[![build](https://img.shields.io/github/actions/workflow/status/3CN-O4D/zen/release.yml)](https://github.com/3CN-O4D/zen/actions)
[![license](https://img.shields.io/github/license/3CN-O4D/zen)](https://github.com/3CN-O4D/zen/blob/main/Cargo.toml)

Zen is a Python-flavoured scripting language with a native Rust runtime:
no interpreter to install, single static binary, and batteries included —
HTTP, sockets, crypto, QR codes, SQLite, packet crafting, and a pure-Zen
standard library. Uncaught failures render rustc-style diagnostics
(`--> file:line:col`, snippet, caret, `= help:`), and a growing security
toolkit covers recon, certificate inspection, and audit reporting.

```bash
zen -e 'print "Hello, Zen!"'
import hashlib
print hashlib.sha256("abc").hexdigest()
```

## Install

**Debian/Ubuntu** — grab the `.deb` from
[releases](https://github.com/3CN-O4D/zen/releases) and install:

```bash
sudo dpkg -i zen_2.4.0_amd64.deb
zen --version
```

**From source** — Rust 1.85+ required:

```bash
git clone https://github.com/3CN-O4D/zen
cd zen
./build.sh install        # build + install to /usr/local
zen --version
```

Other flows live in [`scripts/`](scripts/): `install.sh`, `package_deb.sh`,
`cross-compile.sh`, `setup-editors.sh`.

## Quick tour

```zen
# scripting with real modules, no imports needed for natives
var resp = http.get("https://api.github.com/zen")
print(resp.status)                    # 200

# pure-Zen standard library (import first)
import hashlib
import hashid
print hashid.likely(hashlib.md5("abc").hexdigest())   # md5

# security auditing: fetch, grade, report
import tls
import tlscheck
var cert = x509.parse_pem(tls.fetch("example.com", 443))
print cert["not_after"]               # 2026-…Z
```

Errors point at the exact line, in both engines, across module boundaries:

```
error: module `sqlite` has no function: `open_memory`
 --> audit.z:9:1
  |
9 | print sqlite.open_memory()
  | ^~~~~~~~~~~~~~~~~~~~~~~~~~
  |
  | note: did you mean `openMemory`? available functions: escape, close, …
```

## Highlights

| Area | Details |
|------|---------|
| Runtime | Tree-walk + bytecode VM with on-disk cache; `print`-before-error preserved on failure paths |
| Diagnostics | rustc-style snippets for runtime, parse, and lexer errors; module/dict/function/object vocabulary with did-you-mean |
| Standard library | 40+ pure-Zen modules (`std/*.z`): hashing, HTML parsing, config, crypto helpers, threading, and more |
| Security toolkit | `hashid`, `codec`, `fingerprint`, `x509`, `tls`/`tlssweep`, `scandiff`, `opsec`, `audit`, `report` — see [Security Auditing](docs/cybersec.md). Authorized audits only |
| Docs | Full MkDocs site in [`docs/`](docs/): language, modules, stdlib, CLI, troubleshooting |

## Repository layout

Two development lines live here:

```
.                    2.x product line (this README, releases v2.x)
├── src/             native runtime (Rust): http, socket, crypto, tls, x509, …
├── std/             pure-Zen standard library (*.z, imported by name)
├── tests/cases/     black-box suites, run with tests/run_tests.sh
├── docs/            MkDocs documentation
└── zen/             3.0 engine workspace (syntax, VM, JIT, pymport bridge)
    └── crates/      zen-syntax, zen-vm, zen-interp, zen-compiler, zen-jit, zen-cli
```

The `zen/` workspace is the next-generation engine (Cranelift JIT,
embedded-Python interop); the root tree is the stable shippable product.

## Develop

```bash
cargo build                        # debug binary → ./target/debug/zen
bash tests/run_tests.sh            # black-box suites (spins up mock FTP/TLS)
ZEN_NO_BC=1 bash tests/run_tests.sh  # same suites on the tree-walk engine
mkdocs build --strict              # docs must build warning-free
```

## Releases

Versions follow SemVer. Release flow: bump `Cargo.toml` + `CHANGELOG.md`,
commit, tag `vX.Y.Z`, push (CI builds `.deb`/tarballs per platform).

- `v2.x` — stable product line (current: `v2.4.0`)
- `v3.0.0-alpha.0` — first tagged snapshot of the `zen/` engine workspace

## License

MIT — see `license` in [`Cargo.toml`](Cargo.toml).
