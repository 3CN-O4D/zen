# pathlib — paths as objects

Object-oriented paths in the spirit of Python's `pathlib`. `Path(p)` (with
`PurePath` / `PurePosixPath` / `PureWindowsPath` aliases) exposes the path as
fields plus a rich method set.

```zen
import pathlib

let p = pathlib.Path("/tmp/x.txt")
print p.name    # x.txt
print p.suffix  # .txt
```

## Fields

`_path`, `str`, `name`, `stem`, `suffix`, `suffixes`, `parent` (a parent
`Path`), `parts`, `anchor`, `is_abs`.

## Methods

| Method | Description |
|--------|-------------|
| `exists()` / `is_file()` / `is_dir()` | Filesystem checks. |
| `is_absolute()` / `is_relative_to(o)` / `is_reserved()` | Path predicates. |
| `absolute()` / `resolve()` / `expanduser()` | Normalization. |
| `joinpath(p)` / `with_name(n)` / `with_suffix(s)` / `with_stem(s)` | Derived paths. |
| `read_text()` / `write_text(d)` / `read_bytes()` / `write_bytes(d)` | File I/O. |
| `touch()` / `mkdir()` / `rmdir()` / `unlink()` / `rename(t)` / `replace(t)` | Mutations. |
| `glob(p)` / `rglob(p)` / `iterdir()` | Directory listing. |
| `stat()` / `owner()` / `group()` | Metadata. |
| `cwd()` / `home()` / `to_string()` | Constructors and rendering. |

## Example

```zen
import pathlib

let p = pathlib.Path("/tmp/demo.txt")
p.write_text("hello")
print p.read_text()  # hello
print p.parent.str   # /tmp
p.unlink()
```
