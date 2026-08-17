# Installation

## From PyPI

```bash
pip install zen-browser-lang
```

## From Source

```bash
git clone https://github.com/3CN-O4D/zen
cd zen
pip install -r requirements.txt
pip install -e .
```

Or use the install script (handles Termux system deps automatically):

```bash
./install.sh
```

## Requirements

- Python 3.8+
- Chrome or Chromium browser (for full browser automation)
- DrissionPage, requests, beautifulsoup4, lxml, psutil (installed automatically with pip)

## Termux (Android)

pip cannot build the C-extension deps on Termux. Install them from apt first:

```bash
pkg update
pkg install python-psutil python-lxml
pip install -e .
```

or just run `./install.sh`, which does this automatically.

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
