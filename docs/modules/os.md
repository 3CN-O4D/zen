# os — Operating system interface

The `os` module provides a way of interacting with the operating system, environment, and processes. It is available globally as `os`.

```zen
# 1. Get environment variables
print(os.getenv("USER"))

# 2. Get system information
print(os.platform()) # e.g., "linux"
print(os.arch())     # e.g., "x86_64"

# 3. Get current process ID
print(os.pid())
```

## System Information

| Function | Description |
|----------|-------------|
| `platform()` | Returns the OS name ("linux", "macos", "windows"). |
| `arch()` | Returns the CPU architecture. |
| `hostname()` | Returns the machine's hostname. |
| `cpu_count()` | Returns the number of logical CPUs. |
| `cwd()` | Returns current working directory. |
| `home()` | Returns user home directory. |

## Environment Variables

| Function | Description |
|----------|-------------|
| `getenv(key)` | Returns the value of an environment variable or null. |
| `setenv(key, val)` | Sets an environment variable. |
| `unsetenv(key)` | Removes an environment variable. |
| `env()` | Returns a dict of all environment variables. |

## Process Control

| Function | Description |
|----------|-------------|
| `pid()` | Current process ID. |
| `pids()` | List of all running process IDs. |
| `kill(pid)` | Terminates the process with the given ID. |
| `exit(code)` | Exits the Zen process with the specified code. |

## Command Execution

| Function | Description |
|----------|-------------|
| `execute(cmd)` | Runs a command in the shell. Returns a dict `{ok, code, stdout, stderr}`. |
| `run(cmd)` | Runs a command and returns `stdout`. Throws on failure. |
| `system(cmd)` | Runs a command and returns the exit code. |
| `popen(cmd)` | Runs a command and returns `{stdout, stderr, code}`. |

## Constants
- `os.sep` (path separator: `/` or `\`)
- `os.linesep` (newline: `\n` or `\r\n`)

## See Also
- [subprocess](subprocess.md) — More control over external commands.
- [fs](fs.md) — For filesystem operations.
