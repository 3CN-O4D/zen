;; Zen grammar for Tree-sitter
;; Compatible with tree-sitter CLI and Neovim

; Includes
(source_file: (statement)* @root)

; Statements
(statement) @statement

; Variables
(let_statement
  name: (identifier) @variable)

(const_statement
  name: (identifier) @variable)

(assignment
  target: (identifier) @variable)

; Keywords
"let" @keyword
"const" @keyword
"function" @keyword.function
"def" @keyword.function
"class" @keyword
"extends" @keyword
"new" @keyword
"lambda" @keyword.function
"if" @keyword.conditional
"elif" @keyword.conditional
"else" @keyword.conditional
"for" @keyword.repeat
"in" @keyword.repeat
"while" @keyword.repeat
"switch" @keyword
"case" @keyword
"default" @keyword
"break" @keyword
"continue" @keyword
"return" @keyword.return
"try" @keyword
"catch" @keyword
"finally" @keyword
"throw" @keyword
"raise" @keyword
"assert" @keyword
"with" @keyword
"as" @keyword
"load" @keyword
"use" @keyword
"include" @keyword
"import" @keyword
"require" @keyword
"typeof" @keyword

; Browser commands
"go" @function.builtin
"fill" @function.builtin
"click" @function.builtin
"wait" @function.builtin
"wait_for" @function.builtin
"shot" @function.builtin
"scroll" @function.builtin
"execute" @function.builtin
"download" @function.builtin
"refresh" @function.builtin
"back" @function.builtin
"forward" @function.builtin
"print" @function.builtin
"input" @function.builtin

; Constants
"true" @boolean
"false" @boolean
"null" @constant.builtin

; Special variables
"_url" @variable.builtin
"__url" @variable.builtin
"___url" @variable.builtin
"_time" @variable.builtin
"_date" @variable.builtin
"_dir" @variable.builtin
"_version" @variable.builtin
"_" @variable.builtin
"_timeout" @variable.builtin
"error" @variable.builtin
"self" @variable.builtin

; Comments
(comment) @comment
(block_comment) @comment

; Strings
(string) @string
(template_literal) @string

; Escape sequences
(escape_sequence) @string.escape

; Template interpolation
(template_interpolation) @embedded

; Numbers
(number) @number
(float) @number.float

; Operators
"=" @operator
"+=" @operator
"-=" @operator
"*=" @operator
"/=" @operator
"%=" @operator
"??=" @operator
"==" @operator
"!=" @operator
"===" @operator
 "!==" @operator
"<" @operator
">" @operator
"<=" @operator
">=" @operator
"+" @operator
"-" @operator
"*" @operator
"/" @operator
"%" @operator
"**" @operator
"&&" @operator
"||" @operator
"&" @operator
"|" @operator
"^" @operator
"~" @operator
"<<" @operator
">>" @operator
"?" @operator
"??" @operator
"?." @operator
"->" @operator
".." @operator
"=>" @operator
"..." @operator

; Logical operators
"and" @keyword.operator
"or" @keyword.operator
"not" @keyword.operator
"is" @keyword.operator
"in" @keyword.operator

; Punctuation
"(" @punctuation.bracket
")" @punctuation.bracket
"[" @punctuation.bracket
"]" @punctuation.bracket
"{" @punctuation.bracket
"}" @punctuation.bracket
"," @punctuation.delimiter
";" @punctuation.delimiter
":" @punctuation.delimiter
"." @punctuation.delimiter

; Function calls
(function_call
  name: (identifier) @function)

(method_call
  name: (identifier) @method)

; Class definition
(class_declaration
  name: (identifier) @type)

(class_declaration
  super: (identifier) @type)

; Function definitions
(function_declaration
  name: (identifier) @function)

(parameter
  name: (identifier) @parameter)

; Arguments
(arguments
  (identifier) @variable)

; Member access
(member_expression
  property: (identifier) @property)

; Subscript
(subscript_expression
  index: (identifier) @variable)
