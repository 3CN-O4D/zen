# Command-line usage

Zen ships as a single native Rust binary. The basic shape:

```
zen <file.z>            # run a script
zen run <file.z>        # the same, explicit
zen -e <source>         # evaluate a source string
zen repl                # interactive REPL
zen --version           # print version
```

## Running scripts

```bash
$ cat hello.z
print("Hello, world!")

$ zen hello.z
Hello, world!

$ zen run hello.z
Hello, world!
```

Pass filename arguments separately; scripts read them via the `os`/CLI API,
not from `ARGV` globals.

## Evaluate a string (`-e`)

```bash
$ zen -e 'print(1 + 1)'
2
```

Useful in shell pipelines and one-liners.

## check — parse without running

Validates syntax (and basic semantics) without executing:

```bash
$ zen check hello.z          # exit 0, silent on success
ok
$ zen check broken.z         # exit 1
zen: expected variable name
```

Use in CI/pre-commit to catch syntax errors fast.

## lint — static warnings

Reports suspicious patterns without executing (still returns 0 on success):

```bash
$ zen lint hello.z
no issues found
```

## repl — interactive session

```bash
$ zen repl
```

REPL helpers:

| Command | Effect |
|---------|--------|
| `:help` / `:h` | overview |
| `:help modules` | list every registered module |
| `:help types` | the value types |
| `:help functions` / `:help builtins` | built-in function list |
| `:help operators` | operator reference |
| `:help keywords` | keyword list |
| `:help <name>` | help for a module or builtin |
| `:q` | quit |

## Version & help

```bash
$ zen --version
zen 2.1.0 (native Rust runtime)
$ zen --help           # same as -h / help
```

## Package manager (`zen pm`)

Zen has a built-in package manager for sharing modules.

| Command | Purpose |
|---------|---------|
| `zen pm init [name]` | scaffold a module (`zen.json` + `main.z`) |
| `zen pm install <spec>` | install via `owner/repo`, URL, `.z` file, or local dir |
| `zen pm install --force <spec>` | reinstall |
| `zen pm install -r <freeze.txt>` | install from a freeze file |
| `zen pm list` | installed packages |
| `zen pm freeze` | write a dependency freeze file |
| `zen pm remove <name>` | uninstall |
| `zen pm info <name>` | package metadata |
| `zen pm verify <name>` | check sha256 against source |
| `zen pm pack <dir>` | build a publishable tarball |
| `zen pm publish <dir> <git-remote>` | publish via git |

```bash
$ zen pm init mylib
$ zen pm install owner/mylib
$ zen pm list
```

Imported packages resolve automatically via `import name` (see
[imports.md](imports.md) for the lookup order).

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | success (or `check`/`lint` with no errors) |
| `1` | parse/validate/lint failure, or uncaught script error |

`exit(n)` in a script overrides the process exit code.