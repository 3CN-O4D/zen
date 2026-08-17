# Zen Command-Line Interface (CLI)

The native Rust Zen executable provides a robust CLI for executing files, performing code validation/linting, launching an interactive shell (REPL), and managing packages via an integrated package manager.

## Executing Code

### Run a Script File

Execute a Zen script directly:

```bash
zen run script.z
# or simply
zen script.z
```

### Evaluate Inline Source Code

Evaluate a string of Zen source code immediately from the terminal:

```bash
zen -e 'let total = 2 + 3 * 4; print total'
# or
zen --eval 'for i in 1..4 { print i }'
```

---

## Code Analysis & Validation

### Syntax Validation (`check`)

Parse and validate the syntax of a script without executing it. If successful, prints `ok` and exits with code 0. If it fails, prints the parse error and exits with code 1.

```bash
zen check script.z
```

### Code Quality Linting (`lint`)

Inspect the file for suspicious patterns, code smell, and static errors. It reports:
- References to undefined variables
- Constant reassignments
- Unreachable statements (e.g., after `return`, `break`, `continue`)

```bash
zen lint script.z
```

---

## Interactive Shell (REPL)

Start an interactive read-eval-print-loop session. This session maintains variable declarations across lines.

```bash
zen repl
```

Within the REPL:
- Type any statement to execute it (e.g., `let x = 10`).
- To evaluate and print an expression without typing `print`, prefix it with `:c ` (e.g., `:c x + 5` prints `15`).
- Type `:h` or `:help` for assistance.
- Type `:q`, `:quit`, or `:exit` (or press `Ctrl+D`) to exit.

---

## Integrated Package Manager (`pm`)

Manage and compile third-party packages.

### Initializing a New Module

Create a new module with a `zen.json` manifest and `main.z`:

```bash
zen pm init mymodule         # creates zen.json + main.z
zen pm init                  # uses parent directory name
```

### Installing Packages

Install a package from a GitHub repository, URL, local file, or directory:

```bash
# Install from a GitHub repository (optionally specify tag/version)
zen pm install owner/repo
zen pm install owner/repo@v1.2.0

# Install from a direct HTTP URL or local tarball file
zen pm install https://example.com/packages/foo.tar.gz
zen pm install ./packages/foo.tar.gz

# Install a single .z file
zen pm install helpers.z

# Install a local directory
zen pm install ./my-local-module

# Force re-installation
zen pm install --force owner/repo
```

### Dependency Freeze and Requirements

To install multiple packages from a lock/freeze file:

```bash
zen pm install -r freeze.txt
```

### Listing Installed Packages

Lists all currently installed packages and their versions:

```bash
zen pm list
```

### Freezing Packages

Output all currently installed package specs (suitable for redirection to a freeze file):

```bash
zen pm freeze > freeze.txt
```

### Removing Packages

Uninstall/remove an installed package:

```bash
zen pm remove package_name
# or
zen pm uninstall package_name
```

### Package Information

View metadata information about an installed package:

```bash
zen pm info package_name
```

### Package Verification

Verify that the local files of an installed package match the SHA256 hashes of the source distribution:

```bash
zen pm verify package_name
```

### Publishing and Packing

Pack a directory containing a package into a publishable `.tar.gz` archive:

```bash
zen pm pack ./my-package
```

Publish a packaged directory directly to a git repository remote:

```bash
zen pm publish ./my-package git@github.com:owner/repo.git
```
