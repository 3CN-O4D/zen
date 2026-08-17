# Shell (Interactive REPL)

The Zen shell is an interactive read-eval-print loop (REPL) for experimenting with the language, testing expressions, and running commands in real time.

## Starting the Shell

```bash
zen shell              # headless mode (default, no browser window)
zen shell --headful    # show browser window
zen shell --no-headless  # same as --headful
```

If no subcommand is given, `zen` starts the shell by default:

```bash
zen
```

---

## The Prompt

The prompt adapts to your session state:

```
zen ❯                                    # no pages visited, clean session
zen[1] ❯                                 # 1 page visited
zen[3] Example Page ❯                    # on page titled "Example Page"
...                                       # multi-line input mode (continuation)
```

### Continuation prompt (`...`)

When you start a multi-line construct (function definition, if block, loop, etc.), the prompt changes to `...`. Continue typing until the block is complete:

```
zen ❯ function greet(name) {
...     return "Hello, " + name + "!"
... }
zen ❯
```

Press **Enter on an empty line** to execute the accumulated buffer, or **Ctrl+C** to cancel.

---

## Evaluating Expressions

Every expression prints its result automatically:

```
zen ❯ 2 + 2
4

zen ❯ "hello".upper()
HELLO

zen ❯ [1, 2, 3].map((x) => x * 2)
[2, 4, 6]

zen ❯ {"a": 1, "b": 2}.keys()
[a, b]
```

### Statements don't print

Assignments, loops, and other statements produce no output:

```
zen ❯ let x = 42
zen ❯ for i in 1 -> 3 { print i }
1
2
3
zen ❯
```

### `_` holds the last expression

```
zen ❯ 2 + 2
4
zen ❯ _ * 10
40
```

---

## Shell Commands

Shell commands start with a dot (`.`). These are processed by the shell itself, not by the Zen interpreter.

| Command | Description |
|---------|-------------|
| `.exit` / `.quit` | Exit the shell |
| `.help` | Show full help reference |
| `.help <topic>` | Show help for a specific topic (e.g., `.help find`) |
| `.help modules` | List all available modules |
| `.help types` | List all data types |
| `.help functions` | List all built-in functions |
| `.help operators` | List all operators |
| `.help keywords` | List all language keywords |
| `.clear` | Clear the screen |
| `.url` | Show the current page URL |
| `.title` | Show the current page title |
| `.vars` | Show all variables in scope |
| `.history` | Show navigation history |
| `.run <file.z>` | Run a script file |
| `.shot <file.png>` | Take a screenshot |
| `.type <expr>` | Show the type of an expression |
| `.dir` | Show the current directory |

### Examples

```
zen ❯ .help
Zen REPL — interactive session
...

zen ❯ .help modules
Available modules (all available as globals, no import needed):
  errors         Python-style error classes with inheritance
  json           JSON encode/decode
  fs             Filesystem operations
  ...

zen ❯ .vars
name: Zen
counter: 42
greet: <function:greet>

zen ❯ .type 42
number

zen ❯ .type "hello"
string

zen ❯ .type [1, 2, 3]
list
```

### Running scripts from the shell

```
zen ❯ .run my_script.z
```

