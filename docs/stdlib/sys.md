# sys — System-specific parameters

The `sys` module provides access to variables and functions that interact
strongly with the Zen runtime and environment.

```zen
import sys

# 1. Command-line arguments
print(sys.argv) # [zen, script.z, arg1, ...]

# 2. Platform info
print(sys.platform) # e.g., "linux"

# 3. Exit the program
sys.exit(0)
```

## Variables

| Variable | Type | Description |
|----------|------|-------------|
| `argv` | `list` | The list of command-line arguments passed to the script. |
| `platform` | `string` | The operating system platform Zen is running on. |
| `version` | `string` | The Zen version string. |
| `path` | `list` | The search path for modules. |
| `stdin` / `stdout` / `stderr` | `string` | Labels for standard I/O streams. |

## Functions

| Function | Description |
|----------|-------------|
| `exit(code)` | Terminates the current process with the given exit code. |
| `getsizeof(obj)` | Returns an approximate memory size of the object in bytes. |
| `getdefaultencoding()` | Returns the default string encoding ("utf-8"). |

## Examples

### Handling command-line arguments
```zen
import sys

if len(sys.argv) < 2 {
    print("Usage: zen script.z <input>")
    sys.exit(1)
}

print("Processing: ${sys.argv[1]}")
```

## See Also
- [os](../modules/os.md) — Broader operating system interface.
- [cli](../cli.md) — Zen command-line interface.
