# Color Module (`color`)

ANSI terminal colors.

```zen
color.red("hello")            // red text
color.green("hello")          // green
color.yellow("hello")         // yellow
color.blue("hello")           // blue
color.magenta("hello")        // magenta
color.cyan("hello")            // cyan
color.white("hello")          // white
color.black("hello")          // black

color.bg_red("hello")         // red background
color.bg_green("hello")       // green background
// ... bg_yellow, bg_blue, bg_magenta, bg_cyan, bg_white, bg_black

color.bold("hello")           // bold
color.dim("hello")            // dim
color.italic("hello")         // italic
color.underline("hello")      // underline

color.rgb(255, 0, 0)          // custom RGB
color.bg_rgb(0, 255, 0)       // custom background RGB
color.hex("#ff0000")          // hex color
color.strip(text)             // remove ANSI codes

// Bright variants
color.bright_red("hello")
color.bright_green("hello")
// ... etc for all colors
```
