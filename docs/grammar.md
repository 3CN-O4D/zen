# Grammar

This reference is generated from the **native Rust runtime** (`src/runtime.rs`),
which is the single source of truth for syntax. It replaces the older grammar
that described the retired Python implementation.

> **Note on binary versions:** the installed `/usr/bin/zen` (2.1.0) is an older
> build. Constructs marked below as 🆕 (and several fixes) are only present in
> the current source; rebuild with `cargo build --release` to use them.

## Lexical notes

- Comments: `//`, `#`, and block comments `/* ... */`.
- Strings:
- `"..."` template literals — `${expression}` interpolation inside double quotes.
  - Single-quoted `'...'` has no interpolation.
  - Triple-quoted `"""` and `'''` multiline.
  - Backtick template literals `` ` `` `...` `` — supported (per quickstart.md:135,684).
- Numbers: integer and float decimal literals only (`42`, `3.14`). No
  scientific (`1e10`), hex (`0x`), or binary (`0b`) literals.
- Statement separators: newlines, `;`, or both. Both are optional around `{ }`
  blocks.

## Keywords

The complete lexer keyword set is:

```
let const var global        declarations
print                print statement
function func def    function definition
lambda               anonymous function
if elif else         conditional
while for in break continue      loops
return               return value
class new extends inherit super   OOP
this                 instance reference (alias for self)
import from as include load native   modules
try catch except finally throw     errors
typeof is            type / equality checks
switch case default  switch statement
match when           match / guard expressions
and or not true false null         literals & logic
```

Anything else is an identifier (variable), including `self` (a plain
identifier conventionally used for the instance) and `error` (conventional
name in `catch` blocks).

## Comments about keywords

- `except` aliases `catch`; `inherit` aliases `extends`.
- `match`/`when` and the fixes below are 🆕 post-2.1.0 (see note above).
- There is no `raise` keyword (use `throw`) and no `assert` keyword
  (`assert` is a built-in function).
- `this` is a runtime alias for `self` and is only valid inside methods.

## EBNF Grammar

```ebnf
program          = statement*

statement        = let | const | var | global | assign | print | input
                 | if | for | while | function | native
                 | return | throw | break | continue
                 | import | from | include | load
                 | class | switch | match | when | try
                 | block | command_call | expression

(* Declarations *)

let              = "let" target "=" expression
const            = "const" target "=" expression          (* 🆕 now enforced on reassignment *)
var              = "var" target "=" expression
global           = "global" target "=" expression          (* alias for `var`; lexer maps both to Var *)
target           = name | pattern
pattern          = "[" pattern_item? ("," pattern_item)* ","? "]"
                 | "{" name ("," name)* ","? "}"
pattern_item = name            (* element name *)
              | "..." name           (* rest pattern: `...name` collects remaining elements *)

(* Assignment *)

assign           = lvalue ("=" | "+=" | "-=" | "*=" | "/=" | "%="
                         | "??=" | "&=" | "|=" | "^=" | "<<=" | ">>=") expression
                 | lvalue ("++" | "--")
