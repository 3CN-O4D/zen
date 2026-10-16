# 03 — Canonical Syntax Contract

> **Status:** Canonical — the single source of truth for valid Zen code.  
> **Milestone:** M1 (lexer/parser must implement exactly this)  
> **Conformance:** All 32 `tests/cases/*.z` files must parse and run unchanged.

---

## 1. Lexical Conventions

### 1.1 Comments
| Form | Example | Notes |
|---|---|---|
| Line `//` | `// comment` | Standard |
| Line `#` | `# comment` | `#` followed by space/EOF; **not** `#{` or `#(` |
| Block `/* */` | `/* comment */` | Nestable |

**`#` disambiguation (lexer rule):**
- `#` followed by `{` → `DictStart` token (`#{`)
- `#` followed by `(` → `TupleStart` token (`#(`)
- `%` followed by `{` → `SetStart` token (`%{`)
- `#` followed by anything else (space, newline, ident, EOF) → Line comment

### 1.2 Identifiers
- `[a-zA-Z_][a-zA-Z0-9_]*`
- Unicode identifiers: **not supported in v3** (adds lexer complexity, defer to v4)
- Keywords (reserved, cannot be identifiers):
  ```
  let const var global func function def lambda
  if elif else while for in break continue return
  try catch except finally throw
  import from as class new extends inherit super
  match when pub shared true false nil null
  and or not is typeof
  ```

### 1.3 Numbers
| Form | Example | Notes |
|---|---|---|
| Decimal int | `42`, `1_000_000` | `_` separators allowed |
| Hex | `0xFF`, `0Xff` | `0x`/`0X` prefix, `_` allowed |
| Binary | `0b1010`, `0B1010` | `_` allowed |
| Octal | `0o755`, `0O755` | `_` allowed |
| Float | `3.14`, `1.5e-10`, `1E10` | Decimal point required; `e`/`E` for exponent; `_` allowed |
| Multi-dot rejection | `1.2.3` | **Lexer error** — prevents version-number ambiguity |

**Overflow:** Integer literals exceeding i48 range (`±140,737,488,355,327`) are a lexer error.

### 1.4 Strings
| Form | Example | Interpolation | Escapes |
|---|---|---|---|
| Double-quoted | `"hello ${name}"` | Yes (`${expr}`) | Full set |
| Single-quoted | `'hello'` | No | Only `\'` `\\` |
| Triple `"""` | `"""multi\nline"""` | Yes | Full set, newlines literal |
| Triple `'''` | `'''multi\nline'''` | No | Only `\'` `\\`, newlines literal |
| Backtick | `` `raw ${no}` `` | **No** | None (raw) |

**Escapes (in interpolated strings only):**
```
\n \t \r \0 \a \b \f \v \\ \' \" \e (≡ \x1b)
\xNN          (2 hex digits)
\u{NNNN}      (1–6 hex digits, Unicode scalar value)
\U{NNNNNN}    (1–6 hex digits, same as \u)
```

**Inside `${...}`:** The expression is parsed as a normal Zen expression. Escapes in the surrounding string are NOT processed inside the interpolation — the interpolation is a hole in the string.

### 1.5 Bytes Literal
- `bytes([1, 2, 0xFF])` — constructs a `bytes` value from a list of integers (0–255).
- Not a string; no interpolation, no escapes.

### 1.6 Automatic Semicolon Insertion
Newlines act as statement separators **except** after:
- Binary operators (`+`, `-`, `*`, `/`, `%`, `**`, `<<`, `>>`, `&`, `|`, `^`, `&&`, `||`, `??`, `==`, `!=`, `===`, `!==`, `<`, `>`, `<=`, `>=`, `in`, `is`)
- Unary operators (`-`, `!`, `not`, `~`, `typeof`)
- `,`
- `(`, `[`, `#{`, `%{`, `#(`
- `:` (in match arms)
- `->`, `=>` (match separators)
- `if`, `while`, `for`, `try`, `match`, `when` (when followed by block)

Explicit `;` is always allowed and never hurts. Use `;` for oneliners: `let x = 1; let y = 2; print(x + y)`.

---

## 2. Literal Syntax

| Type | Syntax | Example |
|---|---|---|
| `nil` | `nil` / `null` | `let x = nil` |
| `bool` | `true` / `false` | `let ok = true` |
| `int` | `42`, `0x2A`, `0b101010` | `let n = 0xFF` |
| `float` | `3.14`, `1e-10` | `let f = 2.5` |
| `string` | `"..."` | `let s = "hi ${name}"` |
| `bytes` | `bytes([1,2])` | `let b = bytes([0x48, 0x69])` |
| `list` | `[a, b, c]` | `let l = [1, 2, 3]` |
| `dict` | `#{k: v, ...}` | `let d = #{name: "Zen", ver: 3}` |
| `set` | `%{a, b, c}` | `let s = %{1, 2, 3}` |
| `tuple` | `#(a, b, c)` | `let t = #(200, "OK")` |

**Legacy dict `{k: v}`:** Still parses (for `tests/cases/` compat) but emits **deprecation warning**: `"use #{k: v} instead of {k: v}"`. Only parsed as dict when a `:` is found after an ident/string inside `{}`. Otherwise `{ ... }` is a block.

**Spread in containers:** `...expr` supported in list/dict/set/tuple literals.

---

## 3. Statements

