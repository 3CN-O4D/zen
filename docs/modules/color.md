# color — ANSI colors and styling

The `color` module provides helpers for printing colored and styled text to the terminal using ANSI escape codes. It is available globally as `color`.

```zen
# 1. Basic foreground colors
print(color.red("This is an error"))
print(color.green("Success!"))

# 2. Styling
print(color.bold("Important Text"))
print(color.underline("Look at this"))

# 3. Combining styles
print(color.bg_red(color.white(color.bold(" ALERT "))))
```

## Functions & Styles

### Foreground Colors
`black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`
(And bright variants: `bright_red`, etc.)

### Background Colors
`bg_black`, `bg_red`, `bg_green`, `bg_yellow`, `bg_blue`, `bg_magenta`, `bg_cyan`, `bg_white`

### Styles
| Style | Description |
|-------|-------------|
| `bold` | Heavy text. |
| `dim` | Faded text. |
| `italic` | Slanted text. |
| `underline` | Line underneath. |
| `blink` | Flashing text. |
| `reverse` | Swaps foreground and background. |
| `hidden` | Invisible text. |
| `strike` | Line through text. |
| `reset` | Resets all styling. |

## Advanced Colors

### RGB and Hex
```zen
# Custom RGB (0-255)
print(color.rgb(255, 165, 0, "Orange Text"))

# Hex strings
print(color.hex("#FF00FF", "Magenta"))

# Background RGB
print(color.bg_rgb(0, 0, 0, "Black background"))
```

## Utilities
- `color.strip(text)`: Removes all ANSI escape codes from a string (useful for logging to files).

## See Also
- [os](os.md) — For checking terminal type.
