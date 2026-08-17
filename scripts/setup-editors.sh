#!/usr/bin/env bash
# Zen language editor setup script
# Detects all installed editors on the system and configures zen syntax
# highlighting, file type detection, and other IDE features.
#
# Works on: Linux, macOS, Windows (Git Bash/WSL), FreeBSD, Termux (Android)
#
# Supported editors:
#   - Vim / Neovim
#   - VS Code / VSCodium / Code-OSS
#   - Helix
#   - Sublime Text
#   - Emacs
#   - Nano
#   - Micro
#   - Kate / KWrite
#   - Gedit / GNOME Text Editor
#   - Notepadqq
#
# Usage:
#   ./scripts/setup-editors.sh              # Auto-detect and configure all
#   ./scripts/setup-editors.sh --vim        # Configure Vim/Neovim only
#   ./scripts/setup-editors.sh --all        # Force configure all (even if not detected)
#   ./scripts/setup-editors.sh --list       # List detected editors
#
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"

# ── Colors ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

log()    { echo -e "${BLUE}==>${NC} $*"; }
ok()     { echo -e "${GREEN}  ✓${NC} $*"; }
warn()   { echo -e "${YELLOW}  !${NC} $*"; }
err()    { echo -e "${RED}  ✗${NC} $*" >&2; }
header() { echo -e "\n${BOLD}── $* ──${NC}"; }

# ── OS detection ─────────────────────────────────────────────────────────────
OS="$(uname -s)"
IS_TERMUX=0
[ -n "${PREFIX:-}" ] && [ "$(uname -o 2>/dev/null)" = "Android" ] && IS_TERMUX=1

config_count=0
skip_count=0

# ── Helpers ──────────────────────────────────────────────────────────────────
has_cmd() { command -v "$1" >/dev/null 2>&1; }

config_file_exists() {
    [ -f "$1" ] && grep -q "zen" "$1" 2>/dev/null
}

# ── Vim / Neovim ────────────────────────────────────────────────────────────
setup_vim() {
    local vim_conf=""
    local nvim_conf=""

    # Detect vim config locations
    if [ -d "${HOME}/.vim" ] || has_cmd vim; then
        vim_conf="${HOME}/.vim"
    fi
    if [ -d "${HOME}/.config/nvim" ] || has_cmd nvim; then
        nvim_conf="${HOME}/.config/nvim"
    fi

    if [ -z "$vim_conf" ] && [ -z "$nvim_conf" ]; then
        warn "Vim/Neovim not found, skipping"
        return
    fi

    # Vim syntax file
    if [ -n "$vim_conf" ]; then
        mkdir -p "${vim_conf}/syntax"
        mkdir -p "${vim_conf}/ftdetect"

        cp "${ROOT}/editors/vim/syntax/zen.vim" "${vim_conf}/syntax/zen.vim"

        # Filetype detection
        if [ ! -f "${vim_conf}/ftdetect/zen.vim" ] || ! grep -q "zen" "${vim_conf}/ftdetect/zen.vim" 2>/dev/null; then
            cat > "${vim_conf}/ftdetect/zen.vim" << 'VIMFT'
autocmd BufNewFile,BufRead *.z setfiletype zen
VIMFT
        fi
        ok "Vim syntax: ${vim_conf}/syntax/zen.vim"
        ok "Vim ftdetect: ${vim_conf}/ftdetect/zen.vim"
        config_count=$((config_count + 1))
    fi

    # Neovim (uses same syntax files via runtimepath, but also copies explicitly)
    if [ -n "$nvim_conf" ]; then
        mkdir -p "${nvim_conf}/syntax"
        mkdir -p "${nvim_conf}/ftdetect"
        cp "${ROOT}/editors/vim/syntax/zen.vim" "${nvim_conf}/syntax/zen.vim"

        if [ ! -f "${nvim_conf}/ftdetect/zen.vim" ] || ! grep -q "zen" "${nvim_conf}/ftdetect/zen.vim" 2>/dev/null; then
            cat > "${nvim_conf}/ftdetect/zen.vim" << 'NVIMFT'
autocmd BufNewFile,BufRead *.z setfiletype zen
NVIMFT
        fi
        ok "Neovim syntax: ${nvim_conf}/syntax/zen.vim"
        ok "Neovim ftdetect: ${nvim_conf}/ftdetect/zen.vim"
        config_count=$((config_count + 1))
    fi

    # Neovim treesitter hint
    if has_cmd nvim; then
        log "  Tip: For Treesitter highlighting in Neovim, install the zen parser:"
        log "    https://github.com/3CN-O4D/tree-sitter-zen"
    fi
}

