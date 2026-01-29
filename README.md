# Tabular

A lightweight, terminal-based CSV editor with vim-like keybindings.

## Installation

```bash
cargo build --release
```

## Usage

```bash
tabular <file.csv>
```

## Key Bindings

### Normal Mode

| Key | Action |
|-----|--------|
| `h` / `←` | Move left |
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `l` / `→` | Move right |
| `i` | Enter insert mode |
| `x` | Clear current cell |
| `o` | Insert row below |
| `O` | Insert row above |
| `dr` | Delete current row |
| `yr` | Yank (copy) current row |
| `p` | Paste row below |
| `A` | Insert column to the right |
| `X` | Delete current column |
| `:` | Enter command mode |
| `q` | Quit (if no unsaved changes) |
| `Ctrl+c` | Force quit |

### Insert Mode

| Key | Action |
|-----|--------|
| `Escape` / `Ctrl+[` | Save cell, return to normal mode |
| `Enter` | Save cell, return to normal mode |
| `Backspace` | Delete character |
| Any character | Insert character |

**Tip:** Use `Ctrl+[` instead of `Escape` for faster mode switching (avoids terminal escape sequence delay).

### Command Mode

| Command | Action |
|---------|--------|
| `:w` | Save file |
| `:q` | Quit (fails if unsaved changes) |
| `:q!` | Force quit without saving |
| `:wq` | Save and quit |
| `:addcol` | Add column at end |
| `:delcol` | Delete current column |
| `:header` | Toggle header mode |

## Display

- **Row numbers**: Displayed on the left (1, 2, 3...)
- **Column letters**: Excel-style letters at the top (A, B, C... Z, AA, AB...)
- **Header mode**: First row is highlighted as a header (on by default, toggle with `:header`)

## Status Bar

The status bar shows:
- Current mode (NORMAL/INSERT/COMMAND)
- File name
- `[+]` indicator if there are unsaved changes
- Cursor position in Excel format (e.g., A1, B2, AA15)
