# Installation

## From PyPI

```bash
pip install zen-browser-lang
```

## From Source

```bash
git clone https://github.com/ecnord/zen
cd zen
pip install -r requirements.txt
pip install -e .
```

## Requirements

- Python 3.8+
- Chrome or Chromium browser (for full browser automation)
- DrissionPage (installed automatically with pip)

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
