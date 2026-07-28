# Grammar

## EBNF Grammar

```ebnf
program      = statement*

statement    = let | const | assign | go | fill | click | wait
             | refresh | back | forward | shot | scroll
             | execute | download | print | input
             | if | for | while | function | return | break
             | continue | try | include | switch | with
             | class | block | expression

let          = "let" (name | destruct) "=" expression
const        = "const" (name | destruct) "=" expression
destruct     = "[" name ("," name)* "]"
            | "{" name ("," name)* "}"
assign       = lvalue ("=" | "+=" | "-=" | "*=" | "/=" | "%=" | "??=") expression
            | lvalue ("++" | "--")
go           = "go" expression
fill         = "fill" expression "with" expression
click        = "click" expression?
print        = "print" expression?
if           = "if" expression block elif_tail
elif_tail    = ("elif" expression block elif_tail | "else" block)?
for          = "for" name "in" expression block
while        = "while" expression block
switch       = "switch" expression "{" case* default? "}"
case         = "case" expression block
default      = "default" block
with         = "with" expression ("as" name)? block
function     = ("function" | "def") name "(" params? ")" block
return       = "return" expression?
throw        = "throw" expression?
assert       = "assert" expression ("," expression)?
break        = "break"
continue     = "continue"
try          = "try" block "catch" name? block ("finally" block)?
include      = "include" string
class        = "class" name? ("extends" expression)? block
block        = "{" statement* "}"

expression   = ternary
ternary      = range ("if" range "else" ternary)?
range        = nullish (("->" | ".." | "to") range ("by" | "@")? range)?
nullish      = or_expr ("??" or_expr)*
or_expr      = and_expr ("or" | "||" and_expr)*
and_expr     = not_expr ("and" | "&&" not_expr)*
not_expr     = "not" not_expr | comparison
comparison   = bitwise (comp_op bitwise)*
comp_op      = "==" | "!=" | "===" | "!==" | "<" | ">" | "<=" | ">="
             | "in" | "not" "in" | "is" | "is" "not"
bitwise      = shift ("&" shift)*
shift        = xor ("<<" | ">>" xor)*
xor          = pipe ("^" pipe)*
pipe         = addition ("|" addition)*
addition     = term (("+" | "-") term)*
term         = unary (("*" | "/" | "%") unary)*
unary        = ("-" | "!" | "not" | "typeof" | "~") unary | pow
pow          = call ("**" unary)?
call         = primary (
               "(" args? ")" |
               ("." | "?.") name |
               "[" expr "]" |
               "++" | "--"
             )*
primary      = number | string | backtick_string | bool | null
             | list | dict | name | "(" expression ")"
             | "function" "(" params? ")" block
             | "lambda" (name ("," name)*)? ":" expression
             | "(" params? ")" "=>" (expression | block)
             | "new" call
             | "class" name? ("extends" expression)? block

list         = "[" (list_element ("," list_element)* ","?)? "]"
list_element = "..." expression
            | expression "for" name "in" expression ("if" expression)?
backtick_string = "`" ( "${" expression "}" | characters )* "`"
dict         = "{" (dict_pair ("," dict_pair)* ","?)? "}"
dict_pair    = "..." expression | expression ":" expression
args         = (expression | name "=" expression)
             ("," (expression | name "=" expression))* ","?
params       = name ("=" expression)?
             ("," name ("=" expression)?)* ","?
```
