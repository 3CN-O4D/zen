#!/usr/bin/env bash
# Zen editor setup script
# Auto-detects installed editors and installs Zen syntax highlighting
# Supports: VS Code, Vim/Neovim, Helix, Sublime Text, Emacs, Zed, Nano
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
EDITOR_DIR="$SCRIPT_DIR/editors"

log()  { echo -e "${GREEN}[ok]${NC}    $*"; }
warn() { echo -e "${YELLOW}[skip]${NC}  $*"; }
info() { echo -e "${BLUE}[info]${NC}  $*"; }

# ─── Helpers ──────────────────────────────────────────────────────────────
has_cmd() { command -v "$1" >/dev/null 2>&1; }

# ─── VS Code / VSCodium / Cursor ──────────────────────────────────────────
setup_vscode() {
    local name="$1" binary="$2" config_dir="$3"

    if ! has_cmd "$binary"; then
        warn "$name not found"
        return
    fi

    log "Found $name"

    # Ensure extensions dir exists
    local ext_dir="$config_dir/extensions"
    local zen_ext_dir="$ext_dir/zen-language"
    mkdir -p "$zen_ext_dir/syntaxes"
    mkdir -p "$zen_ext_dir"

    # Copy files
    cp "$EDITOR_DIR/vscode/package.json" "$zen_ext_dir/"
    cp "$EDITOR_DIR/vscode/language-configuration.json" "$zen_ext_dir/"
    cp "$EDITOR_DIR/vscode/syntaxes/zen.tmLanguage.json" "$zen_ext_dir/syntaxes/"

    log "Installed Zen syntax for $name -> $zen_ext_dir"
}

setup_all_vscode() {
    echo ""
    echo "── VS Code variants ──"

    # VS Code
    local vscode_config=""
    case "$(uname -s)" in
        Linux*)  vscode_config="$HOME/.config/Code/User" ;;
        Darwin*) vscode_config="$HOME/Library/Application Support/Code/User" ;;
    esac
    [ -n "$vscode_config" ] && setup_vscode "VS Code" "code" "$vscode_config" || true

    # VSCodium
    local codium_config=""
    case "$(uname -s)" in
        Linux*)  codium_config="$HOME/.config/VSCodium/User" ;;
        Darwin*) codium_config="$HOME/Library/Application Support/VSCodium/User" ;;
    esac
    [ -n "$codium_config" ] && setup_vscode "VSCodium" "codium" "$codium_config"

    # Cursor
    local cursor_config=""
    case "$(uname -s)" in
        Linux*)  cursor_config="$HOME/.config/Cursor/User" ;;
        Darwin*) cursor_config="$HOME/Library/Application Support/Cursor/User" ;;
    esac
    [ -n "$cursor_config" ] && setup_vscode "Cursor" "cursor" "$cursor_config"

    # code-server (web VS Code)
    local cs_config="$HOME/.local/share/code-server/User"
    if [ -d "$cs_config/.." ]; then
        setup_vscode "code-server" "code-server" "$cs_config"
    fi
}

# ─── Vim ──────────────────────────────────────────────────────────────────
setup_vim() {
    echo ""
    echo "── Vim / Neovim ──"

    local vim_dir=""
    local nvim_dir=""

    # Detect Vim
    local vim_home="${VIM_CONFIG_DIR:-$HOME/.vim}"
    if has_cmd vim || has_cmd nvim; then
        vim_dir="$vim_home"
    fi

    # Detect Neovim
    if has_cmd nvim; then
        nvim_dir="${XDG_CONFIG_HOME:-$HOME/.config}/nvim"
    fi

    # Install Vim syntax
    if [ -n "$vim_dir" ]; then
        mkdir -p "$vim_dir/syntax"
        mkdir -p "$vim_dir/ftdetect"
        cp "$EDITOR_DIR/vim/syntax/zen.vim" "$vim_dir/syntax/"

        cat > "$vim_dir/ftdetect/zen.vim" << 'FTDETECT'
autocmd BufNewFile,BufRead *.z setfiletype zen
autocmd BufNewFile,BufRead *.zen setfiletype zen
FTDETECT
        log "Installed Vim syntax -> $vim_dir/syntax/zen.vim"
    else
        warn "Vim not found"
    fi

    # Install Neovim syntax (reuse Vim syntax file)
    if [ -n "$nvim_dir" ]; then
        mkdir -p "$nvim_dir/syntax/zen"
        mkdir -p "$nvim_dir/ftdetect"
        cp "$EDITOR_DIR/vim/syntax/zen.vim" "$nvim_dir/syntax/zen.vim"

        cat > "$nvim_dir/ftdetect/zen.vim" << 'FTDETECT'
autocmd BufNewFile,BufRead *.z setfiletype zen
autocmd BufNewFile,BufRead *.zen setfiletype zen
FTDETECT
        log "Installed Neovim syntax -> $nvim_dir/syntax/zen.vim"
    else
        info "Neovim not installed (skipped)"
    fi
}