### 3.1 Declarations
```
let name = expr           # immutable
const name = expr         # immutable, enforced at runtime
var name = expr           # mutable
global name = expr        # alias for var (module-level)
shared name = expr        # thread-shared mutable (Arc<Mutex<_>>)
```

### 3.2 Assignment
```
target = expr
target += expr    # etc: -= *= /= %= ??= &= |= ^= <<= >>=
target++          # prefix/postfix
target--
```
Valid targets: `name`, `obj.field`, `obj?.field`, `list[idx]`, `dict[key]`.

### 3.3 Control Flow
```
if cond { block } (elif cond { block })* (else { block })?
while cond { block }
for var in iter { block }           # multi-var: for a, b in d.items()
break / continue
try { block } (catch [Type] [as name] { block })* (finally { block })?
throw expr
match expr { pattern [if guard] (=> | -> | :) (expr | block), ... }
when { condition (=> | -> | :) (expr | block), ... }
return [expr]
```

**Match/when arm separators:** `=>`, `->`, `:` all accepted. `->` is also the range operator; disambiguated by context.

**Patterns:**
- `_` (wildcard)
- `name` (binding)
- `literal` (int, float, string, bytes, nil, true, false)
- `[p1, p2, ...]` (list)
- `#{k: p, ...}` (dict)
- `#(p1, p2, ...)` (tuple)
- `p1 | p2` (or-pattern — v4)

### 3.4 Functions & Classes
```
func name(params) { block }          # named function
def name(params) { block }           # alias
function name(params) { block }      # alias
fn(params) { block }                 # anonymous function (expression)
fn(params) => expr                   # arrow function (expression)
lambda params : expr                 # legacy arrow (compat)
(params) => expr                     # required parens
params = name [= default] (, name [= default])*  # no trailing comma
```

```
class Name [extends Parent] {        # single inheritance
    func init(params) { block }      # constructor
    func method(params) { block }    # method (self is implicit receiver)
    var field = default              # instance field
}
new Name(args)                       # instantiation
```

### 3.5 Modules
```
import module                        # plain
import module as alias
import mod.sub.mod as alias
from module import name, name as alias
from module import *                 # star import
```
**Resolution:** `import a.b.c` → `a/b/c.z` or `a/b/c/mod.z`. The `mod.z` file is the canonical package entry point. `main.z` is deprecated with warning.

**Visibility:** `pub` prefix on `func`, `var`, `const`, `class` exports the symbol. Non-`pub` is private to the file.

---

## 4. Expressions (Precedence: Low → High)

| Level | Operators | Associativity |
|---|---|---|
| 1 | `?:` (ternary), `if expr else expr` | Right |
| 2 | `??` (nullish coalesce) | Left |
| 3 | `or` `||` | Left |
| 4 | `and` `&&` | Left |
| 5 | `is` `is not` `===` `!==` `==` `!=` `<` `>` `<=` `>=` `in` `not in` | Left |
| 6 | `|` | Left |
| 7 | `^` | Left |
| 8 | `&` | Left |
| 9 | `<<` `>>` | Left |
| 10 | `+` `-` | Left |
| 11 | `*` `/` `%` | Left |
| 12 | `-` `!` `not` `~` `typeof` (unary) | Right |
| 13 | `**` | Right |
| 14 | `.field` `?.field` `[index]` `call(args)` | Left (postfix) |
| 15 | Primary: literals, identifiers, `(expr)`, list/dict/set/tuple, lambda, arrow, `if` expr, `match`, `when`, `new` | — |

**Range operators:** `start .. end` (exclusive), `start -> end` (inclusive, auto-descends).

**Chained comparisons:** `1 < x < 10` parses as `(1 < x) and (x < 10)`.

---

## 5. Built-in Operators Summary

| Category | Operators |
|---|---|
| Arithmetic | `+`, `-`, `*`, `/`, `%`, `**` |
| Comparison | `==`, `!=`, `===`, `!==`, `<`, `>`, `<=`, `>=` |
| Equality | `is` (plain equality), `is not` |
| Logical | `and`, `or`, `not`, `&&`, `||`, `!` |
| Nullish | `??`, `??=` |
| Bitwise | `&`, `|`, `^`, `~`, `<<`, `>>` |
| Compound assign | `+=`, `-=`, `*=`, `/=`, `%=`, `??=`, `&=`, `|=`, `^=`, `<<=`, `>>=` |
| Increment | `++`, `--` |
| Membership | `in`, `not in` |
| Optional chaining | `?.` |
| Ternary | `cond ? a : b`, `a if cond else b` |

---

## 6. Compatibility Rules (v2 → v3)

| v2 Syntax | v3 Status |
|---|---|
| `{a: 1}` dict | **Parses with deprecation warning** |
| `null` | **Alias for `nil`** (both valid) |
| `def` / `function` | **Aliases for `func`** |
| `except` | **Alias for `catch`** |
| `inherit` | **Alias for `extends`** |
| `this` | **Alias for `self` in methods** |
| `lambda x: expr` | **Supported** (arrow preferred) |
| `raise` | **Not a keyword** — use `throw` |
| `assert` | **Built-in function**, not keyword |
| Hex/binary/octal/sci notation | **Added** (v2 rejected) |

---

## 7. Grammar Reference (EBNF)

See [`05-lexer.md`](05-lexer.md) and [`06-parser.md`](06-parser.md) for the complete machine-readable grammar. This document is the human-readable contract; the parser implementation is the executable contract.