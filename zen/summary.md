## Objective
Build a complete browser-automation shell language (Zen) with rich standard library, concurrency, WhatsApp via Baileys bridge, cookies module via CDP, and consistent error UX.

## Work State
- `wa` module: Baileys bridge — 90+ methods (messaging, 25 group methods incl admin, status, contacts, profile, chat, presence, privacy, callbacks). All Baileys capabilities covered.
- `cookies` module: CDP `Network.getAllCookies` + file-based fallback.
- `load` keyword: now works with built-in modules. Looks up the name in the environment first; if it's a dict, merges items into scope.
- `Browser.start()`: catches all exceptions gracefully.
- Dict missing-key access: now errors with available keys instead of returning `None` silently.
- Shell `_is_complete`: uses brace/paren/bracket balance instead of parser error messages. Prose no longer triggers `...` continuation.
- Dot commands (`.help`, `.exit`, etc.): only KNOWN commands are intercepted; unknown `.x`, `.s`, `.{` etc. fall through to the Zen parser, which shows "Unexpected token: DOT".
- `.67` → `0.67`: lexer now recognizes `\.\d+` as a single NUMBER token.
- `with` is now a soft keyword: `this language has a weaknesss with dot` parses successfully as 7 variable statements, and the interpreter errors "Undefined variable: this" instead of the confusing "Expected LBRACE".
- All other prior features (ranges, try/catch, boolean guard, split, trunc, load/use, etc.) complete.
