# Vim/Neovim Setup Guide for VM Project

This guide covers setting up Vim or Neovim with LSP support for Rust development.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Configuration Options](#configuration-options)
- [Plugin Setup](#plugin-setup)
- [Key Mappings](#key-mappings)
- [Troubleshooting](#troubleshooting)

---

## Prerequisites

### Required Tools

1. **Vim/Neovim**
   - Neovim 0.9+ (recommended) or Vim 9.0+
   - Install: See [neovim.io](https://neovim.io/)

2. **rust-analyzer**
   ```bash
   # Install via cargo
   cargo install rust-analyzer

   # Or download pre-built binary
   # Check: https://github.com/rust-analyzer/rust-analyzer#installation
   ```

3. **Cargo tools**
   ```bash
   rustup component add rustfmt
   rustup component add clippy
   ```

---

## Installation

### Option 1: Neovim with nvim-lsp (Recommended)

**Modern, fast, with excellent LSP support**

#### Plugin Manager: vim-plug

Install vim-plug:
```bash
sh -c 'curl -fLo "${XDG_DATA_HOME:-$HOME/.local/share}"/nvim/site/autoload/plug.vim --create-dirs \
       https://raw.githubusercontent.com/junegunn/vim-plug/master/plug.vim'
```

#### Configuration (`~/.config/nvim/init.vim` or `~/.config/nvim/init.lua`)

**Vimscript version**:

```vim
" Plugin setup
call plug#begin('~/.local/share/nvim/plugged')

" LSP Configuration
Plug 'neovim/nvim-lspconfig'
Plug 'hrsh7th/nvim-cmp'         " Completion engine
Plug 'hrsh7th/nvim-cmp'         " Completion sources
Plug 'hrsh7th/cmp-nvim-lsp'     " LSP completion source
Plug 'hrsh7th/cmp-buffer'       " Buffer completion
Plug 'hrsh7th/cmp-path'         " Path completion
Plug 'saadparwaiz1/cmp_luasnip' " Snippet completion

" Snippets
Plug 'L3MON4D3/LuaSnip'
Plug 'rafamadriz/friendly-snippets'

" UI enhancements
Plug 'nvim-lua/lsp-status.nvim'
Plug 'onsails/lspkind-nvim'

" Fuzzy finder
Plug 'nvim-telescope/telescope.nvim'
Plug 'nvim-lua/plenary.nvim'

" Syntax highlighting
Plug 'nvim-treesitter/nvim-treesitter', {'do': ':TSUpdate'}

" Git integration
Plug 'tpope/vim-fugitive'
Plug 'lewis6991/gitsigns.nvim'

" Rust-specific
Plug 'simrat39/rust-tools.nvim'

call plug#end()

" LSP setup
lua << EOF
local lspconfig = require('lspconfig')
local cmp = require('cmp')

-- Rust analyzer setup
lspconfig.rust_analyzer.setup({
  settings = {
    ['rust-analyzer'] = {
      checkOnSave = {
        command = "clippy",
        extraArgs = {"--all-targets", "-D", "warnings"}
      },
      cargo = {
        features = "all",
        loadOutDirsFromCheck = true
      },
      procMacro = {
        enable = true
      },
      diagnostics = {
        enable = true
      },
      inlayHints = {
        bindingModeHints = {
          enable = true
        },
        chainingHints = {
          enable = true
        },
        closingBraceHints = {
          enable = true
        },
        closureReturnTypeHints = {
          enable = "always"
        },
        discriminantHints = {
          enable = "fieldless"
        },
        lifetimeElisionHints = {
          enable = "skip_trivial"
        },
        parameterHints = {
          enable = true
        },
        reborrowHints = {
          enable = "mutable"
        },
        renderColons = true,
        typeHints = {
          enable = true
        }
      }
    }
  }
})

-- Completion setup
cmp.setup({
  snippet = {
    expand = function(args)
      require('luasnip').lsp_expand(args.body)
    end,
  },
  mapping = cmp.mapping.preset.insert({
    ['<C-d>'] = cmp.mapping.scroll_docs(-4),
    ['<C-f>'] = cmp.mapping.scroll_docs(4),
    ['<C-Space>'] = cmp.mapping.complete(),
    ['<CR>'] = cmp.mapping.confirm({ select = true }),
    ['<Tab>'] = cmp.mapping(function(fallback)
      if cmp.visible() then
        cmp.select_next_item()
      else
        fallback()
      end
    end, { 'i', 's' }),
  }),
  sources = cmp.config.sources({
    { name = 'nvim_lsp' },
    { name = 'luasnip' },
  }, {
    { name = 'buffer' },
    { name = 'path' },
  })
})

-- Treesitter setup
require('nvim-treesitter.configs').setup({
  highlight = {
    enable = true,
  },
  incremental_selection = {
    enable = true,
  },
  indent = {
    enable = true,
  },
})

-- Rust tools setup
require('rust-tools').setup({
  server = {
    on_attach = function(_, bufnr)
      -- Hover actions
      vim.keymap.set("n", "<Leader>la", require('rust-tools').hover_actions.hover_actions, { buffer = bufnr })
      -- Code action groups
      vim.keymap.set("n", "<Leader>lr", require('rust-tools').code_action_group.code_action_group, { buffer = bufnr })
    end,
  },
})
EOF

" General settings
set number
set relativenumber
set autoindent
set smartindent
set expandtab
set tabstop=4
set shiftwidth=4
set signcolumn=yes
set updatetime=100
set shortmess+=c

" Rust-specific settings
autocmd FileType rust setlocal shiftwidth=4 tabstop=4 softtabstop=4 expandtab
```

---

### Option 2: Neovim with coc.nvim

**Alternative approach with VSCode-like experience**

#### Installation

```vim
" In init.vim
call plug#begin('~/.local/share/nvim/plugged')

Plug 'neoclide/coc.nvim', {'branch': 'release'}
Plug 'rust-lang/rust.vim'

call plug#end()
```

#### Configuration

```vim
" Coc settings
let g:coc_global_extensions = [
    \ 'coc-rust-analyzer',
    \ 'coc-snippets',
    \ 'coc-pairs',
    \ 'coc-todolist',
    \ ]

" Rust-analyzer specific settings
autocmd FileType rust call s:rust_settings()
function! s:rust_settings() abort
    " Use clippy for on-save checks
    call coc#config('rust-analyzer.checkOnSave.command', 'clippy')
    call coc#config('rust-analyzer.cargo.features', 'all')
    call coc#config('rust-analyzer.inlayHints.enable', v:true)

    " Key mappings
    nnoremap <buffer> <Leader>gd :CocCommand rust-analyzer.gotoDefinition<CR>
    nnoremap <buffer> <Leader>gr :CocCommand rust-analyzer.showReferences<CR>
    nnoremap <buffer> <Leader>ca :CocCommand rust-analyzer.run<CR>
endfunction

" General coc settings
nmap <silent> gd <Plug>(coc-definition)
nmap <silent> gy <Plug>(coc-type-definition)
nmap <silent> gi <Plug>(coc-implementation)
nmap <silent> gr <Plug>(coc-references)
```

---

### Option 3: Vim with vim-lsp

**For Vim 9.0+ (without Neovim features)**

```vim
" Install using vim-plug
call plug#begin('~/.vim/plugged')

Plug 'prabirshrestha/vim-lsp'
Plug 'prabirshrestha/asyncomplete.vim'
Plug 'prabirshrestha/asyncomplete-lsp.vim'
Plug 'rhysd/vimrust.vim'

call plug#end()

" LSP configuration
if executable('rust-analyzer')
    au User lsp_setup call lsp#register_server({
        \ 'name': 'rust-analyzer',
        \ 'cmd': {server_info->['rust-analyzer']},
        \ 'whitelist': ['rust'],
        \ })
endif

" Auto-format on save
autocmd BufWritePre *.rs call execute('LspDocumentFormatSync')
```

---

## Key Mappings

### Neovim LSP (nvim-lspconfig)

Default mappings (add to your config):

```lua
-- Navigation
vim.keymap.set('n', 'gD', vim.lsp.buf.declaration, bufopts)
vim.keymap.set('n', 'gd', vim.lsp.buf.definition, bufopts)
vim.keymap.set('n', 'K', vim.lsp.buf.hover, bufopts)
vim.keymap.set('n', 'gi', vim.lsp.buf.implementation, bufopts)
vim.keymap.set('n', '<C-k>', vim.lsp.buf.signature_help, bufopts)
vim.keymap.set('n', '<space>wa', vim.lsp.buf.add_workspace_folder, bufopts)
vim.keymap.set('n', '<space>wr', vim.lsp.buf.remove_workspace_folder, bufopts)
vim.keymap.set('n', '<space>wl', function()
    print(vim.inspect(vim.lsp.buf.list_workspace_folders()))
end, bufopts)
vim.keymap.set('n', '<space>D', vim.lsp.buf.type_definition, bufopts)
vim.keymap.set('n', '<space>rn', vim.lsp.buf.rename, bufopts)
vim.keymap.set('n', '<space>ca', vim.lsp.buf.code_action, bufopts)
vim.keymap.set('n', 'gr', vim.lsp.buf.references, bufopts)
vim.keymap.set('n', '<space>f', function() vim.lsp.buf.format { async = true } end, bufopts)
```

### Coc.nvim

```vim
" Use <c-space> to trigger completion
inoremap <silent><expr> <c-space> coc#refresh()

" Use `[g` and `]g` to navigate diagnostics
nmap <silent> [g <Plug>(coc-diagnostic-prev)
nmap <silent> ]g <Plug>(coc-diagnostic-next)

" Go to code navigation
nmap <silent> gd <Plug>(coc-definition)
nmap <silent> gy <Plug>(coc-type-definition)
nmap <silent> gi <Plug>(coc-implementation)
nmap <silent> gr <Plug>(coc-references)

" Apply codeAction
nmap <leader>ac  <Plug>(coc-codeaction)
```

---

## Project-Specific Configuration

### `.nvimrc` or `.exrc`

Create a project-specific config file in the VM project root:

**`.nvimrc`**:

```vim
" VM project settings

" Set tab width for Rust
autocmd FileType rust setlocal shiftwidth=4 tabstop=4 softtabstop=4 expandtab

" Enable inlay hints
lua require('lsp-inlayhints').setup()

" Quick commands
nnoremap <leader>ct :!cargo test --workspace<CR>
nnoremap <leader>cb :!cargo build --workspace<CR>
nnoremap <leader>cf :!cargo fmt --all<CR>
nnoremap <leader>cc :!cargo clippy --workspace --all-targets -- -D warnings<CR>
nnoremap <leader>cq :!./scripts/quick_test.sh<CR>

" Run current test
nnoremap <leader>tt :wa<CR>:!cargo test --package <C-r>=expand('%:p:h:t')<CR> -- <C-r>=expand('<cword>')<CR><CR>

" Run tests in current file
autocmd FileType rust nnoremap <buffer> <leader>tf :wa<CR>:!cargo test --package <C-r>=expand('%:p:h:t')<CR> -- %<CR>
```

---

## Troubleshooting

### rust-analyzer Not Starting

**Check if rust-analyzer is installed**:
```bash
rust-analyzer --version
```

**Check path in Neovim**:
```lua
:lua print(vim.inspect(vim.lsp.get_active_clients()))
```

**Restart LSP**:
```vim
:LspRestart
```

### No Autocompletion

**Check completion engine**:
```vim
:lua print(vim.inspect(require('cmp').get_config()))
```

**Restart Neovim after plugin installation**:
```vim
:PlugInstall
:qa
nvim
```

### Slow Performance

**Disable unused features**:
```lua
lspconfig.rust_analyzer.setup({
  settings = {
    ['rust-analyzer'] = {
      diagnostics = {
        disabled = {"unresolved-proc-macro"}
      },
      procMacro = {
        enable = false  " Disable if slow
      }
    }
  }
})
```

### Inlay Hints Not Showing

**Enable inlay hints**:
```lua
vim.lsp.inlay_hint.enable(true)
```

**Or with rust-tools**:
```lua
require('rust-tools').setup({
  tools = {
    inlay_hints = {
      auto = true,
      show_parameter_hints = true,
      parameter_hints_prefix = "<- ",
      other_hints_prefix = "=> ",
    }
  }
})
```

---

## Additional Plugins

### Fuzzy Finding (Telescope)

```vim
Plug 'nvim-telescope/telescope.nvim'
Plug 'nvim-lua/plenary.nvim'

" Find files
nnoremap <leader>ff :Telescope find_files<CR>
" Live grep
nnoremap <leader>fg :Telescope live_grep<CR>
" Buffer list
nnoremap <leader>fb :Telescope buffers<CR>
```

### Status Line (Lightline)

```vim
Plug 'itchyny/lightline.vim'
Plug 'nvim-lua/lsp-status.nvim'

let g:lightline = {
      \ 'active': {
      \   'left': [ [ 'mode', 'paste' ],
      \             [ 'lspstatus', 'readonly', 'filename', 'modified' ] ]
      \ },
      \ 'component_function': {
      \   'lspstatus': 'LspStatus'
      \ },
      \ }
```

### Git Integration (Gitsigns)

```vim
Plug 'lewis6991/gitsigns.nvim'

require('gitsigns').setup{
  signs = {
    add          = {hl = 'GitGutterAdd'   , text = '│', numhl='GitGutterAdd'   , linehl='GitGutterAdd'},
    change       = {hl = 'GitGutterChange', text = '│', numhl='GitGutterChange', linehl='GitGutterChange'},
    delete       = {hl = 'GitGutterDelete', text = '_', numhl='GitGutterDelete', linehl='GitGutterDelete'},
    topdelete    = {hl = 'GitGutterDelete', text = '‾', numhl='GitGutterDelete', linehl='GitGutterDelete'},
    changedelete = {hl = 'GitGutterChange', text = '~', numhl='GitGutterChange', linehl='GitGutterChange'},
  },
}
```

---

## Testing Integration

Run tests from within Vim:

```vim
" Run all tests
nnoremap <leader>ta :!cargo test --workspace<CR>

" Run tests for current package
autocmd FileType rust nnoremap <buffer> <leader>tp :!cargo test --package <C-r>=expand('%:p:h:t')<CR><CR>

" Run test under cursor
autocmd FileType rust nnoremap <buffer> <leader>tt :wa<CR>:!cargo test --package <C-r>=expand('%:p:h:t')<CR> -- <C-r>=expand('<cword>')<CR><CR>

" Run tests with output
autocmd FileType rust nnoremap <buffer> <leader>to :!cargo test --workspace -- --nocapture<CR>
```

---

## Quick Reference

### Common Commands

| Action | Neovim LSP | Coc.nvim |
|--------|------------|-----------|
| Go to definition | `gd` | `gd` |
| Find references | `gr` | `gr` |
| Hover docs | `K` | `K` |
| Rename symbol | `<space>rn` | `<space>rn` |
| Code actions | `<space>ca` | `<space>ca` |
| Format | `<space>f` | `<space>f` |
| Diagnostics | `[d` / `]d` | `[g` / `]g` |

### Cargo Commands

```vim
:terminal cargo build      " Build in terminal
:terminal cargo test       " Run tests
:terminal cargo fmt        " Format code
:terminal cargo clippy     " Run linter
```

---

**Happy hacking in Vim! 🦀**
