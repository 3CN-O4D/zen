# Interacting

## Getting an Element

```
let el = find("h1")                    // CSS
let el = find_by_text("Welcome")       // text
let el = find_by_url("example.com")    // URL
let el = first(".item")               // alias for find
```

## Properties

| Property | Description |
|----------|-------------|
| `.text` | Visible text content |
| `.html` | Inner HTML |
| `.exists` | Is element in the DOM? |
| `.tag` | Tag name (e.g. "h1", "a") |
| `.url` | `href` attribute value |
| `.src` | `src` attribute value |
| `.is_visible` | Is element visible? |
| `.is_enabled` | Is element enabled? |
| `.is_checked` | Is checkbox/radio checked? |

## Methods

| Method | Description |
|--------|-------------|
| `.attr("href")` | Get attribute value |
| `.click()` | Click the element (returns self) |
| `.fill("value")` | Fill form field (returns self) |
| `.check()` | Check checkbox (returns self) |
| `.uncheck()` | Uncheck checkbox (returns self) |
| `.select("option")` | Select option (returns self) |
| `.hover()` | Hover over element |
| `.screenshot("path")` | Screenshot the element |
| `.find("span")` | Find child element |
| `.find_all("span")` | Find all children |
| `.play()` | Start media playback |
| `.pause()` | Pause media playback |
| `.download("path")` | Download media or link target |

## Method Chaining

All action methods return the element itself for chaining:

```
find("#user").fill("admin").check()
find(".login-form").find("button").click()
```

## Media Properties (video/audio)

| Property | Description |
|----------|-------------|
| `.duration` | Total duration in seconds |
| `.paused` | Is playback paused? |
| `.ended` | Has playback ended? |
| `.muted` | Get/set muted state |
| `.muted = true` | Mute |
| `.volume = 0.5` | Set volume (0.0 to 1.0) |
| `.current_time = 10` | Seek to position (seconds) |
| `.loop = true` | Enable looping |
