# ini — INI / config files

Config-file parsing in the spirit of Python's `configparser`. Sections,
`DEFAULT` fallbacks, typed getters, mutation, and serialization.

```zen
import ini

let cfg = ini.parse("[server]\nhost = example.com\nport = 8080\n")
print cfg.get("server", "host", "?")  # example.com
```

## Format

- `[section]` headers; `key = value` or `key : value` pairs.
- `#` / `;` comments (only at line start or after whitespace, so URLs survive).
- Indented lines continue the previous value.
- `[DEFAULT]` values act as fallbacks for every section.

## Config methods

| Method | Description |
|--------|-------------|
| `get(section, option, dflt)` | String value: section first, then `DEFAULT`, then `dflt`. |
| `getint(section, option, dflt)` | Integer value (or `dflt` when missing). |
| `getfloat(section, option, dflt)` | Float value (or `dflt` when missing). |
| `getbool(section, option, dflt)` | Boolean: `1/yes/true/on` → true, `0/no/false/off` → false, else `dflt`. |
| `set(section, option, value)` | Stores a value (creates the section if needed). |
| `set_default(option, value)` | Stores a `DEFAULT` fallback. |
| `has_section(name)` / `has_option(section, option)` | Existence checks. |
| `options(section)` | Option names in a section. |
| `section_names()` | All section names. |
| `write()` | Serializes back to INI text (defaults first). |

## Functions

| Function | Description |
|----------|-------------|
| `parse(text)` | Parses INI text into a `Config`. |
| `read(path)` | Reads a file and parses it. |

## Example

```zen
import ini

let cfg = ini.parse("[db]\npath = /var/db\n[limits]\nopen = 100\n")
cfg.set("limits", "open", "200")
print cfg.getint("limits", "open", 0)  # 200
print cfg.write()
# [db]
# path = /var/db
#
# [limits]
# open = 200
```
