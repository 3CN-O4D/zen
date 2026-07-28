" Zen syntax highlighting for Vim
" Language:    Zen
" Maintainer:  ecnord
" Filenames:   *.z

if exists("b:current_syntax")
  finish
endif

" Keywords
syn keyword zenKeyword let const function def class extends new lambda
syn keyword zenKeyword if elif else for in while switch case default
syn keyword zenKeyword break continue return try catch finally
syn keyword zenKeyword with as load use include import require
syn keyword zenKeyword throw raise assert typeof

" Browser commands
syn keyword zenBrowser go fill click wait wait_for wait_for_network
syn keyword zenBrowser shot scroll_to execute download refresh back forward
syn keyword zenBrowser set_user_agent set_headers user_agent headers
syn keyword zenBrowser page_html page_text page_links page_images
syn keyword zenBrowser page_forms page_inputs page_buttons

" Built-in functions
syn keyword zenBuiltin abs min max len type str int float bool round trunc
syn keyword zenBuiltin print input sleep random range interval enumerate zip
syn keyword zenBuiltin map filter reduce flatten unique chunk partition
syn keyword zenBuiltin read_file write_file append_file read_binary write_binary
syn keyword zenBuiltin file_exists list_dir mkdir remove_file path_join
syn keyword zenBuiltin basename dirname exec prompt confirm
syn keyword zenBuiltin json_parse json_encode csv_read csv_write csv_parse csv_encode
syn keyword zenBuiltin find find_all find_by_text find_by_url first search

" Constants
syn keyword zenConstant true false null

" Special variables
syn keyword zenSpecial _url __url ___url _time _date _dir _version
syn keyword zenSpecial _ _timeout error self

" Comments
syn match zenComment "//.*$" contains=@Spell
syn match zenComment "#.*$" contains=@Spell
syn region zenBlockComment start="/\*" end="\*/" contains=@Spell

" Strings
syn region zenString start='"' end='"' skip='\\"' contains=zenEscape,zenInterpolation
syn region zenString start="'" end="'" skip="\\'" contains=zenEscape
syn region zenString start='"""' end='"""' contains=zenEscape,zenInterpolation
syn region zenString start="'''" end="'''" contains=zenEscape

" Template literals
syn region zenTemplate start='`' end='`' contains=zenTemplateExpr,zenEscape
syn region zenTemplateExpr matchgroup=zenTemplateBrace start='\${' end='}' contains=TOP

" Escape sequences
syn match zenEscape contained /\\[ntr0'"\\]/
syn match zenEscape contained /\\x[0-9a-fA-F]\{2}/
syn match zenEscape contained /\\u[0-9a-fA-F]\{4}/
syn match zenEscape contained /\\U[0-9a-fA-F]\{8}/

" Interpolation in strings
syn region zenInterpolation matchgroup=zenInterpBrace start='{%?' end='}' contained contains=TOP

" Numbers
syn match zenNumber /\b\d\+\.\d\+\b/
syn match zenNumber /\b\d\+\b/

" Operators
syn match zenOperator /\.\.\./
syn match zenOperator /\.\./
syn match zenOperator /->/
syn match zenOperator /=>/
syn match zenOperator /\*\*/
syn match zenOperator /++/
syn match zenOperator /--/
syn match zenOperator /+=\|-=\|\/=\|\*=\|%=/
syn match zenOperator /??=\|??/
syn match zenOperator /===\|!==\|==\|!=/
syn match zenOperator /<=\|>=/
syn match zenOperator /&&\|||/
syn match zenOperator /[&|^~]/
syn match zenOperator /<</
syn match zenOperator />>/
syn match zenOperator /\.\?/

" Type names
syn match zenType /\bZEN_[A-Z][a-zA-Z]*\b/

" Function calls
syn match zenFuncCall /\b\w\+\ze\s*(/

" Class instantiation
syn match zenNew /\bnew\s\+\w\+/

" Boolean operators
syn keyword zenBooleanOperator and or not is in

" Ranges
syn match zenRange /->\|to\|\.\./

" Block delimiters
syn match zenBrace /[{}]/
syn match zenParen /[()]/
syn match zenBracket /[\[\]]/

" Highlight links
hi def link zenKeyword       Keyword
hi def link zenBrowser       Function
hi def link zenBuiltin       Function
hi def link zenConstant      Constant
hi def link zenSpecial       Special
hi def link zenComment       Comment
hi def link zenBlockComment  Comment
hi def link zenString        String
hi def link zenTemplate      String
hi def link zenTemplateBrace Special
hi def link zenInterpBrace   Special
hi def link zenEscape        Special
hi def link zenNumber        Number
hi def link zenOperator      Operator
hi def link zenType          Type
hi def link zenFuncCall      Function
hi def link zenNew           Keyword
hi def link zenBooleanOperator Operator
hi def link zenRange         Operator
hi def link zenBrace         Delimiter
hi def link zenParen         Delimiter
hi def link zenBracket       Delimiter

let b:current_syntax = "zen"
