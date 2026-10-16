# Changelog

All notable changes to Zen. Releases ship via the
[Build & Release workflow](.github/workflows/release.yml) on `v*` tags
(Linux x86_64/aarch64/armv7/armv6, macOS aarch64/x86_64, Windows x86_64).

## [Unreleased]

### Changed — rust-like diagnostics and error vocabulary (engine)

- Every runtime/parse failure now renders rustc-style: message,
  `--> file:line:col`, source snippet with caret, `= help:`/`= note:`
  footer (statement granularity; shared by the tree-walk and bytecode
  engines via compiler line tables, cache FORMAT 3).
- Modules are named in errors: ``module `sqlite` has no function: `x` ``
  (with did-you-mean) instead of `dictionary has no method`; user dicts
  keep the dict wording, calling a non-function names its type
  (``cannot call `f`: value of type `int` is not a function ``).
- Error snippets attribute to the failing module's own file across
  import boundaries; caught errors (`catch as e`) still see raw strings.
- Suite `41_error_vocab`.

### Added — cybersec M0: offline recon (pure Zen, `std/*.z`)

- `hashid` — hash-algorithm identification (`identify` / `likely`): hex
  lengths plus bcrypt/md5crypt/apr1/sha256crypt/sha512crypt/argon2.
- `codec` — `rot13` / `rot47` / repeating-key `xor` / `jwt_decode`
  (structure only) / stepwise `pipe` chains.
- `fingerprint` — offline service/banner fingerprinting (`service` /
  `http_server`) for SSH/FTP/SMTP/HTTP banners; unknown stays unknown.
- Suite `38_cybersec_m0`.

### Added — cybersec M1: reporting and certificate inspection

- `x509` (native) — offline PEM parsing: subject/issuer, serial, validity
  window (ISO + epoch), SAN list, CA flag, signature algorithm, key
  facts. No chain building, verification, or network.
- `tlscheck` (pure Zen) — expiry grading (critical/high/medium), weak
  signatures, short RSA keys, hostname mismatch, self-signed leaves;
  findings plug into `report`.
- `report` (pure Zen) — findings to Markdown/HTML (`create` / `add` /
  `extend` / `summary` / `markdown` / `html` / `save`), critical-first,
  HTML-escaped.
- Suite `39_cybersec_m1`.

### Added — cybersec M2: live fetch and scan-state diffing

- `tls` (native) — `fetch(host, port?, opts?)` runs a verifying TLS
  handshake (SNI + hostname checks) and returns the leaf certificate as
  PEM for the M1 inspection chain; `{insecure: true}` is lab-only.
- `scandiff` (pure Zen) — pure `diff(old, new)` (opened/closed/changed)
  plus sqlite-backed `record` / `restore` / `compare` snapshots.
- `tests/mock_tls.py` (localhost:2443, serves `tests/fixtures/lab.pem`)
  wired into `tests/run_tests.sh` like the FTP mock.
- Suite `40_cybersec_m2`.

### Added — cybersec M3: sweeps and low-profile helpers

- `tlssweep` (pure Zen) — `sweep_host` / `sweep` / `sweep_report` compose
  the fetch→parse→assess→report chain across many hosts; per-host
  failures become `info` findings, never aborts.
- `opsec` (pure Zen) — nmap-style timing templates, jittered delays,
  rotating browser user-agents and benign request headers.
- `x509` SANs now include IPv4 `iPAddress` entries.
- Suite `42_cybersec_m3`; `tests/mock_tls.py` fixture gained an IP SAN.

### Added — cybersec M4: audit framework and guide

- `audit` (pure Zen) — `create` / `set_scope` / `add_check` / `run_checks`
  with scope-guard refusals and check-failure isolation; findings feed
  `report` directly. Detection and reporting only, by design.
- `docs/cybersec.md` — the auditing guide: pipeline, module map, worked
  TLS-posture example, explicit out-of-scope list (no payloads/exploits).
- Suite `43_cybersec_m4`.

## [2.3.0] — 2026-10-14

### Added — standard library (pure Zen, `std/*.z`)

- `soup` — BeautifulSoup-like HTML parsing: lenient parser, `find` /
  `find_all` / CSS `select`, `get_text`, `prettify`, entities.
- `ini` — INI/config files: sections, `DEFAULT` fallbacks, typed getters,
  mutation, serialization.
- `textwrap` — `wrap` / `fill` / `dedent` / `indent` / `shorten`.
- `pprint` — pretty-printing (`pformat` / `pp`).
- `heapq` — binary heaps (`heappush` / `heappop` / `heapify` / `nlargest` /
  `nsmallest`); functional rebinding style.
- `bisect` — binary search and sorted insertion.
- `calendar` — leap years, weekdays, month calendars.
- `fractions` — exact rational arithmetic.
- `copy` — shallow (`shallow`) and deep (`deepcopy`) copies.
- Suites `33_soup`, `34_ini_textwrap_pprint`, `35_heapq_bisect`,
  `36_calendar_fractions_copy`.

### Fixed — interpreter scoping (engine)

- Bare field names now work as nested-assignment roots on first use
  (`data[s][k] = v` no longer errors with "undefined variable").
- Member/index write-backs route to instance fields instead of spawning
  shadowing globals, which previously leaked state across objects.
- Reads and writes agree: locals, then self fields, then globals — matching
  the documented implicit-`self` rule and plain `x = v` behavior.
- Regression suite `37_field_assign`.

### Docs

- Per-module pages for all 32 standard modules; fixed stale Standard
  Library navigation (it referenced files that do not exist) and missing
  Language/Modules targets; new Language overview/types pages; soup
  scraping example.

## [2.2.0]

- Lexicographic string ordering; clarified comparison errors.
- Prior stdlib waves: `json`, `math`, `itertools`, `random`, `operator`,
  `functools`, `statistics`, `glob`, `shutil`, `pathlib`, `hashlib`, `uuid`,
  `collections`, `string`, `tempfile`, `binascii`.
- Nested destructuring; `sys`/`ffi` native modules and sockets suite.
