# CLI Reference

## zen shell

```bash
zen shell [--headful | --no-headless]
```

Start the interactive shell. Default is headless.

## zen run

```bash
zen run <file.z> [--headful | --no-headless]
```

Execute a `.z` script file.

Inline evaluation with `-e` / `--eval` (can be used multiple times):

```bash
zen run -e 'print "hello"' -e 'print 1 + 1'
zen run -e '
    for i in 1 -> 5 {
        print i
    }
'
```

The `-e` flag can also be used without `run`:

```bash
zen -e 'print "Hello from Zen!"'
```

## zen open

```bash
zen open <url> [--html] [--headful | --no-headless]
```

Open a URL, print the page title. With `--html`, also print the full HTML.

## zen shot

```bash
zen shot <url> [-o/--output <file.png>] [--headful | --no-headless]
```

Take a screenshot of a page. Output defaults to `screenshot.png`.

## zen scrape

```bash
zen scrape <url> -s/--selector <css> [--headful | --no-headless]
```

Scrape text content by CSS selector.

## Global Options

| Option | Description |
|--------|-------------|
| `--headful` | Show browser window |
| `--no-headless` | Same as --headful |
| `--version` | Show version number |

If no subcommand is given, the shell starts.
