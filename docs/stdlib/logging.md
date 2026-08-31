# logging — Structured logging

The `logging` module provides a flexible framework for generating log messages
from Zen applications. It follows a design similar to Python's logging module.

```zen
import logging

# Configure basic logging
logging.basicConfig({level: logging.INFO})

# Log messages
logging.info("Everything is working")
logging.warning("Something might be wrong")
logging.error("An actual error occurred")
```

## Log Levels

| Level | Value | Usage |
|-------|-------|-------|
| `logging.DEBUG` | 10 | Detailed information for diagnostics. |
| `logging.INFO` | 20 | Confirmation that things are working as expected. |
| `logging.WARNING` | 30 | An indication that something unexpected happened. |
| `logging.ERROR` | 40 | A more serious problem; the program can't perform a function. |
| `logging.CRITICAL` | 50 | A serious error; the program may be unable to continue. |

## Functions

| Function | Description |
|----------|-------------|
| `basicConfig(config)` | Sets up the logger (keys: `level`, `filename`, `stream`). |
| `debug(msg)` / `info(msg)` | Logs a message at the specified level. |
| `warning(msg)` / `error(msg)` | Logs a message at the specified level. |
| `critical(msg)` | Logs a message at the critical level. |
| `getLogger(name)` | Returns a named logger object. |
| `setLevel(level)` | Sets the global logging threshold. |

## Handlers

You can log to different destinations by adding handlers.

- `FileHandler(filename)`: Writes logs to a file.
- `StreamHandler()`: Writes logs to the terminal (stdout).

```zen
import logging
logging.addHandler(logging.FileHandler("app.log"))
logging.info("This will be saved to app.log")
```

## Named Loggers

Use named loggers to track logs from different parts of your application.

```zen
var log = logging.getLogger("auth")
log.setLevel(logging.DEBUG)
log.info("User logged in")
```

## See Also
- [datetime](../modules/datetime.md) — Used for log timestamps.
- [fs](../modules/fs.md) — Used for file logging.
