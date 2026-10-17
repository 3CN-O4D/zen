# Zen — a fast native scripting language

[![release](https://img.shields.io/github/v/release/3CN-O4D/zen)](https://github.com/3CN-O4D/zen/releases)
[![build](https://img.shields.io/github/actions/workflow/status/3CN-O4D/zen/release.yml)](https://github.com/3CN-O4D/zen/actions)
[![license](https://img.shields.io/github/license/3CN-O4D/zen)](https://github.com/3CN-O4D/zen/blob/main/Cargo.toml)

The flagship is the **3.0 engine** (`zen/` workspace): a Cranelift JIT
(steady-state 10×+ over CPython-class interpreters), embedded-Python
interop via `pymport`, and Rust-grade coded diagnostics (`zen explain`).
The **2.x line** (repo root) is the stable shippable product: single static
binary, rich module set, and an offline-first security auditing toolkit.

```bash
# 3.0 engine: Python interop in three lines
pymport json
print json.dumps({"a": 1})     # {"a": 1}
```

## The 3.0 engine (flagship, alpha)

Location: [`zen/`](zen/) workspace — `zen-syntax`, `zen-vm`, `zen-interp`,
`zen-compiler`, `zen-jit`, `zen-cli` (tagged `v3.0.0-alpha.0`).

```bash
cd zen
cargo build -p zen-cli --bin zen --all-features
ZEN_JIT=1 ./target/debug/zen run app.z   # JIT tier on
./target/debug/zen explain Z0416        # every error code documented
```

Highlights:

| Capability | Details |
|------------|---------|
| JIT | Cranelift backend, monomorphic inline caches, OSR/deopt, call-site caches |
| `pymport` | `import` Python modules (stdlib, venv, project-local `.py`) with value marshalling and chained tracebacks |
| Errors | Coded diagnostics (`Z01xx–Z04xx`), `zen explain`, Python tracebacks chained under Zen frames |
| Discipline | 46/46 suites green in default and `--all-features` builds; no commits of red code |

Engine docs live in [`zen/docs/`](zen/docs/) (blueprint, principles,
architecture, milestones).

## The 2.x stable line (deprecated)

Location: repo root (current: `v2.4.0`). Install from the `.deb` in
[releases](https://github.com/3CN-O4D/zen/releases):

```bash
sudo dpkg -i zen_2.4.0_amd64.deb
zen --version
```

Or from source (`./build.sh install`; other flows in [`scripts/`](scripts/)).

> ⚠️ **Deprecated:** The 2.x line is in maintenance mode. All new development
> happens in the `zen/` workspace (3.0). Use 2.x only for existing
> deployments; new projects should start with 3.0.

```zen
# batteries included, natives need no import
var resp = http.get("https://api.github.com/zen")
print(resp.status)                    # 200

import hashid                          # pure-Zen stdlib (43 modules)
print hashid.likely("900150983cd24fb0d6963f7d28e17f72")  # md5
```

Uncaught failures render rustc-style diagnostics (`--> file:line:col`,
snippet, caret, `= help:`), and modules get their own error vocabulary
(``module `sqlite` has no function `` — never "dictionary has no method").

### Security auditing toolkit (2.x)

Offline-first recon, certificate inspection, and audit reporting — see
[Security Auditing](docs/cybersec.md). Authorized audits only.

| Module | Role |
|--------|------|
| `hashid`, `codec`, `fingerprint` | hash ID, text transforms, banner fingerprinting |
| `x509`, `tls`/`tlssweep`, `tlscheck` | cert parse, live fetch, expiry/weakness grading |
| `scandiff`, `opsec`, `audit`, `report` | scan diffing, low-profile helpers, audit framework, reports |

## Repository layout

```
.                    2.x product line (releases v2.x)
├── src/             native runtime: http, socket, crypto, tls, x509, …
├── std/             pure-Zen standard library (*.z, imported by name)
├── tests/cases/     black-box suites → tests/run_tests.sh
├── docs/            MkDocs documentation
└── zen/             3.0 engine workspace (releases v3.x) ← flagship
    └── crates/      syntax, vm, interp, compiler, jit, cli
```

Both `zen` binaries share a name; the 3.0 one is built from `zen/`
(`target/debug/zen`), the 2.x one installs to `/usr/bin/zen`.

## Develop

```bash
# 3.0 engine
cd zen && cargo test --workspace --all-features

# 2.x line
cargo build                                  # → ./target/debug/zen
bash tests/run_tests.sh                      # black-box suites (+ mock FTP/TLS)
ZEN_NO_BC=1 bash tests/run_tests.sh          # same suites, tree-walk engine
mkdocs build --strict                        # docs must build warning-free
```

## Releases

SemVer, one namespace, two lines: `v2.x` tracks the stable product,
`v3.x` tracks the engine (`v3.0.0-alpha.0` cut). Tag `vX.Y.Z`, push, and CI
builds the platform artifacts.

## License

MIT — see `license` in [`Cargo.toml`](Cargo.toml).
