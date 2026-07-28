# Troubleshooting

## Installation Issues

### DrissionPage not found

```bash
pip install DrissionPage
```

### Chromium not found

Zen uses your system Chrome/Chromium. Install Chrome or use `--connect` to attach to a running browser.

### Permission Errors

```bash
# Install with user flag
pip install -e . --user

# Or override externally managed environment
pip install -e . --break-system-packages
```

## Common Runtime Issues

### Element not found

- Check the selector is correct (use `page.html` to inspect the page)
- Wait for the element: `wait_for(".my-selector")`
- Use text-based finding: `find(text="Visible Text")`

### Timeout

- Increase the timeout: `_timeout = "10s"`
- Check if the page loaded correctly
- Use `wait_for_network()` before finding elements

### "Element is not an \<input\>"

- You're calling `.fill()` on a non-input element (like a `<label>`)
- Find the actual input: `page.inputs` shows all input fields
- Use the input's CSS selector instead

### Page not loading

- Check the URL is correct and reachable
- Some sites block headless browsers — try `--headful`
- Add protocol: `"https://example.com"` not `"example.com"`

### "Cannot find module"

- Make sure the path is correct: `include "lib/list.z"`
- Check that `lib/` exists in the same directory as your script

### "Undefined variable"

- Variable name is misspelled or not declared
- Check for typos

### "Not callable"

- Trying to call something that isn't a function
- Check if the variable is a function

## Chrome/Chromium Issues

### Sandbox errors on Linux

```bash
# Run with sandbox disabled (for Docker/CI):
export CHROME_FLAGS="--no-sandbox"
```

### Display errors on headless servers

```bash
# Install Xvfb for virtual display
sudo apt install xvfb

# Run with virtual display
xvfb-run zen shell
```

## Performance Issues

### Slow execution

- Use headless mode (default) for faster execution
- Minimize `wait` calls — use `wait_for` instead
- Cache selectors in variables

### Memory issues

- Close browser when done: `conn.disconnect()`
- Use `--no-history` flag to disable history

## Getting Help

- Check the [Shell Usage](getting-started/shell.md) section
- Run `.help` in the shell for built-in help
- Report issues at https://github.com/ecnord/zen/issues
