# argparser — Command-line argument parsing

The `argparser` module provides a Python-inspired way to define and parse
command-line arguments. It is located in `std/argparser.z`.

```zen
import argparser

# 1. Initialize the parser
var p = argparser.ArgumentParser("A simple example tool")

# 2. Add arguments
p.add_argument("input")                              # Positional argument
p.add_argument("--verbose", {action: "store_true"})  # Boolean flag
p.add_argument("-o", {name: "outfile", value: "out.txt"}) # Option with value

# 3. Parse arguments
var args = p.parse_args()

# 4. Access the values
print("Input file: ${args.input}")
if args.verbose {
    print("Working verbosely...")
}
```

## The ArgumentParser Class

### `ArgumentParser(description)`
Creates a new parser object with a help description.

### `p.add_argument(name, config)`
Defines an argument. The `name` can be a positional name (like `input`) or an
optional flag (starting with `-` or `--`).

**Config options:**
- `action`: Set to `"store_true"` or `"store_false"` for boolean flags.
- `type`: Set to `"int"` or `"float"` for automatic type conversion.
- `value`: Default value if the argument is not provided.
- `short`: An alias for the argument (e.g., `-v` for `--verbose`).
- `help`: A description string for help text.

### `p.parse_args()`
Parses the current `sys.argv` and returns a dictionary containing the results.

## Examples

### Using short flags and defaults
```zen
import argparser
var p = argparser.ArgumentParser("Calculator")
p.add_argument("--count", {short: "-c", type: "int", value: 1})
var args = p.parse_args()

print("Counting to: ${args.count}")
```

## See Also
- [sys](sys.md) — For raw `sys.argv` access.
- [cli](../cli.md) — Zen CLI usage.
