# Chapter 87: Reference Implementation — Environment Variable Template Expander

Reads a template file containing `${VARIABLE}` placeholders and replaces each
with the corresponding environment variable value. Useful for generating config
files from environment-specific values. Demonstrates string interpolation,
`sys.environ`, and file I/O.

## 87.1 The program (`envtpl.z`)

```zen
import sys

func expand(text) {
    let result = text
    for name in sys.environ {
        result = result.replace("${" + name + "}", sys.environ[name] ?? "")
    }
    return result
}

func read_all(path) {
    with open(path, "r") as f { return f.read() }
}

func write_all(path, text) {
    with open(path, "w") as f { f.write(text) }
}

func main(argv) {
    if len(argv) < 2 {
        print("Usage: zen run envtpl.z <template> [output]")
        print("  Example: zen run envtpl.z config.tmpl > config.conf")
        return 1
    }
    let template = read_all(argv[1])
    let output = expand(template)
    if len(argv) >= 3 {
        write_all(argv[2], output)
        print("wrote " + argv[2])
    } else {
        print(output)
    }
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 87.2 Example session

Template `config.tmpl`:
```
server:
  host: ${HOST}
  port: ${PORT}
  database: ${DB_NAME}
  api_key: ${API_KEY}
```

```
$ HOST=localhost PORT=8080 DB_NAME=mydb API_KEY=secret123 \
  zen run envtpl.z config.tmpl
server:
  host: localhost
  port: 8080
  database: mydb
  api_key: secret123

$ zen run envtpl.z config.tmpl > config.conf   # write to file
```

Undefined variables expand to an empty string (use a default with `??` if you
want fallbacks).

## 87.3 Design notes

- **`${VAR}` placeholders:** matched with a regex; replaced from
  `sys.environ`.
- **Undefined → empty:** variables not set in the environment become `""`.
  Change `?? ""` to `?? "<default>"` for fallbacks.
- **Output:** prints to stdout by default; optional second argument writes to
  a file (useful in build scripts/CI).

## 87.4 Exercises

1. Add a `--default <value>` flag for undefined variables.
2. Add `--strict` mode that fails (exit non-zero) on an undefined variable.
3. Support nested defaults: `${VAR:-fallback}`.
4. Add `--env-file <path>` to load variables from a `.env` file first.
5. Support `${VAR}` and `$VAR` both.
