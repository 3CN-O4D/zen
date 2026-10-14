# Changelog

All notable changes to Zen. Releases ship via the
[Build & Release workflow](.github/workflows/release.yml) on `v*` tags
(Linux x86_64/aarch64/armv7/armv6, macOS aarch64/x86_64, Windows x86_64).

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