# ─── Helix ────────────────────────────────────────────────────────────────
setup_helix() {
    echo ""
    echo "── Helix ──"

    if ! has_cmd hx; then
        warn "Helix not found"
        return
    fi

    local hx_config="${XDG_CONFIG_HOME:-$HOME/.config}/helix"
    mkdir -p "$hx_config/languages.d"
    cp "$EDITOR_DIR/helix/languages.toml" "$hx_config/languages.d/zen.toml"
    log "Installed Helix config -> $hx_config/languages.d/zen.toml"
}

# ─── Sublime Text ─────────────────────────────────────────────────────────
setup_sublime() {
    echo ""
    echo "── Sublime Text ──"

    local subl_dir=""

    case "$(uname -s)" in
        Linux)
            subl_dir="$HOME/.config/sublime-text"
            ;;
        Darwin*)
            subl_dir="$HOME/Library/Application Support/Sublime Text"
            ;;
    esac

    if [ -z "$subl_dir" ] || [ ! -d "$subl_dir" ]; then
        warn "Sublime Text not found"
        return
    fi

    local pkg_dir="$subl_dir/Packages/Zen"
    mkdir -p "$pkg_dir"
    cp "$EDITOR_DIR/sublime-text/zen.sublime-syntax" "$pkg_dir/"
    log "Installed Sublime Text syntax -> $pkg_dir/zen.sublime-syntax"
}

