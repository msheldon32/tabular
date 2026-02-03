# Tabular

A terminal-based CSV editor with vim-like keybindings.

## Philosophy

Tabular is **not** a spreadsheet. It's a focused tool for editing tabular data:

- **Vim-inspired**: Modal editing, registers, counts, visual modes - if you know vim, you know tabular
- **Minimal**: Does one thing well. No charts, no pivot tables, no macros
- **Fast**: Opens instantly, handles large files, stays out of your way

## Install

```bash
cargo build --release
cp target/release/tabular ~/.local/bin/
```

## Quick Start

```bash
tabular data.csv
```

| Key | Action |
|-----|--------|
| `h` `j` `k` `l` | Navigate |
| `i` | Edit cell |
| `Enter` or `Esc` | Finish editing |
| `o` / `O` | Insert row below/above |
| `dr` / `dc` | Delete row/column |
| `yr` / `yc` | Yank row/column |
| `p` | Paste |
| `u` / `Ctrl+r` | Undo/redo |
| `/` | Search |
| `:w` | Save |
| `:q` | Quit |

## Features

- **Visual selection**: `v` (cells), `V` (rows), `Ctrl+v` (columns)
- **Sorting**: `:sort`, `:sortd` - auto-detects numeric vs text
- **Filtering**: `:filter > 100`, `:filter = active`
- **Find/replace**: `:%s/old/new/g`
- **Formulas**: `=sum(A1:A10)`, `=avg(B1:B5)`, then `:calc`
- **Registers**: `"ayy` to yank into register a, `"ap` to paste
- **Formatting**: Visual select then `f$` (currency), `f%` (percent), `f,` (commas)
- **Themes**: `:theme dark`, `:theme light`, `:theme solarized-dark`
- **Plugins**: Extend with Lua scripts

## Documentation

- [Key Bindings](docs/KEYBINDINGS.md) - Complete keyboard reference
- [Commands](docs/COMMANDS.md) - All `:` commands
- [Features](docs/FEATURES.md) - Detailed feature documentation
- [Plugins](docs/PLUGINS.md) - Writing Lua plugins
- [Themes](docs/styles.md) - Custom color themes

## License

MPL
