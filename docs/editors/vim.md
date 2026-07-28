# Vim/Neovim Setup

## Installation

### Option 1: Manual Installation

Copy the syntax file to your Vim configuration:

```bash
# For Vim
mkdir -p ~/.vim/syntax
cp editors/vim/syntax/zen.vim ~/.vim/syntax/

# For Neovim
mkdir -p ~/.config/nvim/syntax
cp editors/vim/syntax/zen.vim ~/.config/nvim/syntax/
```

### Option 2: Plugin Manager

#### vim-plug

Add to your `.vimrc` or `init.vim`:

```vim
Plug 'ecnord/zen-vim'
```

Then run:

```vim
:PlugInstall
```

#### lazy.nvim (Neovim)

Add to your `plugins.lua`:

```lua
{
  "ecnord/zen-vim",
  ft = "z",
}
```

## Filetype Detection

Add to your `.vimrc` or `init.vim`:

```vim
autocmd BufNewFile,BufRead *.z setfiletype zen
```

## Features

- Syntax highlighting for all Zen keywords
- Comment highlighting (`//`, `#`, `/* */`)
- String interpolation
- Template literals
- Operator highlighting
- Built-in function highlighting

## Treesitter (Neovim)

For better highlighting with Treesitter:

1. Install the Treesitter parser:

```bash
cd ~/tree-sitter-zen
npx tree-sitter generate
```

2. Add to your Neovim config:

```lua
require'nvim-treesitter.configs'.setup {
  ensure_installed = { "zen" },
  highlight = {
    enable = true,
  },
}
```

3. Add the parser to your runtime path:

```lua
vim.opt.runtimepath:append("~/tree-sitter-zen")
```