# ─── Emacs ────────────────────────────────────────────────────────────────
setup_emacs() {
    echo ""
    echo "── Emacs ──"

    if ! has_cmd emacs; then
        warn "Emacs not found"
        return
    fi

    local emacs_dir="$HOME/.emacs.d"
    mkdir -p "$emacs_dir/lisp"

    cat > "$emacs_dir/lisp/zen-mode.el" << 'EMACS'
;;; zen-mode.el --- Zen language major mode -*- lexical-binding: t; -*-

(defvar zen-mode-syntax-table
  (let ((st (make-syntax-table)))
    (modify-syntax-entry ?/ ". 124" st)
    (modify-syntax-entry ?* ". 23b" st)
    (modify-syntax-entry ?\n "> a" st)
    (modify-syntax-entry ?# "< a" st)
    (modify-syntax-entry ?' "\"" st)
    st)
  "Syntax table for `zen-mode'.")

(defvar zen-keywords
  '("let" "const" "global" "function" "def" "fn" "class" "extends"
    "new" "lambda" "if" "elif" "else" "for" "in" "while" "break"
    "continue" "return" "try" "catch" "finally" "throw" "raise"
    "import" "from" "as" "load" "include" "switch" "case" "default"
    "this" "self" "true" "false" "null" "typeof" "and" "or" "not")
  "Zen language keywords.")

(defun zen-indent-line ()
  "Indent current line."
  (interactive)
  (let ((indent 0))
    (save-excursion
      (forward-line -1)
      (when (looking-at ".*{\\s-*$")
        (setq indent (+ indent 4))))
    (indent-line-to indent)))

;;;###autoload
(define-derived-mode zen-mode prog-mode "Zen"
  "Major mode for editing Zen language files."
  :syntax-table zen-mode-syntax-table
  (font-lock-add-keywords nil
    `((,(regexp-opt zen-keywords 'symbols) . font-lock-keyword-face)
      ("\\b\\(true\\|false\\|null\\)\\b" . font-lock-constant-face)
      ("\\b\\(print\\|input\\|len\\|str\\|int\\|float\\|bool\\|list\\|type\\|typeof\\)\\b" . font-lock-builtin-face)
      ("\\(//.*\\)$" . font-lock-comment-face)
      ("\\(#.*\\)$" . font-lock-comment-face)
      ("\"[^\"\\\\]*\\\\.[^\"\\\\]*\"" . font-lock-string-face)
      ("'[^'\\\\]*\\\\.[^'\\\\]*'" . font-lock-string-face)))
  (setq-local indent-line-function #'zen-indent-line)
  (setq-local comment-start "// ")
  (setq-local comment-end ""))

;;;###autoload
(add-to-list 'auto-mode-alist '("\\.z\\'" . zen-mode))
(add-to-list 'auto-mode-alist '("\\.zen\\'" . zen-mode))

(provide 'zen-mode)
;;; zen-mode.el ends here
EMACS

    log "Installed Emacs mode -> $emacs_dir/lisp/zen-mode.el"

    # Try to add to load-path if init.el exists
    local init_file="$emacs_dir/init.el"
    if [ -f "$init_file" ] && ! grep -q "zen-mode" "$init_file" 2>/dev/null; then
        cat >> "$init_file" << 'INIT'

;; Zen language support
(add-to-list 'load-path (expand-file-name "lisp" user-emacs-directory))
(require 'zen-mode)
INIT
        log "Added zen-mode to Emacs init.el"
    fi
}

# ─── Zed ──────────────────────────────────────────────────────────────────
setup_zed() {
    echo ""
    echo "── Zed ──"

    if ! has_cmd zed; then
        warn "Zed not found"
        return
    fi

    local zed_config="${XDG_CONFIG_HOME:-$HOME/.config}/zed"
    mkdir -p "$zed_config/languages"

    cat > "$zed_config/languages/zen.json" << ZEDEOF
{
  "name": "Zen",
  "grammar": "tree-sitter-zen",
  "path_suffixes": ["z", "zen"],
  "line_comments": ["//", "#"],
  "block_comment": ["/*", "*/"],
  "autoclose_before": ",;)}]:\\\"' "
}
ZEDEOF

    log "Installed Zed config -> $zed_config/languages/zen.json"
}

# ─── Nano ─────────────────────────────────────────────────────────────────
setup_nano() {
    echo ""
    echo "── Nano ──"

    local nanorc="$HOME/.nanorc"
    if ! has_cmd nano; then
        warn "Nano not found"
        return
    fi

    # Add Zen syntax to nanorc if not already present
    if ! grep -q "zen" "$nanorc" 2>/dev/null; then
        cat >> "$nanorc" << 'NANORC'

# Zen language
syntax zen "\.z$"
icolor green "\b(let|const|global|function|def|class|extends|new|lambda|if|elif|else|for|in|while|break|continue|return|try|catch|finally|throw|raise|import|from|as|load|include)\b"
icolor cyan "\b(print|input|len|str|int|float|bool|list|type|typeof|abs|min|max|round|hex|range|exit|sleep|wait)\b"
icolor yellow "\b(true|false|null)\b"
icolor magenta "\"[^\"\\]*\"|'[^'\\]*'"
icolor blue "//.*$"
NANORC
        log "Added Zen syntax to $nanorc"
    else
        info "Nano already has Zen syntax configured"
    fi
}

# ─── Tree-sitter grammar (Neovim/other editors) ──────────────────────────
setup_tree_sitter() {
    echo ""
    echo "── Tree-sitter grammar ──"

    local ts_dir="${XDG_DATA_HOME:-$HOME/.local/share}/tree-sitter/zen"
    mkdir -p "$ts_dir"
    cp "$EDITOR_DIR/tree-sitter/grammar.js" "$ts_dir/"
    log "Copied tree-sitter grammar -> $ts_dir/grammar.js"

    info "To build the grammar for Neovim:"
    info "  cd $ts_dir"
    info "  tree-sitter generate"
    info "  tree-sitter build"
}

# ─── Main ──────────────────────────────────────────────────────────────────
main() {
    local action="${1:-all}"

    echo ""
    echo "╔══════════════════════════════════════╗"
    echo "║     Zen Editor Setup Script          ║"
    echo "╚══════════════════════════════════════╝"
    echo ""
    info "Source: $EDITOR_DIR"

    case "$action" in
        all)
            setup_all_vscode
            setup_vim
            setup_helix
            setup_sublime
            setup_emacs
            setup_zed
            setup_nano
            setup_tree_sitter
            ;;
        vscode)   setup_all_vscode ;;
        vim)      setup_vim ;;
        helix)    setup_helix ;;
        sublime)  setup_sublime ;;
        emacs)    setup_emacs ;;
        zed)      setup_zed ;;
        nano)     setup_nano ;;
        *)
            echo "Usage: $0 [all|vscode|vim|helix|sublime|emacs|zed|nano]"
            exit 1
            ;;
    esac

    echo ""
    log "Editor setup complete!"
    echo ""
}

main "$@"