# ── VS Code / VSCodium / Code-OSS ──────────────────────────────────────────
setup_vscode() {
    local vscode_dirs=()

    # Detect all VS Code variants
    for dir in \
        "${HOME}/.vscode/extensions" \
        "${HOME}/.vscode-insiders/extensions" \
        "${HOME}/.vscode-oss/extensions" \
        "${HOME}/.codium/extensions" \
        "${HOME}/.vscode-server/extensions" \
        "/usr/share/code/resources/app/extensions" \
        "${HOME}/Library/Application Support/Code/User/extensions" \
        "${HOME}/.vscode/extensions"
    do
        if [ -d "$dir" ]; then
            vscode_dirs+=("$dir")
        fi
    done

    if [ ${#vscode_dirs[@]} -eq 0 ]; then
        warn "VS Code not found, skipping"
        return
    fi

    # Check if we already have a zen extension
    for dir in "${vscode_dirs[@]}"; do
        if ls "${dir}/zen-"* 2>/dev/null | head -1 >/dev/null 2>&1; then
            ok "VS Code zen extension already installed in ${dir}"
            config_count=$((config_count + 1))
            return
        fi
    done

    # Install extension to first detected directory
    local target_dir="${vscode_dirs[0]}"
    local ext_dir="${target_dir}/zen-language-0.1.0"
    mkdir -p "${ext_dir}/syntaxes"

    # Create VS Code extension files
    cat > "${ext_dir}/package.json" << 'VSCODEPKG'
{
  "name": "zen-language",
  "displayName": "Zen Language Support",
  "description": "Syntax highlighting and language support for the Zen programming language (.z files)",
  "version": "0.1.0",
  "publisher": "3CN-O4D",
  "engines": { "vscode": "^1.60.0" },
  "categories": ["Programming Languages"],
  "contributes": {
    "languages": [{
      "id": "zen",
      "aliases": ["Zen", "zen"],
      "extensions": [".z"],
      "configuration": "./language-configuration.json"
    }],
    "grammars": [{
      "language": "zen",
      "scopeName": "source.zen",
      "path": "./syntaxes/zen.tmLanguage.json"
    }]
  }
}
VSCODEPKG

    # Convert Sublime Text syntax to TextMate format for VS Code
    cat > "${ext_dir}/syntaxes/zen.tmLanguage.json" << 'VSCODETM'
{
  "scopeName": "source.zen",
  "name": "Zen",
  "patterns": [
    { "include": "#comments" },
    { "match": "\\b(let|const|function|func|def|class|extends|new|lambda|if|elif|else|for|in|while|switch|case|default|break|continue|return|try|catch|finally|with|as|load|use|include|import|require|throw|raise|assert|typeof|super|native|and|or|not|is)\\b", "name": "keyword.control.zen" },
    { "match": "\\b(true|false|null)\\b", "name": "constant.language.zen" },
    { "match": "\\b(self|error|_url|__url|___url|_time|_date|_dir|_version|_timeout|_)\\b", "name": "variable.language.zen" },
    { "match": "\\b(abs|min|max|len|type|str|int|float|bool|round|trunc|print|input|sleep|random|range|interval|enumerate|zip|map|filter|reduce|flatten|unique|chunk|partition|read_file|write_file|append_file|read_binary|write_binary|file_exists|list_dir|mkdir|remove_file|path_join|basename|dirname|exec|prompt|confirm|json_parse|json_encode|csv_read|csv_write|csv_parse|csv_encode|find|find_all|find_by_text|find_by_url|first|search)\\b", "name": "support.function.builtin.zen" },
    { "match": "\\b(go|fill|click|wait|wait_for|wait_for_network|shot|scroll|scroll_to|execute|download|refresh|back|forward|set_user_agent|set_headers|user_agent|headers|page_html|page_text|page_links|page_images|page_forms|page_inputs|page_buttons)\\b", "name": "support.function.browser.zen" },
    { "match": "0[xX][0-9a-fA-F]+|0[bB][01]+|0[oO][0-7]+|\\b\\d+(\\.\\d+)?([eE][+-]?\\d+)?\\b", "name": "constant.numeric.zen" },
    { "include": "#strings" },
    { "include": "#template-literals" },
    { "match": "(===|!==|==|!=|<=|>=|\\?\\?|\\.\\.|\\.\\.\\.|->|=>|\\+=|-=|\\*=|/=|%=|&&|\\|\\||<<|>>|[&|^~])", "name": "keyword.operator.zen" },
    { "match": "([a-zA-Z_]\\w*)(?=\\s*\\()", "name": "entity.name.function.zen" }
  ],
  "repository": {
    "comments": {
      "patterns": [
        { "match": "//.*$", "name": "comment.line.double-slash.zen" },
        { "match": "#.*$", "name": "comment.line.number-sign.zen" },
        { "begin": "/\\*", "end": "\\*/", "name": "comment.block.zen" }
      ]
    },
    "strings": {
      "patterns": [
        { "begin": "\"\"\"", "end": "\"\"\"", "name": "string.quoted.triple.zen", "patterns": [{ "match": "\\\\(?:[ntr0\"'\\\\]|x[0-9a-fA-F]{2}|u[0-9a-fA-F]{4})", "name": "constant.character.escape.zen" }] },
        { "begin": "'''", "end": "'''", "name": "string.quoted.triple.zen" },
        { "begin": "\"", "end": "\"", "name": "string.quoted.double.zen", "patterns": [{ "match": "\\\\(?:[ntr0\"'\\\\]|x[0-9a-fA-F]{2}|u[0-9a-fA-F]{4})", "name": "constant.character.escape.zen" }, { "begin": "\\{", "end": "\\}", "name": "meta.interpolation.zen", "patterns": [{ "include": "source.zen" }] }] },
        { "begin": "'", "end": "'", "name": "string.quoted.single.zen", "patterns": [{ "match": "\\\\(?:[ntr0\"'\\\\]|x[0-9a-fA-F]{2}|u[0-9a-fA-F]{4})", "name": "constant.character.escape.zen" }] }
      ]
    },
    "template-literals": {
      "patterns": [
        { "begin": "`", "end": "`", "name": "string.template.zen", "patterns": [{ "match": "\\\\(?:[ntr0\"'\\\\]|x[0-9a-fA-F]{2}|u[0-9a-fA-F]{4})", "name": "constant.character.escape.zen" }, { "begin": "\\$\\{", "end": "\\}", "name": "meta.interpolation.template.zen", "patterns": [{ "include": "source.zen" }] }] }
      ]
    }
  }
}
VSCODETM

    cat > "${ext_dir}/language-configuration.json" << 'VSCODELC'
{
  "comments": { "lineComment": "//", "blockComment": ["/*", "*/"] },
  "brackets": [["(", ")"], ["[", "]"], ["{", "}"]],
  "autoClosingPairs": [
    { "open": "(", "close": ")" },
    { "open": "[", "close": "]" },
    { "open": "{", "close": "}" },
    { "open": "\"", "close": "\"" },
    { "open": "'", "close": "'" },
    { "open": "`", "close": "`" }
  ],
  "surroundingPairs": [
    { "open": "(", "close": ")" },
    { "open": "[", "close": "]" },
    { "open": "{", "close": "}" },
    { "open": "\"", "close": "\"" },
    { "open": "'", "close": "'" },
    { "open": "`", "close": "`" }
  ],
  "folding": {
    "markers": {
      "start": "^\\s*//\\s*#?region\\b",
      "end": "^\\s*//\\s*#?endregion\\b"
    }
  },
  "indentationRules": {
    "increaseIndentPattern": "^\\s*(.*\\{[^}\"']*$|.*\\([^)\"']*$)",
    "decreaseIndentPattern": "^\\s*(\\}|\\))"
  },
  "wordPattern": "(-?\\d*\\.\\d\\w*)|([^\\`\\~\\!\\@\\#\\%\\^\\&\\*\\(\\)\\=\\+\\{\\}\\\\\\|\\;\\:\\'\\\"\\,\\.\\<\\>\\/\\?\\s]+)"
}
VSCODELC

    ok "VS Code extension: ${ext_dir}"
    log "  Restart VS Code to activate Zen syntax highlighting"
    log "  Or install the .vsix: cd ${ext_dir} && vsce package"
    config_count=$((config_count + 1))
}

# ── Helix ────────────────────────────────────────────────────────────────────
setup_helix() {
    local helix_conf=""
    for dir in \
        "${HOME}/.config/helix" \
        "${HOME}/Library/Application Support/Helix" \
        "${APPDATA:-}/helix"
    do
        if [ -d "$dir" ] || has_cmd hx; then
            helix_conf="$dir"
            break
        fi
    done

    if [ -z "$helix_conf" ]; then
        warn "Helix not found, skipping"
        return
    fi

    mkdir -p "$helix_conf"
    local lang_file="${helix_conf}/languages.toml"

    if [ -f "$lang_file" ] && grep -q 'name = "zen"' "$lang_file" 2>/dev/null; then
        ok "Helix already configured: ${lang_file}"
        config_count=$((config_count + 1))
        return
    fi

    # Append zen config to languages.toml
    if [ -f "$lang_file" ]; then
        echo "" >> "$lang_file"
    fi

    cat >> "$lang_file" << 'HELIXLANG'

# Zen language support
[[language]]
name = "zen"
language-servers = []
autoclose = true
comment-tokens = ["//", "#"]
block-comment-tokens = { start = "/*", end = "*/" }
grammar = "zen"

[[language.auto-pairs]]
open = "("
close = ")"

[[language.auto-pairs]]
open = "["
close = "]"

[[language.auto-pairs]]
open = "{"
close = "}"

[[language.auto-pairs]]
open = "\""
close = "\""

[[language.auto-pairs]]
open = "'"
close = "'"

[[language.auto-pairs]]
open = "`"
close = "`"

[[grammar]]
name = "zen"
source = "git+https://github.com/3CN-O4D/tree-sitter-zen"
HELIXLANG

    ok "Helix: ${lang_file}"
    log "  Run 'hx --grammar fetch' then 'hx --grammar build' to compile the parser"
    config_count=$((config_count + 1))
}

# ── Sublime Text ─────────────────────────────────────────────────────────────
setup_sublime() {
    local sublime_dirs=()

    for dir in \
        "${HOME}/.config/sublime-text/Packages/User" \
        "${HOME}/Library/Application Support/Sublime Text/Packages/User" \
        "${APPDATA:-}/Sublime Text/Packages/User" \
        "${HOME}/.config/sublime-text-dev/Packages/User" \
        "${HOME}/.config/sublime-merge/Packages/User"
    do
        if [ -d "$dir" ] || [ -d "$(dirname "$dir")" ]; then
            sublime_dirs+=("$dir")
        fi
    done

    if [ ${#sublime_dirs[@]} -eq 0 ]; then
        # Check if subl is installed even if config dir doesn't exist yet
        if has_cmd subl; then
            sublime_dirs=("${HOME}/.config/sublime-text/Packages/User")
        else
            warn "Sublime Text not found, skipping"
            return
        fi
    fi

    for dir in "${sublime_dirs[@]}"; do
        mkdir -p "$dir"
        if [ ! -f "${dir}/zen.sublime-syntax" ] || ! diff -q "${ROOT}/editors/sublime-text/zen.sublime-syntax" "${dir}/zen.sublime-syntax" >/dev/null 2>&1; then
            cp "${ROOT}/editors/sublime-text/zen.sublime-syntax" "${dir}/zen.sublime-syntax"
        fi
        ok "Sublime Text: ${dir}/zen.sublime-syntax"
        config_count=$((config_count + 1))
    done
}

# ── Emacs ────────────────────────────────────────────────────────────────────
setup_emacs() {
    if ! has_cmd emacs; then
        warn "Emacs not found, skipping"
        return
    fi

    local emacs_dir="${HOME}/.emacs.d"
    [ -d "${HOME}/.config/emacs" ] && emacs_dir="${HOME}/.config/emacs"

    mkdir -p "${emacs_dir}/lisp"

    cat > "${emacs_dir}/lisp/zen-mode.el" << 'EMACSEL'
;;; zen-mode.el --- Major mode for the Zen programming language -*- lexical-binding: t; -*-

;;; Commentary:
;; Provides syntax highlighting and basic editing support for Zen (.z) files.

;;; Code:

(defvar zen-mode-syntax-table
  (let ((st (make-syntax-table)))
    (c-add-style "zen" '(
      (c-basic-offset . 4)
      (c-offsets-alist
       (case-label . +)
       (defun-block-intro . +)
       (substatement-open . 0))))
    ;; Comments
    (modify-syntax-entry ?/ "< 12" st)
    (modify-syntax-entry ?* ". 23" st)
    (modify-syntax-entry ?\n "> a" st)
    ;; Strings
    (modify-syntax-entry ?\" "\"" st)
    (modify-syntax-entry ?\\ "\\" st)
    st)
  "Zen mode syntax table.")

(defconst zen-mode-keywords
  '("let" "const" "function" "func" "def" "class" "extends" "new"
    "lambda" "if" "elif" "else" "for" "in" "while" "switch" "case"
    "default" "break" "continue" "return" "try" "catch" "finally"
    "with" "as" "load" "use" "include" "import" "require" "throw"
    "raise" "assert" "typeof" "super" "native" "and" "or" "not"
    "is" "true" "false" "null")
  "Zen language keywords.")

(defconst zen-mode-builtins
  '("abs" "min" "max" "len" "type" "str" "int" "float" "bool"
    "round" "trunc" "print" "input" "sleep" "random" "range"
    "enumerate" "zip" "map" "filter" "reduce" "flatten" "unique"
    "read_file" "write_file" "file_exists" "list_dir" "mkdir"
    "exec" "json_parse" "json_encode" "csv_read" "csv_write")
  "Zen built-in functions.")

(defvar zen-mode-font-lock-keywords
  `(
    (,(regexp-opt zen-mode-keywords 'words) . font-lock-keyword-face)
    (,(regexp-opt zen-mode-builtins 'words) . font-lock-builtin-face)
    ("\\b\\(true\\|false\\|null\\)\\b" . font-lock-constant-face)
    ("\\b\\(self\\|error\\|_url\\|_time\\|_date\\|_dir\\|_version\\)\\b" . font-lock-variable-name-face)
    ("\\(function\\|def\\|lambda\\)\\s-+\\(\\w+\\)" . ((1 . font-lock-keyword-face) (2 . font-lock-function-name-face)))
    ("\\b[0-9]+\\.?[0-9]*\\b" . font-lock-constant-face)
    ("\\(//\\|#\\).*" . font-lock-comment-face)
    )
  "Font-lock keywords for Zen mode.")

;;;###autoload
(define-derived-mode zen-mode prog-mode "Zen"
  "Major mode for editing Zen language files."
  :syntax-table zen-mode-syntax-table
  (setq font-lock-defaults '(zen-mode-font-lock-keywords))
  (setq indent-tabs-mode nil)
  (setq indent-line-function 'c-indent-line))

;;;###autoload
(add-to-list 'auto-mode-alist '("\\.z\\'" . zen-mode))

(provide 'zen-mode)
;;; zen-mode.el ends here
EMACSEL

    # Check if init.el or init.el exists
    local init_file=""
    for f in "${emacs_dir}/init.el" "${emacs_dir}/init.el"; do
        [ -f "$f" ] && init_file="$f" && break
    done

    if [ -n "$init_file" ]; then
        if ! grep -q "zen-mode" "$init_file" 2>/dev/null; then
            echo "" >> "$init_file"
            echo ";; Zen language support" >> "$init_file"
            echo "(add-to-list 'load-path \"${emacs_dir}/lisp\")" >> "$init_file"
            echo "(require 'zen-mode)" >> "$init_file"
            ok "Emacs: ${emacs_dir}/lisp/zen-mode.el (added to init)"
        else
            ok "Emacs: zen-mode already configured"
        fi
    else
        ok "Emacs: ${emacs_dir}/lisp/zen-mode.el"
        log "  Add to your init.el:"
        log "    (add-to-list 'load-path \"${emacs_dir}/lisp\")"
        log "    (require 'zen-mode)"
    fi
    config_count=$((config_count + 1))
}

# ── Nano ─────────────────────────────────────────────────────────────────────
setup_nano() {
    if ! has_cmd nano && [ ! -d "/usr/share/nano" ]; then
        warn "Nano not found, skipping"
        return
    fi

    local nano_dir="${HOME}/.nano"
    mkdir -p "$nano_dir"

    cat > "${nano_dir}/zen.nanorc" << 'NANO'
# Zen language syntax highlighting for Nano

# Keywords
color brightblue "\b(let|const|function|func|def|class|extends|new|lambda|if|elif|else|for|in|while|switch|case|default|break|continue|return|try|catch|finally|with|as|load|use|include|import|require|throw|raise|assert|typeof|super|native|and|or|not|is)\b"

# Constants
color brightcyan "\b(true|false|null)\b"

# Built-in functions
color green "\b(abs|min|max|len|type|str|int|float|bool|round|trunc|print|input|sleep|random|range|enumerate|zip|map|filter|reduce|flatten|unique|read_file|write_file|file_exists|list_dir|mkdir|exec|json_parse|json_encode|csv_read|csv_write)\b"

# Special variables
color magenta "\b(self|error|_url|_time|_date|_dir|_version|_timeout|_)\b"

# Numbers
color cyan "\b[0-9]+\.?[0-9]*\b"

# Strings
color yellow "\"[^"]*\""
color yellow "'[^']*'"

# Template literals
color yellow "`[^`]*`"

# Comments
color brightblack "//.*$"
color brightblack "#.*$"
color brightblack "/\*([^*]|\*[^/])*\*/"

# Operators
color brightred "(===|!==|==|!=|<=|>=|\.\.\.|->|\.\.|=>|\+=|-=|\*=|/=|%=|\?\?=|\?\?|&&|\|\||<<|>>|[&|^~])"

# Block delimiters
color brightwhite "[{}]"
color brightwhite "[()]"
color brightwhite "[\[\]]"
NANO

    ok "Nano: ${nano_dir}/zen.nanorc"
    log "  Add to your ~/.nanorc:"
    log "    include \"~/.nano/zen.nanorc\""
    config_count=$((config_count + 1))
}

# ── Micro ────────────────────────────────────────────────────────────────────
setup_micro() {
    if ! has_cmd micro; then
        warn "Micro not found, skipping"
        return
    fi

    local micro_dir="${HOME}/.config/micro/syntax"
    mkdir -p "$micro_dir"

    cat > "${micro_dir}/zen.yaml" << 'MICRO'
filetype: zen
detect:
    filename: "\\.z$"
rules:
    - type: "\\b(let|const|function|func|def|class|extends|new|lambda|if|elif|else|for|in|while|switch|case|default|break|continue|return|try|catch|finally|with|as|load|use|include|import|require|throw|raise|assert|typeof|super|native|and|or|not|is)\\b"
    - constant.bool: "\\b(true|false|null)\\b"
    - identifier: "\\b(self|error|_url|_time|_date|_dir|_version|_timeout|_)\\b"
    - identifier.builtin: "\\b(abs|min|max|len|type|str|int|float|bool|round|trunc|print|input|sleep|random|range|enumerate|zip|map|filter|reduce|flatten|unique|read_file|write_file|file_exists|list_dir|mkdir|exec|json_parse|json_encode|csv_read|csv_write)\\b"
    - constant.number: "\\b[0-9]+\\.?[0-9]*\\b"
    - constant.string: "\"[^\"]*\""
    - constant.string: "'[^']*'"
    - constant.string: "`[^`]*`"
    - comment: "//.*$"
    - comment: "#.*$"
    - comment: "/\\*([^*]|\\*[^/])*\\*/"
MICRO

    ok "Micro: ${micro_dir}/zen.yaml"
    config_count=$((config_count + 1))
}

# ── Kate / KWrite ────────────────────────────────────────────────────────────
setup_kate() {
    local kate_dirs=()

    for dir in \
        "${HOME}/.local/share/kate/syntax" \
        "${HOME}/.kde/share/apps/kate/syntax" \
        "${HOME}/.kde4/share/apps/kate/syntax" \
        "/usr/share/katepart5/syntax" \
        "/usr/share/kate/syntax"
    do
        if [ -d "$(dirname "$dir")" ]; then
            kate_dirs+=("$dir")
            break
        fi
    done

    if [ ${#kate_dirs[@]} -eq 0 ]; then
        # Create default location anyway
        kate_dirs=("${HOME}/.local/share/kate/syntax")
    fi

    for dir in "${kate_dirs[@]}"; do
        mkdir -p "$dir"

        cat > "${dir}/zen.xml" << 'KATEXML'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE language SYSTEM "katepart4.dtd">
<language name="Zen" version="1.0" kateversion="5.0" section="Sources" extensions="*.z" priority="10">
  <highlighting>
    <list name="Keywords">
      <item> let </item><item> const </item><item> function </item><item> func </item>
      <item> def </item><item> class </item><item> extends </item><item> new </item>
      <item> lambda </item><item> if </item><item> elif </item><item> else </item>
      <item> for </item><item> in </item><item> while </item><item> switch </item>
      <item> case </item><item> default </item><item> break </item><item> continue </item>
      <item> return </item><item> try </item><item> catch </item><item> finally </item>
      <item> with </item><item> as </item><item> load </item><item> use </item>
      <item> include </item><item> import </item><item> require </item>
      <item> throw </item><item> raise </item><item> assert </item><item> typeof </item>
      <item> super </item><item> native </item>
      <item> and </item><item> or </item><item> not </item><item> is </item>
    </list>
    <list name="Builtins">
      <item> abs </item><item> min </item><item> max </item><item> len </item>
      <item> type </item><item> str </item><item> int </item><item> float </item>
      <item> bool </item><item> print </item><item> input </item><item> sleep </item>
      <item> range </item><item> enumerate </item><item> zip </item>
      <item> map </item><item> filter </item><item> reduce </item>
    </list>
    <list name="Constants">
      <item> true </item><item> false </item><item> null </item>
    </list>
    <context name="Normal" attribute="Normal" lineEndContext="#pop">
      <DetectChar char="%" attribute="Comment" context="Comment"/>
      <DetectChar char="/" attribute="Comment" context="Comment"/>
      <StringDetect char="%" attribute="String" context="String"/>
      <StringDetect char="\"" attribute="String" context="StringDQ"/>
      <StringDetect char="'" attribute="String" context="StringSQ"/>
      <keywordString list="Keywords" attribute="Keyword"/>
      <keywordString list="Builtins" attribute="BuiltIn"/>
      <keywordString list="Constants" attribute="Constant"/>
      <Float attribute="Float"/>
      <Int attribute="Int"/>
    </context>
    <context name="Comment" attribute="Comment" lineEndContext="#pop">
      <DetectChar char="\n" attribute="Comment" context="#pop"/>
    </context>
    <context name="String" attribute="String" lineEndContext="#pop">
      <DetectChar char="\n" attribute="Error" context="#pop"/>
    </context>
    <context name="StringDQ" attribute="String" lineEndContext="#pop">
      <DetectChar char="\"" attribute="String" context="#pop"/>
      <DetectChar char="\n" attribute="Error" context="#pop"/>
    </context>
    <context name="StringSQ" attribute="String" lineEndContext="#pop">
      <DetectChar char="'" attribute="String" context="#pop"/>
      <DetectChar char="\n" attribute="Error" context="#pop"/>
    </context>
  </highlighting>
  <comments>
    <comment name="singleLine" start="//" />
    <comment name="singleLine" start="#" />
    <comment name="multiLine" start="/*" end="*/"/>
  </comments>
  <folding>
    <keywords begins="{" ends="}"/>
  </folding>
</language>
KATEXML

        ok "Kate: ${dir}/zen.xml"
        config_count=$((config_count + 1))
    done
}

# ── Gedit / GNOME Text Editor ───────────────────────────────────────────────
setup_gedit() {
    local gedit_dir="${HOME}/.local/share/gtksourceview-4/language-specs"
    [ -d "${HOME}/.local/share/gtksourceview-3.0/language-specs" ] && gedit_dir="${HOME}/.local/share/gtksourceview-3.0/language-specs"
    [ -d "${HOME}/.local/share/gtksourceview-5/language-specs" ] && gedit_dir="${HOME}/local/share/gtksourceview-5/language-specs"

    mkdir -p "$gedit_dir"

    cat > "${gedit_dir}/zen.lang" << 'GEDIT'
<?xml version="1.0" encoding="UTF-8"?>
<!language system "language.dtd">
<language id="zen" name="Zen" version="2.0" _section="Sources">
  <metadata>
    <property name="mimetypes">text/x-zen</property>
    <property name="globs">*.z</property>
  </metadata>
  <definitions>
    <context id="comment_multi" style-ref="comment">
      <start>/\*</start>
      <end>\*/</end>
    </context>
    <context id="comment_single" style-ref="comment">
      <match>//.*$</match>
    </context>
    <context id="comment_hash" style-ref="comment">
      <match>#.*$</match>
    </context>
    <context id="string_double" style-ref="string">
      <start>"</start>
      <end>"</end>
      <include>
        <context id="escaped_char" style-ref="special-char">
          <match>\\[ntr0"\\]</match>
        </context>
      </include>
    </context>
    <context id="string_single" style-ref="string">
      <start>'</start>
      <end>'</end>
    </context>
    <context id="template" style-ref="string">
      <start>`</start>
      <end>`</end>
    </context>
    <context id="keywords" style-ref="keyword">
      <match>\b(let|const|function|func|def|class|extends|new|lambda|if|elif|else|for|in|while|switch|case|default|break|continue|return|try|catch|finally|with|as|load|use|include|import|require|throw|raise|assert|typeof|super|native|and|or|not|is)\b</match>
    </context>
    <context id="constants" style-ref="constant">
      <match>\b(true|false|null)\b</match>
    </context>
    <context id="builtins" style-ref="builtin">
      <match>\b(abs|min|max|len|type|str|int|float|bool|round|trunc|print|input|sleep|random|range|enumerate|zip|map|filter|reduce|flatten|unique|read_file|write_file|file_exists|list_dir|mkdir|exec|json_parse|json_encode|csv_read|csv_write)\b</match>
    </context>
    <context id="numbers" style-ref="number">
      <match>\b\d+\.?\d*\b</match>
    </context>
  </definitions>
</language>
GEDIT

    ok "Gedit/GNOME: ${gedit_dir}/zen.lang"
    config_count=$((config_count + 1))
}

# ── Notepadqq ────────────────────────────────────────────────────────────────
setup_notepadqq() {
    if ! has_cmd notepadqq 2>/dev/null; then
        warn "Notepadqq not found, skipping"
        return
    fi

    local nqq_dir="${HOME}/.config/notepadqq"
    mkdir -p "${nqq_dir}/syntax"

    # Notepadqq uses Kate syntax XML format
    cp "${ROOT}/editors/kate-syntax/zen.xml" "${nqq_dir}/syntax/zen.xml" 2>/dev/null || true
    ok "Notepadqq: syntax installed"
    config_count=$((config_count + 1))
}

# ── Bash completion ──────────────────────────────────────────────────────────
setup_bash_completion() {
    local bash_completion_dirs=()

    for dir in \
        "${HOME}/.bash_completion.d" \
        "${HOME}/.local/share/bash-completion/completions" \
        "/etc/bash_completion.d" \
        "/usr/share/bash-completion/completions"
    do
        if [ -d "$(dirname "$dir")" ] || [ -d "$dir" ]; then
            bash_completion_dirs+=("$dir")
            break
        fi
    done

    if [ ${#bash_completion_dirs[@]} -eq 0 ]; then
        bash_completion_dirs=("${HOME}/.bash_completion.d")
    fi

    local target="${bash_completion_dirs[0]}"
    mkdir -p "$target"

    cp "${ROOT}/editors/bash/zen-completion.bash" "${target}/zen"
    ok "Bash completion: ${target}/zen"
    log "  Source it: source ${target}/zen"
    config_count=$((config_count + 1))
}

# ── Zsh completion ───────────────────────────────────────────────────────────
setup_zsh_completion() {
    local zsh_dirs=()

    for dir in \
        "${HOME}/.zsh/completions" \
        "${HOME}/.config/zsh/completions" \
        "/usr/share/zsh/vendor-completions" \
        "/usr/local/share/zsh/site-functions" \
        "${fpath[1]:-}"
    do
        if [ -d "$dir" ] || [ -d "$(dirname "$dir")" ]; then
            zsh_dirs+=("$dir")
            break
        fi
    done

    if [ ${#zsh_dirs[@]} -eq 0 ]; then
        zsh_dirs=("${HOME}/.zsh/completions")
    fi

    local target="${zsh_dirs[0]}"
    mkdir -p "$target"

    cp "${ROOT}/editors/zsh/_zen" "${target}/_zen"
    ok "Zsh completion: ${target}/_zen"
    log "  Ensure fpath includes ${target} in your .zshrc:"
    log "    fpath=(${target} \$fpath)"
    log "    autoload -Uz compinit && compinit"
    config_count=$((config_count + 1))
}

# ── Main ─────────────────────────────────────────────────────────────────────
main() {
    local mode="auto"
    local only_editor=""

    while [ $# -gt 0 ]; do
        case "$1" in
            --list) mode="list"; shift ;;
            --all)  mode="all"; shift ;;
            --vim)  only_editor="vim"; shift ;;
            --vscode) only_editor="vscode"; shift ;;
            --helix) only_editor="helix"; shift ;;
            --sublime) only_editor="sublime"; shift ;;
            -h|--help)
                echo "Usage: $0 [--list] [--all] [--vim] [--vscode] [--helix] [--sublime]"
                echo ""
                echo "Auto-detects installed editors and configures Zen syntax support."
                echo ""
                echo "Options:"
                echo "  --list     Just list detected editors, don't configure"
                echo "  --all      Configure all editors, even if not detected"
                echo "  --vim      Configure only Vim/Neovim"
                echo "  --vscode   Configure only VS Code"
                echo "  --helix    Configure only Helix"
                echo "  --sublime  Configure only Sublime Text"
                exit 0
                ;;
            *) err "Unknown option: $1"; exit 1 ;;
        esac
    done

    echo ""
    echo -e "${BOLD}Zen Editor Setup${NC}"
    echo -e "${BLUE}────────────────────────────────────────${NC}"
    echo ""
    log "OS: ${OS} | Termux: ${IS_TERMUX}"
    echo ""

    # Always configure shell completions
    if [ -z "$only_editor" ] || [ "$only_editor" = "bash" ]; then
        header "Shell Completions"
        setup_bash_completion
        setup_zsh_completion
    fi

    if [ -z "$only_editor" ] || [ "$only_editor" = "vim" ]; then
        header "Vim / Neovim"
        setup_vim
    fi

    if [ -z "$only_editor" ] || [ "$only_editor" = "vscode" ]; then
        header "VS Code / VSCodium"
        setup_vscode
    fi

    if [ -z "$only_editor" ] || [ "$only_editor" = "helix" ]; then
        header "Helix"
        setup_helix
    fi

    if [ -z "$only_editor" ] || [ "$only_editor" = "sublime" ]; then
        header "Sublime Text"
        setup_sublime
    fi

    if [ -z "$only_editor" ]; then
        header "Emacs"
        setup_emacs

        header "Nano"
        setup_nano

        header "Micro"
        setup_micro

        header "Kate / KWrite"
        setup_kate

        header "Gedit / GNOME Text Editor"
        setup_gedit
    fi

    echo ""
    echo -e "${GREEN}${BOLD}Done!${NC} Configured ${config_count} editor(s)."
    echo ""
    if [ $config_count -eq 0 ]; then
        log "No editors were configured. Install an editor and re-run this script."
    else
        log "Restart your editor(s) to pick up the new syntax files."
    fi
    echo ""
}

main "$@"
