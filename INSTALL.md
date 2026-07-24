# Installation

## From PyPI

```bash
pip install zen-browser-lang
playwright install chromium
```

## From Source

```bash
git clone https://github.com/ecnord/zen
cd zen
pip install -r requirements.txt
pip install -e .
playwright install chromium
```

## Requirements

- Python 3.8+
- Chromium (installed via `playwright install chromium`)

## Verify

```bash
zen --version
zen shell
```

Inside the shell, try:

```
print "hello world"
range(10)
```
