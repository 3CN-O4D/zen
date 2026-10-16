# 05 — Lexer

> **Status:** Draft for Milestone 1. Lexical rulings below derive from the ruling table in
> [`00-blueprint-review.md`](00-blueprint-review.md) §6 and the baseline grammar at
> `/home/bard/Desktop/zen/docs/grammar.md`. Where the two disagree, the ruling is marked
> **RULING** and carries a test id in §11.
> **Milestone:** M1 (Lexer + AST + parser) — build order in
> [`00-blueprint-review.md`](00-blueprint-review.md) §2 A6.
> **Baseline:** the Zen 2.2.0 lexer at `src/runtime.rs:672-1331` is the conformance oracle. Every
> behaviour preserved from it is cited by line.

## Contents

- [1. Purpose and scope](#1-purpose-and-scope)
- [2. Input and output](#2-input-and-output)
- [3. `TokenKind`: the complete table](#3-tokenkind-the-complete-table)
- [4. The `#` sigil: disambiguation](#4-the--sigil-disambiguation)
- [5. Numbers](#5-numbers)
- [6. Strings](#6-strings)
- [7. Statement separation and newlines](#7-statement-separation-and-newlines)
- [8. Line continuation](#8-line-continuation)
- [9. Errors and diagnostics](#9-errors-and-diagnostics)
- [10. Implementation plan](#10-implementation-plan)
- [11. Test plan](#11-test-plan)
- [12. Performance](#12-performance)

---

## 1. Purpose and scope

**The lexer guarantees this, in one sentence:** given a byte string that is valid UTF-8, it produces
a `Vec<Token>` in which every token carries its exact byte range, comments and whitespace are gone,
newlines are classified as *terminators* or *continuations* according to §7, and the only failure
modes are `LexError`s with spans — it never panics, never loops, and never needs to look backwards
past one token.

Three consequences follow, and they shape the whole design:

1. **The lexer is total on well-formed bytes.** It is called on every compile, on every REPL line,
   and on every file the M3↔M4 differential harness loads. A lexer that can panic is a crash
   vector that the rest of the system must defend against, which violates the stability contract in
   [`00-blueprint-review.md`](00-blueprint-review.md) §5.
2. **The lexer is context-free.** It never consults the token stream to decide what a byte means.
   The legacy lexer does do this in two places — the `after_dot` check at
   `src/runtime.rs:1043-1046` and the "last token implies continuation" check at
   `src/runtime.rs:696` — both of which become one bit and one word of state in the new lexer
   (**M6**, §10.4).
3. **The lexer is not on the hot path.** It runs once per compile; the VM runs forever. §12 sets
   the budget. [`06-parser.md`](06-parser.md) §13 sets the same for the parser.

**In scope:** bytes → tokens, comment elision, newline classification, numeric and string literal
cooking, error recovery, source positions.

**Out of scope:** the `#{}` vs `{` block/dict decision (a *parser* decision, see
[`06-parser.md`](06-parser.md) §5), the `name expr` command-call decision (parser, §6 there),
module resolution (parser, §7 there), and any type inference.

---

## 2. Input and output

### 2.1 Signature

```rust
pub fn lex<'a>(src: &'a str, file: FileId) -> Lexed<'a>;
```

`Lexed` is always returned; it carries its own `errors: Vec<LexError>`, so the caller decides
whether an empty error list is fatal. This deliberately differs from the legacy
`fn lex(source: &str) -> Result<Vec<Token>, String>` at `src/runtime.rs:672`, which aborts on the
first problem and can never report a second.

**RULING A1 — a leading BOM is skipped, silently, once.** If `src` begins with `U+FEFF`, the
`Lexer::new` constructor advances past its three bytes and records the offset so every `Span` in
the result is still an offset into the *original* `src`. Nothing else changes: a `U+FEFF` anywhere
else is `Class::Illegal`, and a BOM after the first byte is almost always a mistake worth seeing.

The alternative is a `LexError::ByteOrderMark`, which is a diagnostic every Windows-authored file
would earn, describing a byte-sequence artefact that has no effect on the meaning of the program.
The offset is preserved rather than `trim`-ming the string because `Span` indexes into the
`&str` the caller handed in, and a `trim`-med copy would invalidate every span above it. Test
T-A15.

### 2.2 Output shape

```rust
pub struct Lexed<'a> {
    pub file:    FileId,
    pub toks:    Vec<Token>,       // the stream; always ends with TokenKind::Eof
    pub idents:  Interner<'a>,     // side table: IDENT payloads
    pub strs:    Interner<'a>,     // side table: cooked STR payloads
    pub interps: Vec<Interp<'a>>,  // side table: INTERP payloads
    pub nums:    Vec<NumLit>,      // side table: INT / FLOAT payloads
    pub lines:   LineIndex,        // side table: byte offset -> (line, col)
    pub errors:  Vec<LexError>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(C)]
pub struct Token {
    pub kind: TokenKind,   // #[repr(u8)]: 8 structural/literal + 50 keyword + 55 punctuation
                           //   = 113 variants, discriminants 0–7, 8–57, 58–110, 141–143
    pub aux:  u32,         // Ident/Str -> interner id, Interp -> interps id, Int/Float -> nums id
    pub lo:   u32,         // byte offset of the first byte, inclusive
    pub hi:   u32,         // byte offset of the last byte + 1, exclusive
}
```

`Token` is 12 bytes, `Copy`, `PartialEq`, and contains no pointer. A 64-byte cache line holds
5.33 tokens. On the 84,490-byte conformance corpus the stream is roughly 22,000 tokens ≈ 264 KB,
and the parser scans it once with a one-token lookahead, so the cost is pure streaming.

Payloads live in side tables for three reasons, all measurable:

- **Tokens stay 16 bytes** (`kind` 4 + `aux` 4 + `lo` 4 + `hi` 4, asserted at compile time in
  `lex/token.rs`). A NaN-boxed or `f64`-inline payload pushes the token to 24 bytes and
  halves the tokens per cache line. The parser's only random access into the stream is a one-token
  lookahead, so a narrower token is strictly better.
- **`f64` cannot be inlined without `unsafe`.** `TokenKind::Float` needs 8 payload bytes; putting
  them in the struct needs either a union or an `f64`↔`i64` pun, both `unsafe`, and `unsafe` is
  confined to `zen-vm::value` and `zen-vm::gc` by
  [`00-blueprint-review.md`](00-blueprint-review.md) §5.4 and
  [`decisions/ADR-0001-tag-budget.md`](decisions/ADR-0001-tag-budget.md). The side table removes
  the question.
- **The indirect load is nearly free.** `nums` is written in scan order and read in scan order by
  the parser, so it is already in L1 when needed. Numbers are ~12% of tokens in the corpus.

**Integer payloads.** `NumLit::Int(i48)` uses the i48 range fixed by
[`decisions/ADR-0001-tag-budget.md`](decisions/ADR-0001-tag-budget.md):
`i48::MIN = -140_737_488_355_328`, `i48::MAX = 140_737_488_355_327`.

**Not NaN-boxed.** The VM's `Value` is not reused here, for the same reason ADR-0001 gives for the
AST: `zen-syntax` must not depend on `zen-vm`, so that `zenn check` links only the front end, and
a NaN-box decode on every token read would buy nothing for a structure read exactly once.

### 2.3 Accessors

All payload reads go through methods on `Lexed`, never through direct field access, so the two
interners can be merged later without touching the parser.

```rust
impl<'a> Lexed<'a> {
    #[inline] pub fn ident(&self, t: &Token) -> &'a str     { self.idents.get(t.aux as usize) }
    #[inline] pub fn str_bytes(&self, t: &Token) -> &'a [u8] { self.strs.get(t.aux as usize) }
    #[inline] pub fn num(&self, t: &Token) -> NumLit         { self.nums[t.aux as usize] }
    #[inline] pub fn span(&self, t: &Token) -> Span          { Span::new(self.file, t.lo, t.hi) }
}
```

### 2.4 The span side table

`Span { file_id: FileId, lo: u32, hi: u32 }` lives on every AST node
([`06-parser.md`](06-parser.md) §3); the lexer only produces `(file, lo, hi)`. Turning an offset
into a line and column is a separate, cold structure built in one pass:

```rust
pub struct LineIndex { starts: Vec<u32> }   // byte offset of each line's first byte; starts[0] == 0

impl LineIndex {
    pub fn build(src: &str) -> Self;            // one pass over memchr::memchr_iter(b'\n', …)
    pub fn line_of(&self, off: u32) -> u32;     // 1-based; binary search over `starts`
    pub fn col_of(&self, off: u32) -> u32;      // 1-based, in CHARS not bytes
    pub fn line_col(&self, off: u32) -> (u32, u32);
}
```

Columns count characters, not bytes, so a caret under `█` (3 UTF-8 bytes) lands correctly;
`col_of` decodes backwards from the line start and is therefore only called on the diagnostic path.
The legacy lexer instead threads a `col` counter incremented on every byte
(`src/runtime.rs:686`, `src/runtime.rs:705-707`), which is pure overhead — see **M8** in §10.4.

### 2.5 Why a single non-recursive pass with no backtracking

The lexer is one `while i < n` loop with a dispatch table (§10.2). It:

- **never re-reads a byte it has already consumed**, and never re-enters itself. No recursion means
  no stack depth, so a 200 MB generated file cannot overflow the stack.
- **never inspects the emitted token stream** to decide what the current byte means.
- **never speculates.** No point in the grammar admits a wrong guess, because every ambiguous byte
  sequence is resolved by a single peek at `src[i + 1]`.

The cost of that property is that the lexer cannot know whether a `{` opens a block or a legacy
dict. It does not need to: see [`06-parser.md`](06-parser.md) §5, where the decision is made from
the *parser's* position in the grammar, which is information the lexer provably does not have. This
is the most important structural decision across the two documents — the `#`/block ambiguity is
split between the two passes along the only axis that can resolve it, and that is *why* the lexer
stays a single context-free pass.

---

## 3. `TokenKind`: the complete table

Numeric discriminants are **frozen once assigned** in Milestone 1. The parser's operator dispatch,
the M4 compiler's opcode-selection switch, and the M5 inline-cache key all index by `u8` and are
compiled against these values. Adding a variant is non-breaking; renumbering is breaking and
requires bumping a `static_assert`.

`Display for TokenKind` produces the canonical source text and is used verbatim in
`expected X, found Y` parser diagnostics.

### 3.1 Structural and literal tokens

| Kind | repr | Display | Class | Payload | Legacy `Kind` |
|---|---|---|---|---|---|
| `Eof` | 0 | `<end of file>` | structural | — | `Eof` `src/runtime.rs:473` |
| `Newline` | 1 | `\n` | structural | — | `Newline` `:432` |
| `Semi` | 2 | `;` | structural | — | `Semi` `:431` |
| `Ident` | 3 | `foo` | name | `Interner` id | `Ident(String)` `:371` |
| `Int` | 4 | `42` | literal | `nums[i] = Int(i48)` | `Number(f64)` `:372` |
| `Float` | 5 | `1.5` | literal | `nums[i] = Float(f64)` | `Number(f64)` `:372` |
| `Str` | 6 | `"ab"` | literal | `strs[i]`, cooked bytes | `String(String)` `:373` |
| `Interp` | 7 | `"a${b}c"` | literal | `interps[i]` | `Interp(Vec<InterpPart>)` `:374` |

`Eof`, `Newline`, `Semi` are the first three discriminants so that `kind <= TokenKind::Semi` is one
compare for "is this a separator", which the parser's `skip_separators` does on every statement.

### 3.2 Keywords

Every keyword is reserved: it can never be an identifier, a parameter, a member, or a dict key —
**except** immediately after `.` or `?.` (rule **K1**, §3.4).

| Kind | repr | Source spellings | Notes | Legacy |
|---|---|---|---|---|
| `KwLet` | 8 | `let` | | `:1051` |
| `KwConst` | 9 | `const` | reassignment is a runtime error (`grammar.md:256-257`) | `:1052` |
| `KwVar` | 10 | `var`, `global` | `global` folds to `KwVar`, as in the legacy lexer | `:1053` |
| `KwPub` | 11 | `pub` | **NEW.** visibility modifier | — |
| `KwShared` | 12 | `shared` | **NEW.** `Arc<Mutex<T>>`, [`decisions/ADR-0003-concurrency-semantics.md`](decisions/ADR-0003-concurrency-semantics.md); M10 | — |
| `KwPrint` | 13 | `print` | statement or call — [`06-parser.md`](06-parser.md) §6.4 | `:1054` |
| `KwFn` | 14 | `fn`, `func`, `function`, `procedure`, `proc`, `def` | **RULING L1:** all six fold to one kind (see below) | `:1063`, `:1064` |
| `KwLambda` | 15 | `lambda` | | `:1088` |
| `KwReturn` | 16 | `return` | | `:1065` |
| `KwIf` | 17 | `if` | statement *and* expression primary | `:1055` |
| `KwElif` | 18 | `elif` | | `:1056` |
| `KwElse` | 19 | `else` | | `:1057` |
| `KwWhile` | 20 | `while` | | `:1058` |
| `KwFor` | 21 | `for` | | `:1059` |
| `KwIn` | 22 | `in` | `for … in` and the `in` comparison share one kind | `:1060` |
| `KwBreak` | 23 | `break` | | `:1061` |
| `KwContinue` | 24 | `continue` | | `:1062` |
| `KwClass` | 25 | `class` | statement only; the name is required (`grammar.md:288-289`) | `:1066` |
| `KwNew` | 26 | `new` | | `:1067` |
| `KwExtends` | 27 | `extends`, `inherit` | alias (`grammar.md:52`) | `:1068`, `:1069` |
| `KwSuper` | 28 | `super` | | `:1080` |
| `KwThis` | 29 | `this` | runtime alias for `self`, methods only (`grammar.md:56`) | — |
| `KwImport` | 30 | `import` | | `:1070` |
| `KwFrom` | 31 | `from` | | `:1071` |
| `KwAs` | 32 | `as` | import alias, catch binding; `let as` cast in M8 | `:1074` |
| `KwNative` | 33 | `native` | | `:1075` |
| `KwInclude` | 34 | `include`, `load` | `load` is an alias (`grammar.md:133`) | `:1072`, `:1073` |
| `KwTry` | 35 | `try` | | `:1076` |
| `KwCatch` | 36 | `catch`, `except` | `except` is an alias (`grammar.md:52`) | `:1077` |
| `KwFinally` | 37 | `finally` | | `:1078` |
| `KwThrow` | 38 | `throw`, `raise` | `throw` canonical (`grammar.md:54-55`) | `:1079` |
| `KwTypeof` | 39 | `typeof` | | `:1081` |
| `KwIs` | 40 | `is` | **plain equality**; no `is not` (`grammar.md:251-252`, review §6) | `:1082` |
| `KwSwitch` | 41 | `switch` | | `:1083` |
| `KwCase` | 42 | `case` | | `:1084` |
| `KwDefault` | 43 | `default` | | `:1085` |
| `KwMatch` | 44 | `match` | | `:1086` |
| `KwWhen` | 45 | `when` | subject-less `match` | `:1087` |
| `KwAnd` | 46 | `and`, `&&` | `&&` lexes as `KwAnd`, not a separate kind | `:1090`, `:1248` |
| `KwOr` | 47 | `or`, `||` | | `:1091`, `:1253` |
| `KwNot` | 48 | `not` | `!` is a separate kind (`Bang`) so diagnostics can say which was written | `:1092` |
| `KwTrue` | 49 | `true` | | `:1093` |
| `KwFalse` | 50 | `false` | | `:1094` |
| `KwNil` | 51 | `nil`, `null` | **both spellings, one value; `nil` canonical** (review §6) | `:1095` |
| `KwWith` | 52 | `with` | | `:1089` |
| `KwYield` | 53 | `yield` | M10 | — |
| `KwSpawn` | 54 | `spawn` | M10 | — |
| `KwSend` | 55 | `send` | M10 | — |
| `KwRecv` | 56 | `recv` | M10 | — |
| `KwSelect` | 57 | `select` | M10 | — |

**RULING L1 — keyword folding.** All six spellings of "define a function" fold to `KwFn`. The
legacy lexer emits a distinct `Kind::Def` (`src/runtime.rs:1064`) that the parser then handles
*identically* to `Kind::Function` (`src/runtime.rs:1895`), so folding loses nothing and removes a
token kind the parser otherwise has to carry. The same reasoning folds `&&` into `KwAnd`
(`src/runtime.rs:1248-1251`) and `||` into `KwOr` (`src/runtime.rs:1253-1256`), which the legacy
lexer already does.

**Not keywords** (ordinary identifiers, per `grammar.md:46-48` and preserved): `self`, `error`,
`main`, `args`, plus every builtin name. Builtins are identifiers the *resolver* recognises, not
reserved words, so `let print = 1` is a compile-time `NameError`, not a syntax error.

### 3.3 Punctuation

Discriminants 58–110 and 141–143, with **two holes on purpose**: **99** (a legacy token that was
retired, kept free so a future re-introduction is not a silent collision) and **111–140**, a
thirty-slot gap reserved so the three new sigils cannot collide with anything added later. Neither
hole is ever assigned, and `Display` has a test (T-A18) that a `#[repr(u8)]` discriminant is
either in the table or explicitly listed here.

| Kind | repr | Display | Legacy |  | Kind | repr | Display | Legacy |
|---|---|---|---|---|---|---|---|---|
| `LBrace` | 58 | `{` | `:424` | | `Assign` | 83 | `=` | `:438` |
| `RBrace` | 59 | `}` | `:425` | | `PlusEq` | 84 | `+=` | `:439` |
| `LBracket` | 60 | `[` | `:426` | | `MinusEq` | 85 | `-=` | `:440` |
| `RBracket` | 61 | `]` | `:427` | | `StarEq` | 86 | `*=` | `:441` |
| `LParen` | 62 | `(` | `:428` | | `SlashEq` | 87 | `/=` | `:442` |
| `RParen` | 63 | `)` | `:429` | | `PercentEq` | 88 | `%=` | `:443` |
| `Comma` | 64 | `,` | `:430` | | `AmpEq` | 89 | `&=` | `:444` |
| `Colon` | 65 | `:` | `:465` | | `PipeEq` | 90 | `\|=` | `:445` |
| `Dot` | 66 | `.` | `:464` | | `CaretEq` | 91 | `^=` | `:446` |
| `DotDot` | 67 | `..` | `:422` | | `ShlEq` | 92 | `<<=` | `:447` |
| `DotDotEq` | 68 | `..=` | **reserved, not adopted** (RULING L4 below) | | `ShrEq` | 93 | `>>=` | `:448` |
| `SafeDot` | 69 | `?.` | `:453` | | `NullishEq` | 94 | `??=` | `:454` |
| `Ellipsis` | 70 | `...` | `:421` | | `Arrow` | 95 | `->` | `:449` |
| `ThinArrow` | 71 | `=>` | `:418` | | `Nullish` | 96 | `??` | `:450` |
| `Question` | 72 | `?` | `:463` | | `Eq` | 97 | `==` | `:466` |
| `Bang` | 73 | `!` | `:472` | | `Ne` | 98 | `!=` | `:467` |
| `StrictEq` | 74 | `===` | `:451` | | `StrictNe` | 75 | `!==` | `:452` |
| `Plus` | 76 | `+` | `:433` | | `Minus` | 77 | `-` | `:434` |
| `Star` | 78 | `*` | `:435` | | `Slash` | 79 | `/` | `:436` |
| `Percent` | 80 | `%` | `:437` | | `Amp` | 81 | `&` | `:457` |
| `Pipe` | 82 | `\|` | `:458` | | `Caret` | 100 | `^` | `:459` |
| `Tilde` | 101 | `~` | `:460` | | `Lt` | 102 | `<` | `:468` |
| `Le` | 103 | `<=` | `:469` | | `Gt` | 104 | `>` | `:470` |
| `Ge` | 105 | `>=` | `:471` | | `Pow` | 106 | `**` | `:423` |
| `Shl` | 107 | `<<` | `:461` | | `Shr` | 108 | `>>` | `:462` |
| `Inc` | 109 | `++` | `:455` | | `Dec` | 110 | `--` | `:456` |
| `DictStart` | 141 | `#{` | **NEW** (§4) | | `TupleStart` | 142 | `#(` | **NEW** (§4) |
| `SetStart` | 143 | `%{` | **NEW** (§4) | | | | | |

**RULING L2 — operator folding.** `&&`, `||`, `??`, `..`, `**`, `->`, `=>`, `?.`, `===`, `!==`
fold into the word or single-token form the parser reasons about. The parser never sees a second
spelling for one operator, so the M4 compiler has one switch arm per operator and the differential
harness cannot diverge on a spelling.

**RULING L3 — the sigil block is last.** `DictStart`, `TupleStart`, `SetStart` take 141–143, the
three highest punctuation discriminants. Every legacy discriminant stays stable, so the frozen
table in this document is a superset of the legacy one.

**RULING L4 — `..=` is reserved, not adopted.** It is lexed (so `1..=2` produces `Int(1)`,
`DotDotEq`, `Int(2)` rather than a confusing error) and rejected by the parser with
`ParseError::ReservedToken("..=")` and the message "write `1 -> 2` for an inclusive range". Two
spellings for one concept is a second way to write the same thing, which is a source of bugs, and
`..=` invites readers to treat `..` as an operator where it is currently a range. Reconsider only
if slice literals (`a[1..2]`) ever need a half-open spelling. Tracked in
[`06-parser.md`](06-parser.md) §4.

### 3.4 Rule K1: keywords are not member names

A word immediately preceded by `.` or `?.` is always `Ident`, even if it is a keyword. Preserved
verbatim from `src/runtime.rs:1043-1046` and required for `ffi.load`, `dict.null`, `obj.default`,
`mod.case`.

The legacy implementation reads `out.last()` to decide. The new lexer maintains a one-bit
`after_dot` flag updated on every emitted token (**M6**), so the decision is one store instead of a
vector back-reference. The flag is set by `Dot` and `SafeDot` and cleared by every other token
*including `Newline`*, so

```zen
a.
  default
```

lexes `a` `.` `Ident("default")`, not `KwDefault`.

### 3.5 `Display`

`impl fmt::Display for TokenKind` emits the **canonical** spelling from the tables above, so a
diagnostic always teaches the preferred form: `and` not `&&`, `function` not `func`, `nil` not
`null`, `!` not `not`. This replaces the legacy `fn kind_text(k: &Kind) -> String` at
`src/runtime.rs:1527-1560`.

---

## 4. The `#` sigil: disambiguation

This is the only genuinely novel lexical problem in the language, because
[`00-blueprint-review.md`](00-blueprint-review.md) §6 makes `#` do double duty:

> Comments | `#` only | `//`, `#`, `/* */` | **Keep all three.** `#` is used by `#{`/`#(`; `# ` (hash-space) is still a comment. Lexer resolves by peeking the next byte.

### 4.1 The rules, formally

Let `b = src[i]` and `b1 = src[i + 1]` (`None` at end of input).

```text
R1   b == '#':
       b1 == Some(b'{')  ->  DictStart        (consume 2)
       b1 == Some(b'(')  ->  TupleStart       (consume 2)
       b1 == Some(_)     ->  LineComment      (consume through '\n' or EOF; emit NO token)
       b1 == None        ->  LineComment      (consume to EOF; emit NO token)

R2   b == '%':
       b1 == Some(b'{')  ->  SetStart         (consume 2)
       otherwise          ->  Percent         (modulo)

R3   b == '/':
       b1 == Some(b'/')  ->  LineComment      (consume through '\n' or EOF; emit NO token)
       b1 == Some(b'*')  ->  BlockComment     (consume through "*/"; emit NO token;
                                               unterminated = LexError)
       otherwise          ->  Slash

R4   No other byte starts a comment, and no other byte produces a sigil opener.
```

**The no-space rule, stated once and without exception:**

> `DictStart` requires `#` and `{` to be **adjacent bytes**. `# {`, `#\t{`, and `# \n{` are
> **comments**, not dict literals. There is no whitespace-skipping lookahead anywhere in the `#`
> handling, and there never will be.

**RULING H1 — no context sensitivity.** `#{` is *always* `DictStart`, `%{` is *always* `SetStart`,
and `#(` is *always* `TupleStart`, regardless of what precedes them. The lexer does not know
whether it sits at the start of a statement (where `{` would be a block) or mid-expression (where
`{` would be a legacy dict), and it does not care. This is what keeps §2.5 true: the pass is
context-free and needs no backtracking. The block/dict question is answered by the parser, which
does have that context — [`06-parser.md`](06-parser.md) §5.

**Consequence, stated so nobody is surprised:** `a % {}` does **not** mean "a modulo an empty
block". It lexes as `a` `SetStart` `RBrace` and is a parse error ("set literal in operand
position"). Modulo-by-a-set is not a meaningful operation in any Zen program, so no program breaks;
if a `%` must ever be immediately followed by a brace-delimited expression it is written
`a % (#{…})` or `a % (…)`.

### 4.2 Truth table

`∅` in the "Tokens" column means the lexer emits nothing at all. `NL` is `TokenKind::Newline`.

| # | Source bytes | Tokens | Why |
|---|---|---|---|
| 1 | `#{` | `DictStart` | R1 |
| 2 | `#{a:1}` | `DictStart` `Ident(a)` `Colon` `Int(1)` `RBrace` | R1 |
| 3 | `#{}` | `DictStart` `RBrace` | empty dict |
| 4 | `#(1,2)` | `TupleStart` `Int(1)` `Comma` `Int(2)` `RParen` | R1 |
| 5 | `#()` | `TupleStart` `RParen` | 0-tuple |
| 6 | `%{1,2}` | `SetStart` `Int(1)` `Comma` `Int(2)` `RBrace` | R2 |
| 7 | `%{}` | `SetStart` `RBrace` | empty set |
| 8 | `# hello` | `∅` | `#` + space |
| 9 | `#hello` | `∅` | `#` + identifier byte |
| 10 | `#!shebang` | `∅` | `#!` is a comment, so a shebang line is free |
| 11 | `##` | `∅` | `#` + `#` |
| 12 | `#\n` at the start of a file | `∅` | A4: no significant token has been emitted, so the newline is suppressed |
| 13 | `#` at EOF | `∅` `Eof` | no trailing newline, no error |
| 14 | `# {` | `∅` | **RULING H1: a comment, not a dict** |
| 15 | `#\t{` | `∅` | same |
| 16 | `# {a:1}` | `∅` | the whole line is a comment; the newline is governed by A3 or A4 |
| 17 | `a # {` | `Ident(a)` `∅` | `a`, then a comment to EOL |
| 18 | `x = 1 #\n#{a:1}` | `Ident(x)` `Assign` `Int(1)` `NL` `DictStart` … | the comment ends at `\n`; the *next line* is a dict |
| 19 | `#{#{a:1}}` | `DictStart` `DictStart` `Ident(a)` `Colon` `Int(1)` `RBrace` `RBrace` | nesting is the parser's job, not the lexer's |
| 20 | `a % {b}` | `Ident(a)` `Percent` `LBrace` `Ident(b)` `RBrace` | **space** between `%` and `{` ⇒ modulo a block |
| 21 | `a %{b}` | `Ident(a)` `SetStart` `Ident(b)` `RBrace` | R2, RULING H1 — a parse error, by design |
| 22 | `%%` | `Percent` `Percent` | |
| 23 | `#{` inside `"…"` | `∅` (part of `Str`) | the string scanner does not apply R1 |
| 24 | `#{` inside `"\"\"\"…\"\"\""` | `∅` (part of `Str`) | |
| 25 | `#{` inside `// …` | `∅` | the line-comment scanner does not apply R1 |
| 26 | `#{` inside `/* … */` | `∅` | the block-comment scanner does not apply R1 |
| 27 | `${#{a:1}}` | `∅` (part of `Interp`) | the `${}` body is a raw byte slice, re-lexed by the parser (§6.4) |
| 28 | `//x` | `∅` `NL` | R3 |
| 29 | `/*x*/` | `∅` | R3 |
| 30 | `/* a\nb */` | `∅` `NL` | the embedded newline is consumed by the comment, not emitted |
| 31 | `/* /* */ */` | `∅` `NL` then a parse error on the stray `*/` | **RULING H2: block comments do not nest** (C-style; first `*/` closes), preserved from `src/runtime.rs:717-744` |
| 32 | `/* x` at EOF | `Eof` + `LexError::UnterminatedBlockComment` | preserved from `src/runtime.rs:737-742` |
| 33 | `a / b` | `Ident(a)` `Slash` `Ident(b)` | R3 fallthrough |
| 34 | `a /*x*/ b` | `Ident(a)` `∅` `Ident(b)` | the comment produces no token, so `after_dot` and `prev` are unaffected |
| 35 | `#{a:1} + 1` | `DictStart` … `RBrace` `Plus` `Int(1)` | the `}` closes the dict; no delimiter underflow |

Row 31 needs a note. Non-nesting is preserved deliberately: making block comments nest is a
*source-breaking* change for any file already writing `/* /* */ */` expecting the outer comment to
close, and nothing in the corpus needs nesting. If nesting is ever wanted it is an explicit
`/*~ … ~*/` form, not a change of meaning for `/*`.

Row 34 is load-bearing for §7. Because comments emit no tokens, the "previous significant token"
used by the newline-classification rule is unaffected by a comment:

```zen
var a = 1 +
    /* a comment that happens to be here */
    2
```

lexes as a single statement, because the previous significant token is still `Plus`.

### 4.3 What `#{` means for the parser

The lexer emits `DictStart` / `RBrace` as an ordinary matched pair. The parser turns a
`DictStart` into `Expr::DictLiteral` with no lookahead and no flag —
[`06-parser.md`](06-parser.md) §2.4. A bare `LBrace` is where the parser does its one piece of
speculation, and that is §5 of that document, not here.

---

## 5. Numbers

### 5.1 Grammar

```ebnf
number        = int_lit | float_lit ;

int_lit       = decimal | hex | binary | octal ;
decimal       = digit , { digit | "_" } ;
hex           = "0" , ( "x" | "X" ) , hex_d , { hex_d | "_" } ;
binary        = "0" , ( "b" | "B" ) , bit , { bit | "_" } ;
octal         = "0" , ( "o" | "O" ) , oct_d , { oct_d | "_" } ;

float_lit     = digits , [ "." , [ digits ] ] , [ exponent ]
             | "." , digits , [ exponent ]
             | digits , exponent ;
exponent      = ( "e" | "E" ) , [ "+" | "-" ] , digit , { digit | "_" } ;

digits        = digit , { digit | "_" } ;
digit         = "0"…"9" ;
hex_d         = digit | "a"…"f" | "A"…"F" ;
bit           = "0" | "1" ;
oct_d         = "0"…"7" ;
```

Entry condition: the number scanner is entered when `src[i]` is an ASCII digit, **or** when
`src[i] == '.'` and `src[i + 1]` is an ASCII digit. The second clause preserves the legacy
behaviour at `src/runtime.rs:963-968`, so `.5` is `0.5` and `..5` is `DotDot` `Int(5)`.

### 5.2 Algorithm

`scan_number` is entered with `start = i` and runs to a fixed point. It is straight-line code, not
a state machine, because the forms are few and predictable.

```rust
/// Precondition: `src[i]` is an ASCII digit, or `src[i] == b'.'` with `src[i+1]` a digit.
/// A bare `.` never reaches this function — `class_of(b'.')` is `Dot`, and `lex_operator`
/// handles it, so there is no `NoDigits` case and no dead branch here.
fn scan_number(src: &[u8], mut i: usize) -> Result<(usize, RawNum), NumErr> {
    let start = i;

    // ── S1. radix prefix ──────────────────────────────────────────────────
    if src[i] == b'0' {
        if let Some(&p) = src.get(i + 1) {
            let (radix, set) = match p {
                b'x' | b'X' => (16u32, DIGITS16),
                b'b' | b'B' => ( 2u32, DIGITS2),
                b'o' | b'O' => ( 8u32, DIGITS8),
                _           => ( 0u32, DIGITS10),
            };
            if radix != 0 {
                i += 2;
                // N4: a separator directly after the prefix is a mistake, and it is checked
                // *before* the digit run so that `0x_FF` cannot slip through as a valid literal.
                if src.get(i) == Some(&b'_') { return Err(NumErr::BadSeparator); }
                let d0 = i;
                i = eat_digits(src, i, set);          // '_' allowed only BETWEEN digits
                if i == d0 { return Err(NumErr::NoDigitsAfterPrefix); }
                // N3 again, at the other end of the run: a trailing or doubled separator.
                if src.get(i) == Some(&b'_') || src.get(i - 1) == Some(&b'_') {
                    return Err(NumErr::BadSeparator);
                }
                // N6: the digit set of the base is enforced, so `0b2` fails here.
                if let Some(&bad) = src.get(i) {
                    if bad == b'_' || (bad.is_ascii_alphanumeric() && set[bad as usize] == 0
                                       && !is_radix_digit(bad, radix)) {
                        return Err(NumErr::InvalidDigit);
                    }
                }
                return Ok((i, RawNum::Radix(radix, &src[d0..i])));
            }
        }
    }

    // ── S2. integer part (may be empty only when the literal starts with '.') ──
    let int_start = i;
    i = eat_digits(src, i, DIGITS10);

    // ── S3. fraction ─────────────────────────────────────────────────────
    // The dot is taken whenever it is NOT the start of `..`, and the digits after it may be
    // empty. Two consequences, both required by tests:
    //   `5.`    → `FLT(5.0)`  (T-N6)  — an empty fractional part is still a float
    //   `1..3`  → `INT(1) DotDot INT(3)` (T-N25) — `..` is a range, never a fraction
    // `1.2.real` is unaffected because S3 stops at the second dot (it is followed by `r`,
    // not by a `.`), and S7 below does not fire on a `.`.
    let mut has_dot = false;
    if src.get(i) == Some(&b'.') && src.get(i + 1) != Some(&b'.') {
        has_dot = true;
        i += 1;
        i = eat_digits(src, i, DIGITS10);
    }

    // ── S4. exponent ─────────────────────────────────────────────────────
    let mut has_exp = false;
    if matches!(src.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(src.get(j), Some(b'+' | b'-')) { j += 1; }
        let d0 = j;
        j = eat_digits(src, j, DIGITS10);
        if j == d0 { return Err(NumErr::EmptyExponent); }   // RULING N1, see 5.3
        i = j;
        has_exp = true;
    }

    // ── S5. reject a second fraction — preserves src/runtime.rs:1007-1021 ──
    if src.get(i) == Some(&b'.') && src.get(i + 1).is_some_and(|b| b.is_ascii_digit()) {
        return Err(NumErr::SecondDot);
    }

    // ── S6. a dangling or doubled '_' is always a mistake ────────────────
    if src.get(i) == Some(&b'_') || (i > start && src.get(i - 1) == Some(&b'_')) {
        return Err(NumErr::BadSeparator);
    }

    // ── S7. a digit run glued to an identifier is a mistake ──────────────
    // This does not fire on a `.`, which is why `1.2.real` is `FLT(1.2) Dot ID(real)` (T-N24)
    // and not an error. It does fire on `5.foo` (T-N33), because S3 above already consumed the
    // dot as the fractional point — the same trade Python makes, and the price of T-N6.
    if src.get(i).is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_') {
        return Err(NumErr::GluedIdentifier);
    }

    if has_dot || has_exp { Ok((i, RawNum::Float(&src[start..i]))) }
    else                 { Ok((i, RawNum::Decimal(&src[start..i]))) }
}

/// Consume `[digit | '_']*` where `set` is a 256-entry digit bitmap. Underscores are dropped
/// here, not later. An `_` must have a digit on both sides; a violation is left in place and
/// reported by rule S6 above.
#[inline]
fn eat_digits(src: &[u8], mut i: usize, set: &[u8; 256]) -> usize {
    while let Some(&b) = src.get(i) {
        if set[b as usize] != 0 { i += 1 }
        else if b == b'_'      { i += 1 }
        else                   { break }
    }
    i
}
```

`DIGITS10`, `DIGITS16`, `DIGITS8`, `DIGITS2` are `static [u8; 256]` bitmaps generated by a
`const fn` at module scope, so `eat_digits` is one load and one compare per byte.

### 5.3 Validity rules

| Id | Rule | Example | Result |
|---|---|---|---|
| **N1** | A digit run immediately followed by `e`/`E` followed by nothing digit-like is an error. The legacy lexer silently backs the exponent off (`src/runtime.rs:1002-1005`), so `1e` became `Int(1)` + `Ident("e")` and the failure surfaced later as a `NameError`. | `1e` | `LexError(EmptyExponent)` |
| **N2** | A literal may not be immediately followed, with no intervening whitespace, by an identifier-start byte. | `1abc`, `12if` | `LexError(GluedIdentifier)` |
| **N3** | `_` may appear only *between* digits. No leading, no trailing, no doubling. | `1_000` ✓ · `1_` ✗ · `1__0` ✗ · `_1` → `Ident` | `LexError(BadSeparator)` |
| **N4** | No separator directly after a radix prefix. | `0x_FF` ✗ | `LexError(BadSeparator)` |
| **N5** | A radix literal needs at least one digit. | `0x` ✗ | `LexError(NoDigitsAfterPrefix)` — preserved from `src/runtime.rs:948-953` |
| **N6** | The digit set of the base is enforced. | `0b2` ✗ | `LexError(InvalidDigit)` |
| **N7** | **Multi-dot literals are rejected.** A `.` followed by an ASCII digit after a complete number is an error, never a second fraction. | `1.2.3` ✗ | `LexError(SecondDot)`; message preserved verbatim from `src/runtime.rs:1017-1020` |
| **N8** | Member access on a float is *not* affected by N7, because the byte after the `.` is not a digit. | `1.2.real` ✓ · `1.2.3` ✗ | — |
| **N9** | `..` is a range, not a second dot, so it is not affected by N7. | `1..3` ✓ | — |
| **N10** | There is no leading-zero octal. | `010` → `Int(10)` | — |
| **N11** | No sign inside the literal. `-1` is `Minus` `Int(1)`; unary `+` arrives in M8. | | — |

**RULING N1 — deliberate strictness.** N1 and N2 make the lexer stricter than
`src/runtime.rs:963-1030`. The risk is a corpus literal like `12if`. Mitigations: the conformance
gate re-lexes every `.z` file in the legacy tree before M1 closes, and the diagnostic names the
fix. The reward is that `1e` and `1abc` fail on the line the user wrote rather than as a
`NameError` three lines later — the same honesty argument as
[`decisions/ADR-0004-honest-types.md`](decisions/ADR-0004-honest-types.md).

### 5.4 Value assignment and the overflow rule

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NumLit { Int(i48), Float(f64) }
```

1. A literal with a fraction or an exponent is **always** `Float`.
2. A decimal or radix literal is accumulated into an `i128` by shift-accumulate — no allocation, no
   `str::parse`.
3. If the value fits in `i48::MIN..=i48::MAX`, the token is `Int(i48)`.
4. **Otherwise the token is promoted to `Float`.** `Int` overflow never wraps, never errors, and
   never produces a wrong answer. This follows directly from the i48 decision in
   [`decisions/ADR-0001-tag-budget.md`](decisions/ADR-0001-tag-budget.md) ("Overflow promotes to
   `float` rather than wrapping") together with
   [`00-blueprint-review.md`](00-blueprint-review.md) §6 ("i48 integer plus f64 float, with
   Lua-style promotion, so `10/4 == 2.5` still holds").
5. Radix literals use a per-base `match digit { … }` accumulator, not
   `u64::from_str_radix` on a slice. The legacy call at `src/runtime.rs:954` is slower *and* has no
   `_` support, so replacing it removes the need to strip separators at all.

Boundary table (`i48::MAX = 140_737_488_355_327`):

| Source | `NumLit` | Notes |
|---|---|---|
| `140737488355326` | `Int(140737488355326)` | `MAX - 1` |
| `140737488355327` | `Int(140737488355327)` | `MAX`, exact |
| `140737488355328` | `Float(1.40737488355328e17)` | `MAX + 1`, promoted |
| `281474976710655` | `Float(2.81474976710655e17)` | `2 * MAX` |
| `9223372036854775807` | `Float(9.223372036854776e18)` | `i64::MAX`; **not** a wrap to `-1` |
| `0x7FFFFFFFFFFF` | `Int(140737488355327)` | radix literals honour the same range |
| `0x800000000000` | `Float(2.251799813685248e16)` | promoted |
| `1e10` | `Float(1.0e10)` | has an exponent ⇒ `Float` even though integral |
| `1E-10` | `Float(1e-10)` | |
| `2.5E+3` | `Float(2500.0)` | |
| `1_000_000` | `Int(1000000)` | |
| `0b1010_1010` | `Int(170)` | |
| `0o755` | `Int(493)` | |
| `0XFF` | `Int(255)` | uppercase prefix and uppercase digits |
| `1/4` at runtime | `Float(0.25)` | `Int / Int` promotes (review §6) |

### 5.5 Float conversion

Decimal floats are parsed by a single-pass `eisel_lemire`-style decimal→binary converter over the
`_`-stripped byte range, falling back to `str::parse::<f64>()` for inputs longer than the fast
path's 19-significant-digit window. `str::parse::<f64>` is the correctness oracle and the slow
path; the fast path is a shift-accumulate into a `u64` mantissa followed by one
`f64::from_bits`. Both are exercised by a property test (§11, T-N30) over 100,000 random decimal
literals generated with `proptest`'s `arbitrary::any::<f64>()` reformatted with `{}` — the property
asserted is `fast(s) == slow(s)`, not a stored golden value, so the test survives changes to the
oracle.

---

## 6. Strings

### 6.1 The five forms

| Form | Interpolates | Multi-line | Terminator | Legacy |
|---|---|---|---|---|
| `"…"` | **yes** (`${…}`) | no | `"` | `:752`, `:757` |
| `'…'` | no | no | `'` | `:757` |
| `"""…"""` | **yes** | yes | `"""` | `:761-765` |
| `'''…'''` | **no** | yes | `'''` | `:757` — `is_interpolated = c == '"' \|\| c == '`'` implies `'` and `'''` do not interpolate |
| `` `…` `` | **yes** | no | `` ` `` | `:757` |

**RULING S1 — no triple backtick.** `` `x` `` interpolates but `` ```x``` `` is not a token; the
legacy scanner explicitly excludes the backtick from triple detection (`c != '`'` at
`src/runtime.rs:754`). Preserved.

**RULING S2 — triple detection is exact.** `"""` means bytes `0x22 0x22 0x22` with nothing between
them, matching `src/runtime.rs:753-756`. `"" ""` therefore lexes as `Str("")` followed by a second
`Str("")` with no operator between them — a parse error — and must **not** be "fixed" into
something that joins the quotes.

**RULING S3 — a raw newline may not appear in a single-line string.** `"abc` continued on the next
physical line is a `LexError(RawNewlineInString)`. The legacy scanner accepts it silently
(`src/runtime.rs:903-911` pushes `\n` into the text and resets `col`), which means a string that
quietly contains a newline is a bug class that surfaces later, in output formatting, rather than
at the source line. The supported spelling is a line continuation (§8). The conformance gate
re-scans the corpus for this specifically.

**RULING S3a — recovery stops at the end of the line, and never truncates the file.** After
reporting, the scanner resynchronises to the next physical line and lexing *continues* from there;
no token is produced for the malformed literal. This matters more than it looks: `None` from a
token producer means "input consumed, diagnostic recorded", and must be distinguished from
end-of-input, or the first bad string in a file silently discards every token after it. The
diagnostic's `span` therefore extends over the resynchronised region, so the T-P3 sum still
balances. `a = "oops\nb = 1\nc = 2` yields `ID Assign ID Assign INT NL ID Assign INT NL Eof`.

**RULING S4 — an unterminated string reports the opening line.** `LexError` carries both `span`
(the literal from opening quote to EOF) and `at` (just the opening quote); the caret goes on `at`,
and a `= note:` line gives the multi-line alternative.

### 6.2 Escapes

Processed in the five forms that can contain them. Preserved from `lex_escape` at
`src/runtime.rs:549-635`, plus one addition.

| Escape | Result | Legacy |
|---|---|---|
| `\n` | U+000A | `:561` |
| `\t` | U+0009 | `:562` |
| `\r` | U+000D | `:563` |
| `\0` | U+0000 | `:564` |
| `\a` | U+0007 | `:565` |
| `\b` | U+0008 | `:566` |
| `\e` | U+001B | `:567` |
| `\f` | U+000C | `:568` |
| `\v` | U+000B | `:569` |
| `\\` | `\` | `:570` |
| `\'` | `'` | `:571` |
| `\"` | `"` | `:572` |
| `` \` `` | `` ` `` | `:573` |
| `\$` | `$` | `:574` |
| `\xNN` | code point `NN`, exactly two hex digits | `:575-593` |
| `\u{H…}` | code point `H…`, 1–6 hex digits | `:594-632` |
| `\U{H…}` | code point `H…`, 1–8 hex digits | **NEW** |

`\xNN` decodes the two digits to a `u8` and pushes it as `(code as char).to_string()`, exactly as
`src/runtime.rs:592` does. That is a Latin-1 mapping, not UTF-8: `\xFF` is U+00FF and encodes as
two UTF-8 bytes. Preserved deliberately — `bench_binascii` and the `binascii` stdlib module depend
on it.

`\U{…}` is redundant with `\u{…}` in capability (six hex digits already reach `0x10FFFF`) and
exists for symmetry with `<stdint.h>`. It adds two checks the legacy `\u{…}` lacks: the value must
be `<= 0x10FFFF` and must not be in the surrogate range `D800..=DFFF`. `\u{…}` keeps the legacy
6-digit cap (`src/runtime.rs:620-622`).

**RULING S5 — unknown escapes warn, they do not fail.** `\q` decodes to `q`, as the legacy
fallthrough at `src/runtime.rs:633` does, and the lexer additionally records a `Warning`-severity
`LexError::UnknownEscape` on the escape. Rationale: the corpus may contain `"\d"` inside a regex
literal, and failing the build on something a codemod fixes is worse than accepting it. `zenn lint`
promotes the warning to an error. Strictly better, zero source breakage.

| Error kind | Trigger | Message stem |
|---|---|---|
| `UnfinishedEscape` | `\` at EOF | `` unfinished escape `` (`:559`) |
| `IncompleteHexEscape` | `\x1` | `` incomplete `\xNN` escape, expected two hex digits `` (`:582`) |
| `InvalidHexEscape` | `\xZZ` | `` invalid `\xNN` escape, expected two hex digits `` (`:586`) |
| `InvalidUnicodeEscape` | `\u[` | `` invalid `\u{...}` escape, expected `{` after `\u` `` (`:599`) |
| `UnterminatedUnicodeEscape` | `\u{41` at EOF | `` unterminated `\u{...}` escape `` (`:609`) |
| `EmptyUnicodeEscape` | `\u{}` | `` empty `\u{...}` escape `` (`:625`) |
| `UnicodeEscapeTooLong` | `\u{1234567}` | `` `\u{...}` escape has too many digits `` (`:621`) |
| `InvalidCodePoint` | `\u{110000}`, `\u{D800}` | `` invalid unicode code point `` (`:631`) |

### 6.3 Interpolation

`${ … }` in `"…"`, `` `…` ``, and `"""…"""`.

**RULING S6 — escapes are NOT processed inside `${…}`.** The body between `${` and its matching `}`
is captured as a **raw byte slice** and handed to the parser, which lexes it as ordinary Zen source
under ordinary rules. `"${x}\n"` interpolates `x`; it does not process escapes inside the braces.

Three reasons, in order of weight:

1. **The body is real source.** If escapes were processed inside the interpolation but not outside
   it, `"a\n${b\nc}d"` would feed the parser an expression containing a literal newline while the
   source the user sees contains a backslash. The two would disagree, and every span in the
   sub-tree would point at bytes the user did not write. One rule — *the `${}` body is source, the
   rest is text* — cannot disagree with itself.
2. **It removes a flag.** A "raw" boolean threaded through the string scanner, the escape handler,
   and the interpolation scanner is three places to get wrong. Not having it is worth more than the
   microseconds saved.
3. **Errors are better for free.** Re-lexing the body means a syntax error inside `${}` is reported
   as a syntax error *inside the interpolation*, with a span relative to the whole file, instead of
   as an escape error. The REPL can then highlight the sub-expression.

Cost: one extra lex of the body. Bodies are short, so the total is O(source bytes) and amortises
into the §12 budget.

**The `${}` scanner** (`scan_interpolation`, `src/lex/string.rs`):

```text
after '$' with src[i+1] == '{':
  depth = 1;  start = i + 2
  loop:
    b == '"' | '\'' | '`'   -> skip a nested string literal with the appropriate scanner
                                 (three-quote aware, escape aware, interpolation aware) and continue
    b == '{'                 -> depth += 1
    b == '}'                 -> depth -= 1; if depth == 0 -> done
    EOF                      -> LexError(UnterminatedInterpolation)
    otherwise                -> i += 1
```

**RULING S7 — the `${}` body is string-aware.** The legacy scanner counts braces with no string
awareness at all (`src/runtime.rs:874-905`), so `"${d["}"]}"` lexes wrong: the first `}` inside the
string literal closes the interpolation. The new scanner skips nested string literals as units, so
that source yields `Interp([Expr("d[\"}\"]")])` and the dictionary lookup works.

**RULING S8 — comments are not honoured inside `${…}`.** `//` and `#` inside an interpolation are
`Slash` and `Hash` bytes belonging to the expression, and `#{` there is a dict. The sub-lexer
*will* treat `// …` as a comment if the body contains a newline, because its own ASI rules apply —
but that is the sub-lexer's business, not the string scanner's. A one-line `${a // b}` is a
`/ /` token pair and a parse error. Write the comment outside the interpolation, or use a
multi-line interpolation. This is a documented limitation, not a bug; the alternative is a second
mini-lexer state machine whose only benefit is a `//` in a one-line interpolation.

`$` not followed by `{` is a literal `$`. `$$` is two literal `$`s — there is no `$$` template
sigil. `\${` escapes the sequence.

**Payload representation.**

```rust
pub struct Interp<'a> {
    pub parts: Vec<InterpPart<'a>>,
    pub subs:  Vec<Lexed<'a>>,          // one per Expr part, in order
}

pub enum InterpPart<'a> {
    Text(Range<'a>),   // already-cooked bytes; escapes applied; no interpolation inside
    Expr(u32),         // index into `subs`
}
```

Nested interpolation (`"${ "${x}" }"`) works because S7 skips the nested string wholesale and the
outer body is handed to the sub-lexer, which runs the same scanner again. Depth is bounded by
`MAX_INTERP_NESTING = 8` (§9.3).

### 6.4 Cooked, not raw

`Str` payloads and `Interp::Text` parts are **cooked** — escapes already applied — so the M3
tree-walker and the M4 constant pool never re-scan a string. The one thing kept raw is the `${}`
body, per RULING S6.

Cooking always yields valid UTF-8: `\xNN` produces a `char` in `0x00..=0xFF` and
`String::from`/`str::push` always produce well-formed UTF-8. The only invalid UTF-8 that could
enter a payload would have to come from the source, and §2 assumes the source is valid UTF-8. The
interner therefore stores `&'a str` for `idents` and `&'a [u8]` for `strs`, with a
`debug_assert!(std::str::from_utf8(bytes).is_ok())` in the insert path.

---

## 7. Statement separation and newlines

Zen has no mandatory semicolons. A statement is terminated by a newline or by `;`, and a newline is
*suppressed* — emitted as nothing — when the expression is obviously still open. This is the
Kotlin/Python-style rule the legacy corpus is written against;
`tests/cases/18_lang_features.z:10-55` tests both directions, and its final negative case
(`tests/cases/18_lang_features.z:52-55`) exists specifically to prove that a trailing operator does
**not** swallow the following statement.

### 7.1 The delimiter stack

```rust
#[derive(Clone, Copy)]
struct Delim { byte: u8, suppress: bool }
```

| Opener | pushed | `suppress` |
|---|---|---|
| `(` `LParen` | yes | **true** |
| `[` `LBracket` | yes | **true** |
| `#{` `DictStart` | yes | **true** |
| `%{` `SetStart` | yes | **true** |
| `#(` `TupleStart` | yes | **true** |
| `{` `LBrace` (block) | yes | **false** |

A closer (`)`/`]`/`}`) pops **only if the top of the stack matches**; a mismatch is a no-op here and
a parse error in the parser. This is strictly more correct than the legacy saturating counter at
`src/runtime.rs:1316-1320`, which cannot distinguish `func f() { [1] }`'s `}` from a dict's `}`.

`MAX_DELIM_DEPTH = 256`; exceeding it is `LexError(NestingTooDeep)`. This is a front-door guard:
the parser recurses once per delimiter, so bounding the delimiter depth bounds that recursion.
`MAX_STMT_DEPTH` and `MAX_AST_DEPTH` are the parser's separate guards
([`06-parser.md`](06-parser.md) §10.4).

`suppress_newline()` is `self.delims.last().is_some_and(|d| d.suppress)` — one byte load from the
top of a `Vec<Delim>` with capacity 16.

**RULING ASI-0 — the three new openers suppress newlines.** The legacy lexer counts only `(` and
`[`, and says why in a comment at `src/runtime.rs:677-678`: *"braces are not counted: blocks need
newlines"*. The new lexer adds `#{`, `%{` and `#(`. The reason is that this is the form every Zen
programmer will write:

```zen
var d = #{
    a: 1,
    b: 2,
}
```

and making `...` the only thing that makes it work is a trap. The `suppress = false` on a bare `{`
is what keeps `match`/`switch`/`class`/`func` bodies splitting on newlines. The two cases are
separated by the *token*, not by context, so this costs nothing and needs no state.

### 7.2 Suppression rules — the complete list

A `\n` is **suppressed** if and only if at least one of the following holds. This list is
exhaustive.

| Id | Condition | Example |
|---|---|---|
| **A1** | `suppress_newline()` is true — we are inside `(`, `[`, `#{`, `%{` or `#(` | `max(10,\n    max(2,\n        7))` |
| **A2** | The previous **significant** token is in the operator set below | `let s = "a" +\n  "b"` |
| **A3** | The previous significant token is `Newline` or `Semi` (blank lines, `;;`, comment-only lines) | `a = 1`, a comment line, then `b = 2` |
| **A4** | No **significant** token has been emitted yet in this file (shebang, header comment, leading blanks) | `#!shebang\n` at offset 0; row 12 of §4.2 |
| **A5** | The `\n` is inside `/* … */` or a `"""…"""` / `'''…'''` literal | handled by those scanners, which consume the newline without reaching this code |

**The A2 operator set** is exactly the legacy `continues_expression` at
`src/runtime.rs:491-545` — the empirical, corpus-validated list:

```text
Plus  Minus  Star  Slash  Percent  Pow
Amp   Pipe   Caret Tilde  LShift    RShift
Eq    Ne     Lt    Le    Gt        Ge
StrictEq StrictNe  KwIs  KwIn
KwAnd KwOr  Nullish
Assign  PlusEq  MinusEq  StarEq  SlashEq  PercentEq
AmpEq   PipeEq  CaretEq  ShlEq    ShrEq     NullishEq
Arrow   ThinArrow  KwLambda  Question
Colon   Dot    SafeDot  DotDot  Comma  Ellipsis
LParen  LBracket  LBrace
```

Three consequences, all deliberate, all surprising enough to state:

- **`Dot` and `SafeDot` continue a line.** The check looks at the token *before* the newline
  (`src/runtime.rs:697`: `prev.is_some_and(continues_expression)`), so this applies when the
  newline *follows* a dot:

  ```zen
  var x = 5.
  real
  ```

  Note the trailing dot is required, and `5.` alone is a trailing-dot float (N6). This is the
  legacy behaviour and it is what makes `tests/cases/18_lang_features.z:26` and fluent chains work.
  It is also a real footgun, and it is why a statement must never begin with `.` in a way the user
  did not intend.
- **`Dot` at the *start* of a line does not continue — the newline stands.** Because the rule is
  previous-token-only, this is **two** statements, not one:

  ```zen
  var x = 5
      .real
  ```

  lexes as `KwVar ID(x) Assign INT(5) NL Dot ID(real) NL`. A2 asks "does the previous significant
  token continue an expression?" and the answer for `INT(5)` is no, so the newline is a terminator
  and the leading `.` starts a new statement (a parse error). This was an open contradiction in an
  earlier draft of this document, which asserted one statement here; the previous-token rule and
  the legacy implementation are normative, so the example is a parse error. Put the dot at the end
  of the previous line, or parenthesise the chain.
- **`Colon` continues a line.** So `case 1:\n  "one"` and `a ? b\n: c` both work, and a match arm
  can put its body on the following line.
- **`DotDot` (`..`) continues a line.** So `0..\n10` is a range.

A newline is a **terminator** in every other case, including after `RBrace`, `RParen`, `RBracket`,
`Int`, `Str`, `Ident`, `KwReturn`, `KwElse`, `KwElif`, `KwCatch`, `KwFinally`, and any closing
keyword.

`return` followed by a newline yields `Return(None)`; the legacy parser already tests for exactly
this at `src/runtime.rs:1983-1989`, and does the same for `throw` at `src/runtime.rs:2337-2344`.
This is Python semantics, not Go's "missing operand" error, and it is preserved.

**RULING ASI-1 — what "a trailing block" means.** The question "is a newline after `}` a
terminator?" has a single answer, inherited from the legacy table: `RBrace` is *deliberately
absent* from it. Compare `src/runtime.rs:543`, where `Kind::LBrace` is the last entry and there is
no `Kind::RBrace` anywhere in `continues_expression`. Therefore:

```zen
func f() { }
func g() { }
```

is two statements: a block closes a statement. The one place this looks wrong is

```zen
if c { a }
else { b }
```

and it is not wrong. The `Newline` after the first `}` is emitted, and the parser's
`skip_separators()` — the legacy `fn separators()` at `src/runtime.rs:1624-1628` — eats it before
`else`. The same covers `try … catch`, `if … elif`, and match-arm chains. So the lexer stays
simple and the parser stays tolerant, which is the correct division of labour.

### 7.3 Semicolons

`;` is a statement separator, interchangeable with a newline (`grammar.md:21-22`).

**RULING ASI-2 — at most one separator token per run.** After emitting a `Semi` or a `Newline`, any
immediately following `;`, blank lines, or comment-only lines produce nothing. So
`let a = 1;;\n\nlet b = 2` yields `KwLet Ident(a) Assign Int(1) Semi KwLet Ident(b) Assign Int(2)
NL Eof`. The legacy lexer emits both the `Semi` and the `Newline`; the parser's `skip_separators()`
loop makes the difference unobservable, and collapsing halves the token count on
separator-heavy code. A3 already implements this for newlines; the `;`-run case is the same code
path.

Because the collapsed bytes must still be *inside* a span — T-P3 sums token and error spans to
`src.len()` — the surviving separator token's `hi` widens to cover the run. So `;;` is one `Semi`
spanning `[0, 2)`, and `;` followed by a blank line is one `Semi`/`Newline` spanning both. The `lo`
still points at the first separator byte, which is what the parser needs.

### 7.4 End of stream

The stream always ends with exactly one `Eof` token whose `lo` and `hi` are both the source length,
preserved from `src/runtime.rs:1325-1330`. **No synthetic trailing `Newline` is emitted**: the
parser's `skip_separators()` accepts `Eof` as a terminator, which the legacy parser already does
at `src/runtime.rs:1986`, `src/runtime.rs:2339`, and `src/runtime.rs:2384`.

### 7.5 Worked cases

| Source | Tokens | Why |
|---|---|---|
| `a = 1\n` | `Ident Assign Int NL Eof` | terminator |
| `a = 1 +\n 2\n` | `Ident Assign Int Plus Int NL Eof` | A2 (`Plus`) |
| `a = 1 -\n 2\n` | `Ident Assign Int Minus Int NL Eof` | A2 — `18_lang_features.z:16-19` |
| `a = 1 +\n 2 * 3\n` | `Ident Assign Int Plus Int Star Int NL Eof` | A2; precedence is the parser's job — `18_lang_features.z:21-23` |
| `a = (\n1\n)\n` | `Ident Assign LParen Int RParen NL Eof` | A1 |
| `a = [\n1,\n2,\n]\n` | `Ident Assign LBracket Int Comma Int Comma RBracket NL Eof` | A1 — `18_lang_features.z:36-41` |
| `a = #{\n"a":1,\n}\n` | `Ident Assign DictStart Str Colon Int Comma RBrace NL Eof` | A1 + ASI-0 — the `18_lang_features.z:30-34` case |
| `match v {\n1: "one",\n_ => "x",\n}\n` | `KwMatch Ident LBrace NL Int Colon Str Comma NL Ident ThinArrow Str Comma NL RBrace NL Eof` | `LBrace` is non-suppressing; `Colon`/`Comma` suppress the newline *after* them |
| `if c {\na\n}\nb\n` | `KwIf Ident LBrace Ident NL RBrace NL Ident NL Eof` | A2 on `{` kills the first newline; `RBrace` terminates |
| `a = 1 +\n/* c */\n2\n` | one statement | the comment emits no token, so `prev` is still `Plus` |
| `a = 1 // c\nb\n` | `a` `NL` `b` `NL` | |
| `a = 1\n\n\nb\n` | `a` `NL` `b` `NL` | A3 |
| `\n\na\n` | `a` `NL` | A4 |
| `a = 1;\nb\n` | `a` `Semi` `b` `NL` | ASI-2 |
| `a = 1\n;\n` | `a` `Semi` | ASI-2 collapses the newline after a `Semi` |
| `return\nx\n` | `KwReturn NL Ident NL` | `Return(None)`, then a separate statement |
| `f(\n)\n` | `Ident LParen RParen NL` | A1 — an empty argument list, not a call with a missing argument |
| `a = 1\n-2\n` | `a` `NL` `-2` `NL` | `-` at line start is unary: `Minus` is *not* the previous token |

The last row is the negative case from `tests/cases/18_lang_features.z:52-55` and it is the one
that most implementations get wrong: the suppression rule looks at the *previous* token, and after
`a = 1` the previous token is `Int(1)`, not `Minus`.

---

## 8. Line continuation

**RULING LC-1 — backslash-newline is a line continuation in string literals and nowhere else.**

```zen
var s = "abc\
def"
```

produces `Str("abcdef")`. The backslash-newline pair is removed *before* escape processing, so
`\` + newline never reaches the escape table and cannot be confused with `\n`, which is two
characters that are both present in the source.

It is **not** a continuation outside a string. A `\` on its own line is
`LexError(UnexpectedCharacter('\\'))` with the message "a line continuation is only valid inside a
string literal". Kotlin, Java, C# and Python all restrict it to strings, and a bare `\` at
statement level is otherwise an immediate syntax error anyway, so this converts a confusing later
error into an immediate one with a caret.

A trailing `\` at end of file is a distinct message — "the file ends with an unfinished line
continuation" — because that case is almost always a truncated file and the fix is different.

**RULING LC-2 — inside `${…}` a backslash-newline is not a continuation.** The body is raw source
(§6.3, RULING S6), so a `\` there is a parse error in the sub-lex. This is the direct, intended
consequence of "the body is source, not text".

---

## 9. Errors and diagnostics

### 9.1 Error type

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexError {
    pub kind:     LexErrorKind,
    pub severity: Severity,           // Error | Warning
    pub span:     Span,               // the full offending region
    pub at:       Span,               // where the caret goes (often the opening quote)
    pub notes:    Vec<DiagnosticNote>, // the "= help:" / "= note:" lines
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity { Error, Warning }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LexErrorKind {
    UnterminatedString         { form: StrForm, line: u32 },
    UnterminatedTripleString   { form: StrForm },
    UnterminatedBlockComment,
    UnterminatedInterpolation  { expr_line: u32, expr_col: u32 },
    RawNewlineInString         { line: u32 },
    UnfinishedEscape,
    UnknownEscape              { byte: u8 },   // Warning (RULING S5)
    IncompleteHexEscape,
    InvalidHexEscape,
    InvalidUnicodeEscape       { tag: u8 },    // b'u' | b'U'
    UnterminatedUnicodeEscape  { tag: u8 },
    EmptyUnicodeEscape         { tag: u8 },
    UnicodeEscapeTooLong       { tag: u8 },
    InvalidCodePoint           { cp: u32 },
    InvalidNumber              { reason: NumErr },
    UnexpectedCharacter        { ch: char },
    StrayLineContinuation,
    TrailingLineContinuation,
    NestingTooDeep             { depth: u32 },
    TooManyErrors              { limit: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumErr {
    NoDigits, NoDigitsAfterPrefix, InvalidDigit,
    EmptyExponent, SecondDot, BadSeparator, GluedIdentifier,
}
```

`Severity` is per-error rather than per-`Lexed`, so RULING S5's unknown-escape warning can coexist
with hard errors in the same run.

### 9.2 Recovery, not abort

The legacy lexer returns `Err(String)` on the first problem — `src/runtime.rs:737`, `:806`, `:892`,
`:913`, `:1017`, and the `unexpected character` at `:1313`. One typo yields one diagnostic, and the
user fix-punctuates five times.

The new lexer **records the error and keeps scanning**:

| Error | Recovery |
|---|---|
| `UnexpectedCharacter`, `StrayLineContinuation` | record; advance one byte |
| any string-family error | record; **resynchronise to the end of the physical line**, so a missing quote costs one error rather than one per line |
| `InvalidNumber` | record; **rewind to `start + 1`** and rescan as a fresh token, so `1.2.3` reports the second dot and does *not* then also report `3` as an error |
| `UnterminatedInterpolation` | record; resynchronise to end of line |
| `UnterminatedBlockComment` | record; jump to EOF — nothing after it is recoverable |
| `NestingTooDeep` | record; emit the token but do not push, and stay in "degraded" mode: every later delimiter is lexed as plain punctuation. The parser reports the depth again at the first `(` in an expression, and the user gets two diagnostics pointing at the same construct rather than 256 nested ones. |

This guarantees the invariant the whole project rests on: **the number of errors is bounded by the
number of wrong lines, not the number of wrong bytes.**

### 9.3 Never panics

1. **No `unsafe`.** The crate is `#![forbid(unsafe_code)]`. Every `&[u8]` access goes through
   `src.get(i)` and every sub-slice through `src.get(a..b)`, both of which return `Option`.
2. **Banned lints**, in `crates/zen-syntax/Cargo.toml`:

   ```toml
   [lints.clippy]
   indexing_slicing        = "deny"
   unwrap_used             = "deny"
   expect_used             = "deny"
   panic                   = "deny"
   ```

   `arithmetic_side_effects` is *not* denied crate-wide; it is denied on the `span` module only,
   where the offsets live, because span arithmetic is the one place an overflow would be a silent
   wrong answer rather than a panic. Span arithmetic uses `u32` with explicit `checked_add` at the
   two places that can cross a `u32::MAX` boundary (a 4 GB single file, which the M4 compiler
   rejects earlier anyway).
3. **Infallible arithmetic.** Every `i` is bounded by `i <= src.len()`; every emitted token passes
   `debug_assert!(lo <= hi && hi as usize <= src.len())` in the single `push` helper.
4. **Loop termination.** Every scanner either advances `i` or returns. The only scanner that can
   return without advancing is `eat_digits` at EOF, which returns `src.len()`, and the main loop
   condition is `i < src.len()`. A `debug_assert!(i > prev_i)` at the top of the main loop turns a
   hypothetical non-advancing path into a test failure, not a hang.
5. **Bounded nesting.** `MAX_DELIM_DEPTH = 256`, `MAX_INTERP_NESTING = 8`. The lexer is iterative,
   so a 200 MB file cannot overflow the stack. The parser's recursion is bounded by
   `MAX_DELIM_DEPTH` (here) plus `MAX_STMT_DEPTH` and `MAX_AST_DEPTH` in
   [`06-parser.md`](06-parser.md) §10.4.
6. **Bounded output.** `MAX_LEX_ERRORS = 64`. On overflow one `TooManyErrors` entry is appended and
   scanning continues in recovery mode without recording further errors.
7. **Runtime backstop.** Per [`00-blueprint-review.md`](00-blueprint-review.md) §5.1 the binary
   installs a panic hook that converts an escaped panic into a Zen error with a non-zero exit code.
   That is a backstop for the whole binary, not a licence for the lexer to be careless.

### 9.4 Diagnostic rendering

```rust
pub fn render(d: &Diagnostic<'_>, files: &SourceMap, color: ColorMode) -> String;
pub enum ColorMode { Always, Auto, Never };   // Auto = isatty(stderr) && NO_COLOR unset
```

Output, with a caret under the offending span:

```text
error: unterminated string literal
  --> /proj/app.z:12:9
   |
12 |     var s = "hello
   |                ^^^^^^^^^
   |                |
   |                this string is never closed
   |
   = help: add a matching `"` to close the string
   = note: for a multi-line string use """ ... """
```

A span that crosses a line boundary names the end line and marks the end:

```text
error: unterminated block comment
  --> /proj/app.z:3:1
   |
 3 | /@@ start
 4 | |@@    still going
 5 | |@@ */ end
   |    ^
   |
   = help: add `*/` to close the comment block
```

(`/@@` and `|@@` are inline annotations added only when `--inline-annotations` is on; the default
output is the first form.)

**Rendering rules** — all pure functions of the span, all in `src/diag.rs`:

- Columns are 1-based and counted in **characters**, not bytes, so a caret under a multi-byte
  character lands correctly. Tabs are expanded to the next multiple of 4 in both the source line and
  the caret run so they stay aligned.
- A zero-length span renders as one `^`; length 1 as `^`; length ≥ 2 as `^` repeated once per
  character. The example above shows 9 characters, `"hello`.
- `= help:` and `= note:` are the legacy house style (`src/runtime.rs:738-740`, `:829-830`, `:894`,
  `:914`, `:1018`, `:1313`) and are preserved verbatim wherever an equivalent message exists, so
  existing documentation and existing user expectations carry over unchanged.
- Colour is a 256-colour `\x1b[…m` wrapper, matching the legacy convention, applied only when
  `ColorMode::Auto` sees a terminal. `zenn check --color=never` is what CI and the differential
  runner use, so golden files stay byte-stable.
- One diagnostic is at most 12 lines. A construct with more context is truncated with
  `= note: … and N more lines`.

---

## 10. Implementation plan

### 10.1 File layout

```text
crates/zen-syntax/
  Cargo.toml
  src/
    lib.rs              pub mod span; pub mod diag; mod source; mod lex; mod ast; mod parse;
    span.rs             FileId, Span, SpanTable, LineIndex
    source.rs           SourceMap, LoadedFile, file-id assignment, module-path helpers
    diag.rs             Diagnostic, DiagnosticNote, Severity, render()
    lex/
      mod.rs            pub fn lex(); Lexer; Lexer::new / run / next_token
      kind.rs           TokenKind + Display + the keyword table + the FROZEN numeric asserts
      token.rs          Token, NumLit, RawNum, Lexed, Interp, InterpPart
      number.rs         scan_number, eat_digits, decimal_to_f64, NumErr
      string.rs         scan_string (5 forms), scan_escape, scan_interpolation, StrForm
      table.rs          class_of[256], operator_index[256], DIGITS2/8/10/16
      interner.rs       Interner<'a>: zero-copy &str -> u32
  src/bin/
    zentok.rs           dev binary: dump a file's token stream
  benches/
    lex.rs              criterion: lex throughput
  tests/
    lex_tokens.rs       the §11 tables as #[test] fns
    lex_corpus.rs       the 32-file legacy corpus conformance gate
```

`zentok` exists because both the M3↔M4 differential harness
([`00-blueprint-review.md`](00-blueprint-review.md) §2 A6) and the golden-token tests need a way to
dump a stream without a running VM:

```console
$ cargo run -q -p zen-syntax --bin zentok -- tests/cases/18_lang_features.z
   1:1   KwPrint  print
   1:7   Ident    fails
   1:13  KwVar    var
  …
  12:1   error    unexpected character `\`  (caret block follows)
```

### 10.2 Core structures

```rust
pub struct Lexer<'a> {
    src:  &'a [u8],
    file: FileId,

    // ── cursor ──────────────────────────────────────────────────────────
    i:          u32,             // byte offset, always <= src.len()
    delims:     Vec<Delim>,      // capacity 16; MAX_DELIM_DEPTH 256
    last:       TokenKind,       // previous SIGNIFICANT token kind (rule A2)
    has_last:   bool,
    after_dot:  bool,            // rule K1
    sep_pending: bool,           // rule ASI-2
    degraded:   bool,            // set by NestingTooDeep; delimiters no longer tracked
    depth:      u32,             // current delimiter depth, for the guard

    // ── output ──────────────────────────────────────────────────────────
    toks:       Vec<Token>,
    idents:     Interner<'a>,
    strs:       Interner<'a>,
    interps:    Vec<Interp<'a>>,
    nums:       Vec<NumLit>,
    errors:     Vec<LexError>,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str, file: FileId) -> Self;

    /// Drive `next_token` to exhaustion. Used by `lex()` and by the REPL.
    pub fn run(&mut self) -> Lexed<'a>;

    /// Produce the next token, or `None` after the `Eof` token has been pushed.
    /// Exposed so the M8 REPL can lex one input line at a time and carry
    /// `delims` / `last` / `after_dot` across the line boundary.
    pub fn next_token(&mut self) -> Option<Token>;

    #[inline]
    fn push(&mut self, kind: TokenKind, aux: u32, lo: u32, hi: u32) {
        debug_assert!(lo <= hi && hi as usize <= self.src.len());
        self.toks.push(Token { kind, aux, lo, hi });
        if kind != TokenKind::Newline && kind != TokenKind::Semi {
            self.last = kind;
            self.has_last = true;
        }
        self.after_dot = matches!(kind, TokenKind::Dot | TokenKind::SafeDot);
    }

    #[cold]
    fn error(&mut self, kind: LexErrorKind, lo: u32, hi: u32, at: (u32, u32), sev: Severity);
}
```

### 10.3 The scanning loop

```rust
pub fn lex<'a>(src: &'a str, file: FileId) -> Lexed<'a> {
    let mut lx = Lexer::new(src, file);
    lx.run();
    lx.finish()
}

impl<'a> Lexer<'a> {
    /// `next_token` returns `None` at end of input, so `Eof` is pushed exactly once, by
    /// `finish()` — never here. Pushing it in both places is the double-`Eof` bug this shape
    /// exists to prevent; T-A17 asserts the last token of every corpus file is exactly one `Eof`.
    pub fn run(&mut self) {
        let n = self.src.len();
        self.toks.reserve(n / 5 + 16);
        while let Some(t) = self.next_token() { self.toks.push(t) }
    }

    fn next_token(&mut self) -> Option<Token> {
        let src = self.src;
        let n = src.len();
        loop {
            if self.i as usize >= n { return None }

            let b = src[self.i as usize];
            let class = class_of(b);                 // one table load, §10.4 M1

            match class {
                Class::Space  => { self.i += 1; continue }
                Class::Newline=> { self.on_newline(); return Some(self.take_tok(TokenKind::Newline)) }
                Class::Semi   => { self.on_separator(); return Some(self.take_tok(TokenKind::Semi)) }
                Class::Hash   => match self.lex_hash() {              // §4, the one interesting branch
                                    Some(t) => return Some(t),
                                    None    => continue,            // it was a comment
                                },
                Class::Quote  => return Some(self.lex_string()),
                Class::Digit  => return Some(self.lex_number()),
                Class::IdentStart => return Some(self.lex_word()),
                Class::Slash  => {
                    match src.get(self.i as usize + 1) {
                        Some(b'/') => { self.skip_line_comment(); continue }
                        Some(b'*') => { self.skip_block_comment(); continue }
                        _          => return Some(self.lex_operator()),
                    }
                }
                _             => return Some(self.lex_operator()),
            }
        }
    }
}
```

`take_tok` reads the token's start from `self.tok_start`, which the dispatch sets once per
iteration; no scanner has to pass a `lo` around, and there is no way to forget it. The legacy
lexer threads the same information through `previous()` (`src/runtime.rs:1497-1499`).

`lex_hash` is eight lines and is the whole of §4. It returns `Option<Token>` rather than
`Option<Option<Token>>` and the `continue` lives at the call site, so the "a `#` comment emits
nothing" fact is expressed once instead of being re-encoded as an `unreachable!()`:

```rust
/// `Some(token)` for `#{` and `#(`; `None` for a line comment, which the caller turns into a
/// `continue`. There is no fallible path and no `unreachable!`.
#[inline]
fn lex_hash(&mut self) -> Option<Token> {
    match self.src.get(self.i as usize + 1) {
        Some(b'{') => { self.i += 2; self.push_delim(TokenKind::DictStart);
                       Some(self.take_tok(TokenKind::DictStart)) }
        Some(b'(') => { self.i += 2; self.push_delim(TokenKind::TupleStart);
                       Some(self.take_tok(TokenKind::TupleStart)) }
        _          => { self.skip_line_comment(); None }
    }
}
```

`Some(b'[')` is deliberately absent: `#[a: 1]` is a comment (RULING H1), so the two-byte `#[`
falls into the comment arm with no extra test. There is exactly **one** `match` on the byte after
`#`, one `src.get`, and no fallible path.

`lex_operator` uses the first-byte operator index (**M2**):

```rust
#[inline]
fn lex_operator(&mut self) -> Token {
    let rest = &self.src[self.i as usize..];
    // Every non-whitespace, non-ident-start byte has at least a 1-byte entry in its bucket, so
    // this loop always returns. An unmatched byte is not reachable from here: `class_of` sends
    // every byte it does not recognise to `Class::Illegal`, which has its own arm above.
    for &(text, kind) in OPERATORS_BY_FIRST_BYTE[rest[0] as usize] {
        if rest.starts_with(text.as_bytes()) {
            self.i += text.len() as u32;
            if matches!(kind, TokenKind::LParen | TokenKind::LBracket
                          | TokenKind::DictStart | TokenKind::SetStart
                          | TokenKind::TupleStart | TokenKind::LBrace) {
                self.push_delim(kind);
            }
            return self.take_tok(kind);
        }
    }
    unreachable!("every operator byte has a 1-byte fallback in OPERATORS_BY_FIRST_BYTE")
}
```

`Class::Illegal` is a separate arm in `next_token` and calls `lex_illegal()` directly, so there is
exactly **one** route to an `Illegal` token and no chance of the operator path emitting one.

Each bucket is sorted longest-first, so `**` is tried before `*`, `<<=` before `<<` before `<`, and
`===` before `==` before `=`. This replaces the twenty sequential three-byte slice comparisons at
`src/runtime.rs:1106-1165` and the twenty-eight two-byte comparisons at `src/runtime.rs:1166-1287`
with, on average, 1.4 compares.

### 10.4 Micro-optimisation list

| Id | Optimisation | Why |
|---|---|---|
| **M1** | A `static [Class; 256]` dispatch table (`class_of`) instead of a `match` on byte ranges | One load replaces a 12-arm range match. This is the single biggest win and the reason the main loop has no range comparisons. |
| **M2** | A 256-entry first-byte index into `&'static [(&'static str, TokenKind)]` buckets, sorted longest-first | Replaces ~48 sequential string compares in the legacy operator path (`src/runtime.rs:1106-1312`) with ~1.4. |
| **M3** | memchr-style bulk skip for whitespace runs and comment bodies | In a single-buffer scan this is a plain `while` over `class_of`; the real `memchr2` is only worth it if profiling shows runs longer than 8 bytes, which real Zen code rarely has. Recorded as "measure, do not assume" — a SIMD `memchr` on 2-byte runs is *slower* than the table loop because of the call overhead. |
| **M4** | No `char_indices()` anywhere in the hot loop. Work on `&[u8]` with `u32` indices; call `str::from_utf8` only when a token actually becomes a `&str` | Non-ASCII appears only inside identifiers, strings, and comments — the slow path. `chars()` on the whole file would decode every multi-byte character up front for nothing. |
| **M5** | One `&[u8]` scan, no `Peekable`, no `Vec<char>`, no re-chunking; all lookahead is `src.get(i + 1)` | `Vec<char>` costs 4 bytes/char and a UTF-8 decode per character, and it is the usual reason a Rust lexer is 3× slower than the equivalent C one. |
| **M6** | Replace the `out.last()` scans at `src/runtime.rs:696` and `src/runtime.rs:1043-1046` with the `last` / `after_dot` fields | Removes a random access into the output vector from two of the hottest branches. |
| **M7** | Four specialised string scanners (`"`, `'`, `"""`, `'''`) plus the backtick scanner, rather than one parameterised scanner | The inner loop's terminator test becomes a single `==` against a register-held byte instead of a table lookup plus a "am I in triple mode" test. |
| **M8** | Do not maintain a column counter. Build the `LineIndex` once and compute line/col on demand | The legacy lexer increments `col` on every single byte (`src/runtime.rs:686` and roughly forty other sites). On the 84 KB corpus that is ~84,000 wasted increments. |
| **M9** | `Vec::with_capacity(src.len() / 5 + 16)` | Measured token density on the corpus is 0.26 tokens/byte, so `/5` over-allocates by ~2×; it is tuned down to `/6` after the first benchmark run. |
| **M10** | Zero-copy interning: `Interner<'a>` stores `&'a str` slices pointing into the source buffer, so lexing a file allocates at most one `String` for the interner's own table | Most Zen programs have far fewer distinct identifiers than identifier occurrences (measured 3.1× on the corpus), so interning shrinks both the token stream and the AST. |
| **M11** | Comment bodies produce no token and no state change at all | Removes a branch from the identifier path and makes the A2 rule in §7.2 work for free (row 34 of the §4.2 truth table). |
| **M12** | The `#` handler is one `match` on `src.get(i + 1)` with `None` folding into the comment case | No allocation, no backtracking, no second pass — and the EOF case needs no separate branch. |
| **M13** | The ASI state is `delims: Vec<Delim>` (2 bytes per entry, capacity 16) plus `last: TokenKind` | A `u16` counter would be smaller but cannot tell `func f() { [1] }`'s `}` from a dict's `}` (§7.1). 16 bytes of stack traffic per delimiter beats a correctness bug. |
| **M14** | `#[inline]` on `push`, `take_tok`, `lex_hash`, `eat_digits`, and the interner insert; `#[cold]` on `error`, `lex_illegal`, and `finish` | The error paths are cold and should not be inlined into the main loop's instruction footprint. |
| **M15** | Build the `LineIndex` in `finish()`, not incrementally | One `memchr::memchr_iter` pass over the source is faster than 84,000 branch-free increments, and it is a pass the compiler can vectorise. |

Explicitly **not** done, with reasons:

- **A perfect hash for keywords.** `match word { "let" => …, "const" => … }` compiles to a
  length switch plus one `memcmp` per candidate of that length, which for ≤10-byte keys is
  typically faster than a hash lookup plus a probe. Revisit only if M1–M3 profiling disagrees.
- **SIMD skipping of identifier runs.** Zen identifiers are short; the SIMD setup cost exceeds the
  gain. Same argument as M3.
- **A `memchr`-based pre-pass to find string/comment boundaries.** That would be two passes over
  the buffer, and the §2.5 single-pass guarantee is worth more than the throughput.

---

## 11. Test plan

Notation: `∅` means the lexer emits no token; `NL` is `Newline`; `Eof` is always present and is
**not** repeated in every row. `ID(x)` is `Ident` with text `x`; `INT(n)` / `FLT(x)` are `Int` /
`Float` with that payload; `STR(s)` is `Str` with cooked payload `s`. `→ err(X)` marks a row that
produces the stated `LexErrorKind` *and continues scanning*.

### 11.1 Atoms and keywords

| # | Test | Input | Expected |
|---|---|---|---|
| T-A1 | identifier | `foo` | `ID(foo)` |
| T-A2 | underscore identifier | `_x1` | `ID(_x1)` |
| T-A3 | keyword | `let` | `KwLet` |
| T-A4 | keyword after dot is an ident | `a.default` | `ID(a)` `Dot` `ID(default)` |
| T-A5 | keyword after safe-dot is an ident | `a?.null` | `ID(a)` `SafeDot` `ID(null)` |
| T-A6 | keyword is not an ident elsewhere | `null` | `KwNil` |
| T-A7 | `global` folds to `KwVar` | `global` | `KwVar` |
| T-A8 | `def` folds to `KwFn` | `def` | `KwFn` |
| T-A9 | `null` and `nil` are one kind | `nil null` | `KwNil KwNil` |
| T-A10 | all six fn spellings | `fn func function procedure proc def` | `KwFn` ×6 |
| T-A11 | `&&` folds to `KwAnd` | `a && b` | `ID(a) KwAnd ID(b)` |
| T-A12 | `except` folds to `KwCatch` | `except` | `KwCatch` |
| T-A13 | empty file | `` | `Eof` only, no `Newline` |
| T-A14 | whitespace-only file | `  \n\t\n` | `Eof` only (A3 + A4) |
| T-A15 | BOM is skipped, spans stay in the original `src` | `\u{FEFF}let x = 1` | `KwLet` … and `lo` of the first token is `3`, not `0` |
| T-A16 | a non-leading BOM is illegal | `a\u{FEFF}b` | `→ err(Illegal)` |
| T-A17 | exactly one `Eof`, always last | every file in `tests/cases/` | `toks.last().kind == Eof` and `toks.iter().filter(Eof).count() == 1` |
| T-A18 | no unlisted discriminant | a generated `match` over every `TokenKind` | every `kind as u8` is in §3.1, §3.2, or §3.3, or is 99 / 111–140 |

### 11.2 Comments and the `#` truth table

Every row of the §4.2 table is a test with the same id prefix, `T-H1` … `T-H35`. The
non-obvious ones are duplicated here so the intent is visible in the test file:

| # | Test | Input | Expected |
|---|---|---|---|
| T-H6 | `#{a:1}` | `#{a:1}` | `DictStart ID(a) Colon INT(1) RBrace` |
| T-H14 | hash-space-brace is a comment; A4 then suppresses the newline | `# {a:1}` | `∅` (not `NL`) |
| T-H18 | comment ends at the newline, next line is a dict | `x = 1 #\n#{a:1}` | `ID(x) Assign INT(1) NL DictStart ID(a) Colon INT(1) RBrace` |
| T-H20 | spaced `%{` is modulo + block | `a % {b}` | `ID(a) Percent LBrace ID(b) RBrace` |
| T-H21 | glued `%{` is a set | `a %{b}` | `ID(a) SetStart ID(b) RBrace` |
| T-H23 | sigils inside strings are literal | `"#{"` | `STR("#{")` |
| T-H30 | comment swallows a newline | `/* a\nb */x` | `ID(x)` |
| T-H34 | comment mid-expression emits nothing | `a /*x*/ b` | `ID(a) ID(b)` |
| T-H35 | dict close does not underflow | `#{a:1}` | `… RBrace` with no `NestingTooDeep` |

### 11.3 Numbers

| # | Test | Input | Expected |
|---|---|---|---|
| T-N1 | decimal int | `0` | `INT(0)` |
| T-N2 | int | `42` | `INT(42)` |
| T-N3 | leading-zero decimal, not octal | `010` | `INT(10)` |
| T-N4 | float | `3.14` | `FLT(3.14)` |
| T-N5 | leading-dot float | `.5` | `FLT(0.5)` |
| T-N6 | trailing-dot float | `5.` | `FLT(5.0)` |
| T-N7 | integer + exponent ⇒ Float | `1e10` | `FLT(1e10)` |
| T-N8 | uppercase exponent, negative | `1E-10` | `FLT(1e-10)` |
| T-N9 | signed exponent | `2.5E+3` | `FLT(2500.0)` |
| T-N10 | separators | `1_000_000` | `INT(1000000)` |
| T-N11 | hex lower | `0xff` | `INT(255)` |
| T-N12 | hex upper, prefix and digits | `0XFF` | `INT(255)` |
| T-N13 | hex separators | `0xFF_FF` | `INT(65535)` |
| T-N14 | binary | `0b1010_1010` | `INT(170)` |
| T-N15 | octal | `0o755` | `INT(493)` |
| T-N16 | `i48::MAX - 1` | `140737488355326` | `INT(140737488355326)` |
| T-N17 | `i48::MAX` | `140737488355327` | `INT(140737488355327)` |
| T-N18 | `i48::MAX + 1` promotes | `140737488355328` | `FLT(1.40737488355328e14)` |
| T-N19 | `i64::MAX` promotes, does not wrap | `9223372036854775807` | `FLT(9.223372036854776e18)`, **not** `-1` |
| T-N20 | hex at the i48 boundary | `0x7FFFFFFFFFFF` | `INT(140737488355327)` |
| T-N21 | hex above the boundary | `0x800000000000` | `FLT(1.40737488355328e14)` (same as T-N18) |
| T-N22 | multi-dot rejected | `1.2.3` | → `err(InvalidNumber{SecondDot})` |
| T-N23 | multi-dot recovery | `1.2.3` | `FLT(1.2)` `FLT(0.3)` — **exactly one** error, per §9.2 |
| T-N24 | member access on a float is fine | `1.2.real` | `FLT(1.2)` `Dot` `ID(real)` |
| T-N25 | `..` after a number is a range | `1..3` | `INT(1) DotDot INT(3)` |
| T-N26 | glued identifier | `1abc` | → `err(InvalidNumber{GluedIdentifier})` |
| T-N27 | bare `e` | `1e` | → `err(InvalidNumber{EmptyExponent})` |
| T-N28 | trailing separator | `1_` | → `err(InvalidNumber{BadSeparator})` |
| T-N29 | doubled separator | `1__0` | → `err(InvalidNumber{BadSeparator})` |
| T-N30 | fast and slow float paths agree | 100,000 `proptest` decimal literals | `fast(s) == str::parse::<f64>(s)` |
| T-N31 | no digits after prefix | `0x` | → `err(InvalidNumber{NoDigitsAfterPrefix})` |
| T-N32 | digit outside the base | `0b2` | → `err(InvalidNumber{InvalidDigit})` |
| T-N33 | `5.` is a float, so `5.foo` is a glued identifier | `5.foo` | → `err(InvalidNumber{GluedIdentifier})` — the price of T-N6, and the same trade Python makes |
| T-N34 | separator directly after a radix prefix | `0x_FF` | → `err(InvalidNumber{BadSeparator})` (N4) |
| T-N35 | trailing separator in a radix literal | `0xFF_` | → `err(InvalidNumber{BadSeparator})` (N3) |

### 11.4 Strings

| # | Test | Input | Expected |
|---|---|---|---|
| T-S1 | plain double | `"a"` | `STR("a")` |
| T-S2 | single quotes, no interpolation | `'a${b}'` | `STR("a${b}")` |
| T-S3 | backslash | `"\\"` | `STR("\\")` |
| T-S4 | every simple escape | `"\n\t\r\0\a\b\f\v\\\'\"\`\$"` | `STR` with the §6.3 bytes, in order |
| T-S5 | `\e` equals `\x1b` | `"\e" "\x1b"` | two `STR`s, equal payloads |
| T-S6 | `\xNN` | `"\x41\x42\x43"` | `STR("ABC")` — the `18_lang_features.z:58` case |
| T-S7 | Latin-1 `\xFF` | `"\xFF"` | `STR` whose `len()` is 1 and whose bytes are `C3 BF` |
| T-S8 | `\u{…}` 6 digits | `"\u{1F600}"` | `STR` of one char, `len() == 1` — the `18_lang_features.z:60` case |
| T-S9 | `\U{…}` 8 digits rejected above `0x10FFFF` | `"\U{110000}"` | → `err(InvalidCodePoint)` |
| T-S10 | surrogate rejected | `"\U{D800}"` | → `err(InvalidCodePoint)` |
| T-S11 | `\u{}` empty | `"\u{}"` | → `err(EmptyUnicodeEscape)` |
| T-S12 | `\u{1234567}` too long | `"\u{1234567}"` | → `err(UnicodeEscapeTooLong)` |
| T-S13 | incomplete `\x` | `"\x1"` | → `err(IncompleteHexEscape)` |
| T-S14 | unknown escape warns, does not fail | `"\q"` | `STR("q")` + one `Warning` |
| T-S15 | unterminated | `"abc` | → `err(UnterminatedString)`, caret on the opening quote |
| T-S16 | unterminated triple | `"""abc` | → `err(UnterminatedTripleString)` |
| T-S17 | raw newline in a single-line string | `"a\nb"` (real newline) | → `err(RawNewlineInString)` |
| T-S18 | multi-line double | `"""a\nb"""` | `STR("a\nb")` with one real newline in the payload |
| T-S19 | `'''` does not interpolate | `'''a${b}'''` | `STR("a${b}")` |
| T-S20 | exact triple detection | `"" ""` | `STR("") STR("")` — two tokens, *not* one string |
| T-S21 | no triple backtick | ` ``x`` ` | `STR("") STR("x")` (assert no `Str` contains a backtick-run) |
| T-S22 | interpolation | `"a${b}c"` | `Interp[Text("a"), Expr("b"), Text("c")]` |
| T-S23 | empty interpolation | `"${}"` | → parse error in the sub-lex, not a lex error |
| T-S24 | nested interpolation | `"${ "${x}" }"` | `Interp[Expr("\"${x}\"")]` |
| T-S25 | string-aware `${}` body | `"${d["}"]}"` | `Interp[Expr("d[\"}\"]")]` — the RULING S7 case |
| T-S26 | escapes are not processed in `${}` | `"a\\n${b\\nc}d"` | the sub-lex sees `b\nc` as two statements' worth of source, i.e. the `\n` is *not* a newline in the sub-stream |
| T-S27 | `$` not followed by `{` | `"$x $$"` | `STR("$x $$")` |
| T-S28 | unterminated interpolation | `"${a"` | → `err(UnterminatedInterpolation)`, span on `${` |
| T-S29 | interpolation nesting limit | 9-deep nesting | → `err(NestingTooDeep)` at depth 9 |
| T-S30 | multi-byte literal | `"█▀"` | `STR` of 2 chars, `len() == 2` — the `18_lang_features.z:70-73` case |

### 11.5 Newline and statement separation

Both directions, as `tests/cases/18_lang_features.z` does:

| # | Test | Input | Expected |
|---|---|---|---|
| T-C1 | plain terminator | `a\nb` | `ID(a) NL ID(b)` |
| T-C2 | blank lines collapse | `a\n\n\nb` | `ID(a) NL ID(b)` |
| T-C3 | leading blanks dropped | `\n\na` | `ID(a)` — rule A4 |
| T-C4 | **positive**: trailing `+` | `a = "a" +\n"b"` | one statement, `STR("a") STR("b")` |
| T-C5 | **positive**: trailing `-` | `a = 1 -\n2 +\n3` | one statement `INT(1) Minus INT(2) Plus INT(3)` |
| T-C6 | **negative**: a trailing operator does not swallow the next statement | `a = 5\n-3` | `ID(a) Assign INT(5) NL Minus INT(3)` — the `18_lang_features.z:52-55` case |
| T-C7 | **negative**: same after a call | `f(1)\n-2` | `ID(f) LParen INT(1) RParen NL Minus INT(2)` |
| T-C8 | inside `(` | `max(10,\nmax(2,\n7))` | one statement, no `NL` |
| T-C9 | inside `[` | `[\n1,\n2,\n]` | one statement, no `NL` |
| T-C10 | inside `#{` | `#{\n"a":1,\n}` | one statement, no `NL` — ASI-0 |
| T-C11 | inside `%{` | `%{\n1,\n2\n}` | one statement, no `NL` |
| T-C12 | inside `#(` | `#(\n1,\n2\n)` | one statement, no `NL` |
| T-C13 | `LBrace` suppresses its own newline | `if c {\na\n}` | `KwIf ID(c) LBrace ID(a) NL RBrace` |
| T-C14 | `RBrace` terminates | `f() { }\ng() { }` | two `KwFn` statements separated by `NL` — ASI-1 |
| T-C15 | `else` on the next line | `if c { a }\nelse { b }` | `… RBrace NL KwElse LBrace …` (the parser eats the `NL`) |
| T-C16 | `:` continues | `case 1:\n"one"` | no `NL` after `Colon` |
| T-C17 | `Dot` continues when it *precedes* the newline; a leading `.` does not | `0..\n10` · `x = 5\n.real` | `INT DotDot INT NL` · `ID Assign INT NL Dot ID NL` |
| T-C18 | `,` continues | `f(a,\nb)` | one statement |
| T-C19 | comment does not disturb A2 | `a = 1 +\n/* c */\n2` | one statement |
| T-C20 | `;` then newline collapse | `a = 1;\nb` | `ID(a) Assign INT(1) Semi ID(b)` |
| T-C21 | `;;` collapses | `a = 1;;b` | `… Semi ID(b)` |
| T-C22 | `return` + newline is an empty return | `return\nx` | `KwReturn NL ID(x)` |
| T-C23 | `throw` + newline | `throw\nx` | `KwThrow NL ID(x)` |
| T-C24 | block comment eats its newlines | `/* a\nb */\nx` | `ID(x)` |
| T-C25 | `match` arms split on newlines | `match v {\n1: "a",\n_ => "b"\n}` | `NL` between the arms, none inside them |

### 11.6 Errors, recovery, and totality

| # | Test | Input | Expected |
|---|---|---|---|
| T-E1 | illegal character | `a § b` | → `err(UnexpectedCharacter{'§'})`, one error, three tokens recovered |
| T-E2 | unterminated block comment | `/* x` | → `err(UnterminatedBlockComment)`; `= help: add */ ` preserved from `:738-740` |
| T-E3 | nested-looking block comment | `/* /* */ */` | comment, then a parse-time stray `*/` — RULING H2 |
| T-E4 | error count is bounded by lines | 100 bad lines | ≤ 32 errors, then `TooManyErrors{32}` |
| T-E5 | missing quote is one error per line | 50 lines each with an unterminated `"` | exactly 50 errors (below the cap of 64), one per line, and the lexer reaches EOF |
| T-E6 | delimiter depth guard | 300 `(` | → `err(NestingTooDeep{256})` |
| T-E7 | lone continuation | `\` alone on a line | → `err(StrayLineContinuation)` |
| T-E8 | trailing continuation at EOF | `"abc\` | → `err(TrailingLineContinuation)` |
| T-E9 | **no panic on the corpus** | all 84,490 bytes of `tests/cases/*.z` | `lex()` returns; errors are recorded, never panicked |
| T-E10 | **no panic on the stdlib** | all 139,681 bytes of `std/*.z` | same |
| T-E11 | **no panic on random bytes** | 10,000 `proptest` arbitrary byte strings | returns or records errors; never panics, never hangs |
| T-E12 | **every span is in range** | all of T-E9/E10/E11 | `debug_assert!(lo <= hi <= src.len())` holds for every token |
| T-E13 | no trailing synthetic `Newline` | `a = 1` | last two tokens are `Int(1)`, `Eof` |
| T-E14 | golden dump stability | all of `tests/cases/*.z` | `zentok --color=never` output matches the committed golden files byte-for-byte |

### 11.7 Property tests

| # | Property |
|---|---|
| T-P1 | The concatenation of `[lo, hi)` over all tokens is a subsequence of the source, in order, non-overlapping, and strictly increasing in `lo`. |
| T-P2 | The stream always ends with exactly one `Eof` whose `lo == hi == src.len()`. |
| T-P3 | `sum(spans of tokens)` + whitespace + comments == `src.len()` — i.e. the lexer loses no bytes. |
| T-P4 | Lexing is deterministic: `lex(s) == lex(s)`, comparing the full `Lexed` for payload equality too, not just kinds. |
| T-P5 | Lexing a prefix never errors *later* than lexing the whole string: `errors(lex(s))[..i] == errors(lex(s[..n]))[..i]`. This is the REPL property, and it is the reason `Lexer` is incremental. |
| T-P6 | For every round-trippable literal, the cooked payload re-cooks to itself: `cook(cook(s)) == cook(s)`. |

---

## 12. Performance

### 12.1 Budget

**Target: ≤ 3.0 ns/byte** on the 32-file legacy corpus, release build, `criterion`.

| Quantity | Value | Derivation |
|---|---|---|
| Corpus | 84,490 bytes (`tests/cases/*.z`, 2,511 lines) | measured |
| Lex budget at 3.0 ns/byte | **254 µs** | |
| Lex budget for `std/*.z` | 419 µs at 139,681 bytes | `zenn build std` |
| Share of `bench_startup` budget | ≤ 2% of 15 ms | CPython reaches `main` in ~15 ms; review §4 flags our startup as the risk, so the front end must be a rounding error in it |
| Tokens produced | ~22,000 | 0.26 tokens/byte |
| Token stream size | ~264 KB | 12 bytes × 22,000 |
| Token allocation | 1 per file | `Vec::with_capacity` |

3.0 ns/byte is roughly 330 MB/s single-threaded. That is an ordinary hand-written-lexer number and
it is deliberately *not* ambitious: the lexer runs once per compile and the VM runs forever, so a
lexer that is 2× faster buys nothing the user can feel, while a lexer that is 2× slower costs
nothing they can feel either. The number is a regression tripwire, not a goal.

### 12.2 How to measure

```rust
// crates/zen-syntax/benches/lex.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_lex(c: &mut Criterion) {
    let mut g = c.benchmark_group("lex");
    for f in corpus() {                       // every tests/cases/*.z
        let src = std::fs::read_to_string(&f).unwrap();
        g.throughput(Throughput::Bytes(src.len() as u64));
        g.bench_with_input(BenchmarkId::from_parameter(f.display().to_string()), &src, |b, s| {
            b.iter(|| black_box(lex(black_box(s.as_str()), FileId(0))));
        });
    }
    g.finish();
}
criterion_group!(benches, bench_lex);
criterion_main!(benches);
```

The three things that make the number trustworthy:

1. **`black_box` on both the input and the output.** Without it, LLVM elides the entire scan and
   reports 0.03 ns/byte, which is the single most common way a lexer benchmark lies.
2. **`Throughput::Bytes`.** Criterion then reports ns/byte directly, so the number in this document
   is reproducible from CI output.
3. **Per-file, not concatenated.** One huge concatenated file makes the token `Vec` grow
   repeatedly and the allocator noise dominate. Per-file matches how the front end is actually
   used.

A second benchmark, `lex_shapes`, isolates each scanner so a regression can be attributed:

| Shape | Bytes | Expected ns/byte | What it measures |
|---|---|---|---|
| `identifiers` | 8 kB of `abcdefgh ` | ≤ 1.5 | class table + keyword match |
| `numbers` | 8 kB of `1234567,` | ≤ 1.8 | `eat_digits` + shift-accumulate |
| `strings` | 8 kB of `"abcdefgh",` | ≤ 2.0 | escape-aware inner loop |
| `comments` | 8 kB of `# a comment line\n` | ≤ 0.4 | bulk skip |
| `operators` | 8 kB of `a + b + c +` | ≤ 2.5 | operator table + ASI check |
| `mixed` | `tests/cases/18_lang_features.z` | ≤ 3.0 | the realistic figure |
| `interp` | 8 kB of `"a${b}c",` | ≤ 4.0 | sub-lexing the `${}` body |

### 12.3 Where the time goes, and what would change it

Expected split on `mixed`, to be confirmed by `perf record` in the first week of M1:

| Component | Share | Reducible? |
|---|---|---|
| `class_of` dispatch + main loop | ~25% | no — this is the irreducible scan |
| identifier scan + keyword match | ~20% | marginally, with a perfect hash (M-note in §10.4) |
| number scan + conversion | ~10% | yes — the Eisel-Lemire path skips `str::parse` entirely |
| string scan | ~12% | no |
| comment skip | ~3% | no |
| token push + `Vec` growth | ~12% | yes — `with_capacity` tuning, M9 |
| `LineIndex` build | ~8% | no, and it only runs once per file |
| error paths | ~0% | they are `#[cold]`; the `Illegal` class goes to `lex_illegal` which most files never reach |
| branch mispredicts on the ASI check | ~10% | no |

### 12.4 Non-goals

- **The lexer is not incremental within a file.** Only across REPL lines (§10.2). A rescan of a
  4 KB file costs 12 µs, which is below the round-trip time of a terminal.
- **The lexer is not threaded per file.** Module graphs are small and the parallelism win is zero;
  it would complicate the `Span`/`FileId` story for no measurable gain.
- **There is no token-stream compression and no token peephole cache.** 264 KB per 84 KB of
  source is fine.


