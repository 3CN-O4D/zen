# Screenshots

## Viewport Screenshot

```
shot "page.png"
```

## Full Page Screenshot

```
shot "full.png" full
```

## Element Screenshot

```
find(".header").screenshot("header.png")
```

## CLI Screenshots

```bash
zen shot https://example.com -o screenshot.png
```

## Programmatic Screenshots

```
go "https://example.com"
shot "screenshot.png"
```

## Headful Mode

For screenshots that require seeing the browser:

```bash
zen shot https://example.com --headful -o screenshot.png
```
