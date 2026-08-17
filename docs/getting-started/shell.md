# Shell

## Starting the Shell

```bash
zen shell                # headless (invisible browser)
zen shell --headful      # visible browser window
```

## The Prompt

The prompt adapts to your session state:

```
zen ❯                          # no pages visited
zen[1] ❯                       # 1 page visited
zen[1] Example Page ❯         # on page "Example Page"
...                             # multi-line input mode
```

## Multi-Line Input

For multi-line statements (functions, loops, blocks), just keep typing. The `...` prompt appears automatically:

```
zen ❯ function greet(name) {
...     return "Hello, " + name + "!"
... }
```

Press Enter on an empty line to execute the buffer, or Ctrl+C to cancel.

## Shell Commands

| Command | Description |
|---------|-------------|
| `.exit` / `.quit` | Exit the shell |
| `.help` | Show full reference |
| `.help find` | Show details for "find" |
| `.help keywords` | List all help topics |
| `.clear` | Clear screen |
| `.url` | Show current URL |
| `.title` | Show page title |
| `.vars` | Show your variables |
| `.history` | Show navigation history |
| `.run file.z` | Run a script |
| `.shot file.png` | Take screenshot |
| `.type expr` | Show type of expression |
| `.dir` | Show current directory |

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+C` | Cancel/interrupt current line or execution |
| `Ctrl+D` | Exit shell (EOF) |
| `↑` / `↓` | Command history |
| `Tab` | Autocomplete words |
| `Ctrl+L` | Clear screen (prompt_toolkit mode) |

## Tab Completion

Press Tab to autocomplete:

- Keywords: `let`, `go`, `fill`, `click`, `if`, `for`, etc.
- Builtins: `find`, `find_all`, `page_html`, `read_file`, etc.
- Specials: `_url`, `_time`, `_page_html`, etc.

## History

- Commands are saved to `~/.z_history`
- Up to 1000 entries
- Persistent across sessions
- prompt_toolkit also provides auto-suggestions from history

## Result Display

Every expression prints its result:

```
zen ❯ 2 + 2
4
zen ❯ "hello".upper()
HELLO
zen ❯ find("h1")
<h1: Welcome to Example>
zen ❯ fill("#user", "admin")
true
```

`null` values and `_VOID` statements (like `let x = 5`, `go url`) don't print anything.