---

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Enter` | Execute current line or buffer |
| `Ctrl+C` | Cancel current input or interrupt running code |
| `Ctrl+D` | Exit the shell (EOF) |
| `Up` / `Down` | Cycle through command history |
| `Tab` | Autocomplete keywords, builtins, and variables |
| `Ctrl+A` | Move cursor to start of line |
| `Ctrl+E` | Move cursor to end of line |
| `Ctrl+K` | Delete from cursor to end of line |
| `Ctrl+U` | Delete from cursor to start of line |
| `Ctrl+L` | Clear screen (when using prompt_toolkit) |

---

## Tab Completion

Press **Tab** to autocomplete. The shell completes:

- **Keywords:** `let`, `const`, `if`, `elif`, `else`, `for`, `while`, `function`, `return`, `class`, `try`, `catch`, `throw`, `import`, `from`, `switch`, `case`, `break`, `continue`
- **Built-in functions:** `print`, `len`, `range`, `str`, `int`, `float`, `bool`, `type`, `sleep`, `input`, `exit`
- **Browser commands:** `go`, `find`, `find_all`, `click`, `fill`, `wait`, `wait_for`, `back`, `forward`, `refresh`, `scroll`, `shot`
- **Module functions:** `http.get`, `fs.read`, `json.parse`, `crypto.sha256`, etc.
- **Special variables:** `_url`, `__url`, `___url`, `_time`, `_date`, `_dir`, `_version`, `_timeout`
- **User-defined variables and functions** from the current session

---

## History

- Commands are saved to `~/.z_history`
- Up to 1000 entries are retained
- History persists across sessions
- If prompt_toolkit is available, auto-suggestions appear as you type (grayed-out text from history)

### Navigating history

- Press **Up** to go to the previous command
- Press **Down** to go to the next command
- Type a partial command then press **Up** to search history for matches

---

## Multi-Line Input

The shell automatically detects incomplete statements and shows the `...` continuation prompt:

### Function definitions

```
zen ❯ function fibonacci(n) {
...     if n <= 1 { return n }
...     return fibonacci(n - 1) + fibonacci(n - 2)
... }
zen ❯ print fibonacci(10)
55
```

### Loops

```
zen ❯ for i in 1 -> 5 {
...     print "Step {i}"
... }
Step 1
Step 2
Step 3
Step 4
Step 5
```

### If blocks

```
zen ❯ let x = 42
zen ❯ if x > 40 {
...     print "Big number"
... } elif x > 20 {
...     print "Medium"
... } else {
...     print "Small"
... }
Big number
```

### Classes

```
zen ❯ class Calculator {
...     __init__ = function(self) {
...         self.result = 0
...     }
...     add = function(self, n) {
...         self.result = self.result + n
...         return self
...     }
... }
zen ❯ let calc = new Calculator()
zen ❯ calc.add(5).add(3)
zen ❯ print calc.result
8
```

---

## Working with the Browser

The shell integrates with a browser for automation:

```
zen ❯ go "https://example.com"
true

zen ❯ .url
https://example.com/

zen ❯ .title
Example Domain

zen ❯ find("h1").text
Example Domain

zen ❯ page.text
"This domain is for use in illustrative examples..."

zen ❯ find_all("a").attr("href")
[https://www.iana.org/domains/example]
```

---

## Pro Tips

### Use the shell as a calculator

```
zen ❯ 2 ** 10
1024

zen ❯ math.pi
3.141592653589793

zen ❯ math.sqrt(144)
12

zen ❯ statistics.mean([1, 2, 3, 4, 5])
3
```

### Test regex patterns

```
zen ❯ re.matches("^\\d+$", "12345")
true

zen ❯ re.findall("[a-z]+", "Hello World 123")
[ello, orld]

zen ❯ re.sub("\\d+", "X", "abc 123 def 456")
abc X def X
```

### Try out JSON manipulation

```
zen ❯ let data = json.parse('{"users": [{"name": "Alice"}, {"name": "Bob"}]}')
zen ❯ data.users[0].name
Alice

zen ❯ json.encode(data, {"pretty": true})
{
  "users": [
    {"name": "Alice"},
    {"name": "Bob"}
  ]
}
```

### Experiment with crypto

```
zen ❯ crypto.md5("hello")
5d41402abc4b2a76b9719d911017c592

zen ❯ crypto.sha256("Zen")
2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
```

---

## Common Mistakes

### Forgetting curly braces

```
// WRONG — the shell won't enter multi-line mode
zen ❯ if true print "hello"

// CORRECT
zen ❯ if true {
...     print "hello"
... }
```

### Using `.exit` in a script

`.exit` is a shell command, not a Zen language feature. In scripts, use `exit(0)`:

```
// In a script (.z file):
exit(0)
```

### Trying to use shell commands in scripts

`.url`, `.title`, `.vars`, `.history` etc. are shell-only. In scripts, use the equivalent built-in variables and functions:

```
// In the shell:
.url         // shows current URL

// In a script:
_url         // current URL variable
```

---

## See Also

- [Quick Start](quickstart.md) — First steps with Zen
- [Scripts](scripts.md) — Running and writing script files
- [CLI Reference](../cli.md) — All CLI commands and flags
- [Language Overview](../language/overview.md) — Complete language reference
