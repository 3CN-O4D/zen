# subprocess — External commands

The `subprocess` module provides powerful tools for running external programs and interacting with their input/output streams. It is available globally as `subprocess`.

```zen
# 1. Run a command and wait
var res = subprocess.run(["ls", "-l"])
print("Exit code: ${res.code}")

# 2. Capture output
var output = subprocess.check_output(["echo", "hello"])
print("Output: ${output}") # hello
```

## Functions

| Function | Description |
|----------|-------------|
| `run(args)` | Runs a command (list of strings). Returns a dict with `code`, `stdout`, `stderr`. |
| `call(args)` | Runs a command and returns the exit code only. |
| `check_output(args)` | Runs a command and returns its `stdout` as a string. Throws if the exit code is non-zero. |

## The Result Dictionary
The `subprocess.run()` function returns a dictionary:
- `code`: The process exit code (int).
- `stdout`: The standard output string.
- `stderr`: The standard error string.

## Examples

### Running a shell pipeline
While `subprocess` works with lists of arguments for safety, you can use the `os` module for simple shell strings.

```zen
if subprocess.call(["grep", "-q", "Zen", "README.md"]) == 0 {
    print("Found it!")
}
```

## See Also
- [os](os.md) — For `os.execute()` and `os.run()`.
- [fs](fs.md) — For checking if files exist before running commands on them.
