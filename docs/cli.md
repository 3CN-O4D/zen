# CLI Reference

Complete reference for the Zen command-line interface.

## Basic Usage

```
zen                       # Start interactive shell
zen script.z              # Run a script
zen shell                 # Explicit shell mode
```

---

## Subcommands

### `zen`

Starts the interactive REPL shell.

```
$ zen
zen v0.14.0 (debug)
Type "help" for available commands.

>> print("Hello!")
Hello!
>> 2 + 2
4
>> exit
```

### `zen <script>`

Runs a Zen script file.

```
$ echo 'print("Hello from script!")' > hello.z
$ zen hello.z
Hello from script!
```

### `zen shell`

Explicitly starts the interactive shell (same as bare `zen`).

```
$ zen shell
zen v0.14.0 (debug)
>>
```

---

## Flags

### `--help`

Show help message and exit.

```
$ zen --help

Usage: zen [OPTIONS] [FILE]

Options:
  --help        Show this help message
  --version     Show version
  --debug       Enable debug output

Run a .z file to execute a script.
Run without arguments to start the interactive shell.
```

### `--version`

Print the Zen version.

```
$ zen --version
zen 0.14.0
```

### `--debug`

Enable debug output for troubleshooting.

```
$ zen --debug script.z
// Verbose output showing AST, bytecode, etc.
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Runtime error |
| 2 | Parse error |
| 3 | File not found |

### Handling exit codes

```
// In a shell script
$ zen script.z
$ echo $?
0
```

### Using exit() in Zen

```
if args.len < 2 {
    print "Usage: zen script.z <input>"
    exit(1)
}
```

---

## Environment Variables

### `ZEN_PATH`

Override the default module search path.

```
$ ZEN_PATH=/custom/modules zen script.z
```

### `ZEN_DEBUG`

Enable debug output (alternative to `--debug`).

```
$ ZEN_DEBUG=1 zen script.z
```

---

## Shell Commands

Inside the Zen REPL, you can run shell commands with the `!` prefix:

```
>> !ls
file1.z  file2.z  modules/
>> !pwd
/home/user
>> !echo "hello"
hello
>> !git status
On branch main
nothing to commit
```

### Getting shell output

```
>> let result = !echo "hello"
>> print result
hello
```

---

## Script Execution

### Running a script

```
$ zen my_script.z
```

### Script with arguments

```
// my_script.z
print "Script name: {args[0]}"
print "Arguments: {args[1:]}"
```

```
$ zen my_script.z foo bar baz
Script name: my_script.z
Arguments: [foo, bar, baz]
```

### Shebang (Unix)

Make scripts directly executable:

```
#!/usr/bin/env zen
print "Hello, World!"
```

```
$ chmod +x script.z
$ ./script.z
Hello, World!
```

---

## File Extensions

Zen scripts use the `.z` extension by convention.

| Extension | Purpose |
|-----------|---------|
| `.z` | Zen script |
| `.zen` | Alternative extension (also supported) |

---

## Pro Tips

1. **Use `--debug` for troubleshooting.** It shows internal state and error details.
2. **Use shebangs for scripts.** Makes them directly executable on Unix.
3. **Use `args` for CLI arguments.** Available in every script.
4. **Use `exit()` for error codes.** Clean exit with status codes.
5. **Use `!` prefix for shell commands.** Quick access without leaving the REPL.

---

## Common Mistakes

### Forgetting to use `zen` command

```
// WRONG — .z files aren't executable by default on some systems
$ ./script.z

// CORRECT — use zen explicitly
$ zen script.z
```

### Not handling missing arguments

```
// BAD — crashes if no arguments
let name = args[1]

// GOOD — check argument count
if args.len < 2 {
    print "Usage: zen script.z <name>"
    exit(1)
}
let name = args[1]
```

### Path issues

```
// If module not found, check:
// 1. File is in the same directory or ZEN_PATH
// 2. File extension is .z
// 3. Use fs.load_module() for explicit paths
```

---

## See Also

- [Getting Started](getting-started/installation.md) — Installation instructions
- [Shell](getting-started/shell.md) — Interactive shell usage
- [Scripts](getting-started/scripts.md) — Writing and running scripts
- [Troubleshooting](troubleshooting.md) — Common issues and solutions
