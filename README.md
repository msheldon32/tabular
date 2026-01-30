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

#### Navigation
| Key | Action |
|-----|--------|
| `h` / `←` | Move left |
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `l` / `→` | Move right |
| `gg` | Jump to first row |
| `G` | Jump to last row |
| `0` / '^' | Jump to first column |
| `$` | Jump to last column |
| `Ctrl+d` | Half page down |
| `Ctrl+u` | Half page up |
| `Ctrl+f` | Full page down |
| `Ctrl+b` | Full page up |

#### Editing
| Key | Action |
|-----|--------|
| `i` | Enter insert mode |
| `v` | Enter visual mode |
| `V` | Enter visual (row) mode |
| `Ctrl+v` | Enter visual (column) mode |
| `x` | Clear current cell |
| `o` | Insert row below |
| `O` | Insert row above |
| `dr` | Delete current row |
| `dc` | Delete current column |
| `yr` | Yank (copy) current row |
| `yc` | Yank (copy) current column |
| `p` | Paste row/column below/after |
| `A` | Insert column to the right |
| `X` | Delete current column |

#### Other
| Key | Action |
|-----|--------|
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


### Visual Mode
| Key | Action |
|-----|--------|
| `y` | Yank (copy) current selection |
| `x` | Clear current selection |


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
| `:calc` | Evaluate all formulas |

## Formulas

Cells starting with `=` are treated as formulas. Run `:calc` to evaluate all formulas and replace them with results.

### Cell References

- Single cell: `A1`, `B2`, `AA10`
- Range: `A1:A10` (column), `A1:E1` (row), `A1:C3` (rectangular)

### Operators

`+`, `-`, `*`, `/`, `%` (modulo), `^` (power)

### Functions

| Function | Description |
|----------|-------------|
| `sum(range)` | Sum of values in range |
| `avg(range)` | Average of values in range |
| `min(range)` | Minimum value in range |
| `max(range)` | Maximum value in range |
| `count(range)` | Count of cells in range |

### Examples

```
=A1+B1           # Add two cells
=sum(A1:A10)     # Sum of column range
=avg(A1:E1)      # Average of row range
=A1*2+B1/2       # Arithmetic expression
=sum(A1:A10)/count(A1:A10)  # Manual average
```

**Note:** Formulas are evaluated once and replaced with values. No circular references allowed.

## Display

- **Row numbers**: Displayed on the left (1, 2, 3...)
- **Column letters**: Excel-style letters at the top (A, B, C... Z, AA, AB...)
- **Header mode**: First row is highlighted as a header (on by default, toggle with `:header`)
- **Scrolling**: Large tables scroll automatically as you navigate. Title bar shows visible range when scrolled.

## Status Bar

The status bar shows:
- Current mode (NORMAL/INSERT/COMMAND)
- File name
- `[+]` indicator if there are unsaved changes
- Cursor position in Excel format (e.g., A1, B2, AA15)