lvalue           = name | name "." name | name "[" expression "]"
                 (* compound ops and ++/-- work for name targets, member targets (`o.n++`), and index targets (`l[1]++`)
                    index targets (`l[1]++`)

(* Simple statements *)

print            = "print" (expression ("," expression)*)?
                 | "print" "(" (expression ("," expression)*)? ([","] "sep" "=" str)? ([","] "end" "=" str)? ")"
input            = "input" expression?          (* command-style call *)
command_call     = name expression             (* e.g. `sleep 2`, `exit 1`, `assert cond` *)
                                                 (* requires the name to be a built-in function *)

(* Flow control *)

if               = "if" expression block
                  ("elif" expression block)* ("else" block)?
                 | "if" expression block ("else if" expression block)* ("else" block)?
tail             = ("elif" expression block tail | "else" block)?
for              = "for" name ("," name)* "in" iteration block   (* one or more loop variables *)
iteration        = expression         (* list of lists/dicts is unpacked into the variables *)
while            = "while" expression block
break            = "break"
continue         = "continue"
return           = "return" expression?

(* Functions *)

function         = ("function" | "func" | "def") name "(" params? ")" block
native           = "native" "function" name "(" params? ")"     (* declares a native fn name *)

(* Classes *)

class            = "class" name ("extends" | "inherit")? qualified_name? block
class_body       = member*
member           = "func" "init" "(" params ")" block        (* constructor *)
                 | "func" name "(" params ")" block          (* method *)
                 | "var" name ("=" expression)?
qualified_name   = name ("." name)*          (* e.g. errors.Error *)
super_call       = "super" "(" args? ")"     (* parent constructor call, in methods only *)
                 | "super" "." name "(" args? ")"   (* 🆕 parent method call, e.g. super.hi() *)

(* Modules *)

import           = "import" module_spec ("," module_spec)*
from             = "from" qualified_module "import"
                   ("*" | import_item ("," import_item)*)
include          = ("include" | "load") (string | name)
module_spec      = (name ("." name)* | string) ("as" name)?
import_item      = name ("as" name)?

(* Switch / match *)

switch           = "switch" expression "{" arm* "}"
arm              = "case" expression (":" statements | block)
                 | "default" (":" statements | block)
match            = "match" expression "{" match_arm* "}"
                 | "when" "{" match_arm* "}"        (* when has no subject *)
match_arm        = pattern ("if" expression)? (":" | "=>") (statements | block)
                 (* arms are separated by commas or newlines; "case" bodies
                    must use ":" or a block *)

(* Errors *)

try              = "try" block catch_clause* "finally"? block?
catch_clause     = ("catch" | "except") catch_type? catch_bind? block
catch_type       = qualified_name            (* `catch TypeError { }` *)
catch_bind       = "as" name | name          (* `catch e { }` binds the caught value:
                                               · `e` is a registered error class → typed catch, no binding
                                               · otherwise → binds the value *)

(* Blocks *)

block            = "{" statement* "}"

(* Lambda / anonymous functions *)

primary_lambda   = "lambda" params? (":" expression | block)
primary_func     = ("function" | "func") "(" params? ")" block  (* anonymous function expression *)
primary_arrow    = "(" params? ")" "=>" (expression | block)   (* 🆕 arrow function; params MUST be parenthesized *)

(* Expressions — precedence low to high:
      ->  ..   (ranges)
      ? :      (ternary, `cond ? a : b`)
      ??       (nullish coalescing)
      or  ||
      and  &&
      == != === !== is
      <  >  <=  >=   in
      |
      ^
      &
      <<  >>
      +  -
      *  /  %
      - ! not ~ typeof (unary)
      ** (right-assoc power)
      .name  ?.name  [index]  (call) *)

expression       = range ternary
ternary          = "?" expression ":" expression        (* `cond ? a : b` *)
                 | "if" range "else" expression        (* 🆕 `a if cond else b` *)
range            = nullish (("->" | "..") nullish)*       (* no `to`, `by`, `@` *)
nullish          = or_expr ("??" or_expr)*
or_expr          = and_expr ("or" | "||" and_expr)*
and_expr         = not_expr ("and" | "&&" not_expr)*
not_expr         = ("not" | "!") not_expr | comparison
comparison       = bit_or (comp_op bit_or)*
comp_op          = "==" | "!=" | "===" | "!==" | "<" | ">" | "<=" | ">=" | "in" | "is"
                 (* no `not in`, `is not`, or `&&` compound — `is` is plain equality *)
bit_or           = bit_xor ("|" bit_xor)*
bit_xor          = bit_and ("^" bit_and)*
bit_and          = shift ("&" shift)*
shift            = addition ("<<" | ">>" addition)*
addition         = term (("+" | "-") term)*
term             = unary (("*" | "/" | "%") unary)*
unary            = ("-" | "!" | "not" | "typeof" | "~") unary | power
power            = call ("**" unary)?          (* right-associative *)

call             = primary (
                     "(" args? ")" |
                     "." name |
                     "?." name |
                     "[" expression "]"
                   )*

primary          = number | string | bool | null | name
                 | "(" expression ")"
                 | list | dict
                 | primary_lambda | primary_func | primary_arrow
                 | "if" expression block tail_as_expr     (* if-as-expression *)
                 | "match" expression "{" match_arm* "}"
                 | "when" "{" match_arm* "}"
                 | "new" name "(" args? ")"
                 | super_call
tail_as_expr     = ("elif" expression block tail_as_expr | "else" block)?

list             = "[" list_element ("," list_element)* ","? "]"
list_element     = "..." expression           (* spread *)
                 | comprehension
                 | expression
comprehension    = expression "for" name "in" expression ("if" expression)?  (* 🆕 single `for`, optional `if`; nested `for` not supported *)

dict             = "{" dict_pair ("," dict_pair)* ","? "}"
dict_pair        = "..." expression           (* spread *)
                 | dict_key ":" expression
dict_key         = string | name              (* expression keys are NOT allowed *)

args             = arg ("," arg)*             (* no trailing comma, no "..." spread *)
arg              = expression | name "=" expression
                 (* named args pack into a trailing dict argument and do NOT
                    bind to parameter names *)

params           = name ("=" expression)?     (* default value *)
                   ("," name ("=" expression)?)*   (* no trailing comma *)
```

## Behavior notes

- `range`:
  - `->` builds an **inclusive** range and auto-descends (`5 -> 1` → `[5,4,3,2,1]`).
  - `..` builds an **exclusive** range (`1..3` → `[1, 2]`).
  - There are no `to`, `by`, or `@` range operators.
- Ternary comes in two forms, both supported: C-style `cond ? a : b` and
  Python-style `a if cond else b`.
- `is` is **plain equality** (`1 is "1"` → false, `[] is []` → true). There is
  no `is not` compound operator.
- `print` accepts either `print a` or `print(a, b, sep=" ", end="")`.
- Command-style calls (`name expression`) only work when `name` resolves to a
  built-in function (for example `sleep 2`, `exit 1`, `assert cond`).
- `const` is **enforced**: reassigning a constant raises an error at runtime
  (`const x = 1; x = 2` → "cannot assign to constant: x").
- `global` is an accepted alias for `var` — the lexer maps both to the same
  token. Writes to enclosing variables from inside functions work without it
  (closures share variables lexically).
- `catch` binding:
  - `catch {}` — catch everything, no variable.
  - `catch as e {}` — catch everything, bind to `e`.
  - `catch errors.TypeError {}` — typed catch, no variable.
  - `catch errors.TypeError as e {}` — typed catch with binding.
  - `catch e {}` — unknown bare names are treated as a catch-all **binding**
    (identical to `catch as e {}`); a bare name that matches a registered error
    class (e.g. `TypeError`) is a typed catch.
- `match` / `when` arm bodies may be a single expression or a `{ ... }` block.
  Arms can be separated by commas or newlines:
  ```
  var v = 3
  print(match v { 1: "one", 3: "three", _: "other" })
  print(match v { x if x > 5 => "big", _ => "small" })
  print(when { v > 5: "big", _: "small" })
  ```
- Chained comparisons are supported: `1 < x < 10` works as expected.
- List comprehensions are supported: `[expr for x in seq if cond]`. Only a
  single `for` clause is allowed — nested `for` clauses are not supported.
- Destructuring supports `let [a, b] = list`, `const [a, b] = list`,
  `let {x} = dict`, and `var a, b = 1, 2`. **Rest patterns** (`...rest`) are
  not supported.
- Index assignment `l[0] = 9` and dict assignment `d["k"] = 7` work; compound
  assignment (`l[0] += 1`) and `++`/`--` through indexes or members do not.
- Named arguments (`f(a=1)`) become a single trailing dict argument. They are
  only usable by functions that accept a dict parameter; they do **not** match
  parameter names.
- Anonymous classes as *expressions* are not supported — `class` is a
  statement only, and the name is required.