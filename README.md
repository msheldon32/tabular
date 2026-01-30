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
| `0` / `^` | Jump to first column |
| `$` | Jump to last column |
| `Ctrl+d` | Half page down |
| `Ctrl+u` | Half page up |
| `Ctrl+f` | Full page down |
| `Ctrl+b` | Full page up |

#### Editing
| Key | Action |
|-----|--------|
| `i` | Enter insert mode |
| `x` | Clear current cell |
| `o` | Insert row below |
| `O` | Insert row above |
| `a` | Insert column to the left |
| `A` | Insert column to the right |
| `dr` | Delete current row |
| `dc` | Delete current column |
| `X` | Delete current column |
| `yr` | Yank (copy) current row |
| `yc` | Yank (copy) current column |
| `p` | Paste yanked content |
| `u` | Undo |
| `Ctrl+r` | Redo |

#### Visual Modes
| Key | Action |
|-----|--------|
| `v` | Enter visual mode (select cells) |
| `V` | Enter visual row mode (select rows) |
| `Ctrl+v` | Enter visual column mode (select columns) |
| `yy` | Yank current selection |

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

All visual modes support navigation keys (`h`, `j`, `k`, `l`, etc.) to extend the selection.

| Key | Action |
|-----|--------|
| `y` | Yank (copy) selection |
| `x` | Clear selection |
| `q` | Drag down (fill from top row) |
| `Q` | Drag right (fill from left column) |
| `Escape` / `Ctrl+[` | Cancel and return to normal mode |

#### Drag Fill

The drag feature works like spreadsheet drag-fill:
- **`q` (drag down)**: Copies the first row of the selection to all rows below, translating cell references (e.g., `A1` becomes `A2`, `A3`, etc.)
- **`Q` (drag right)**: Copies the first column of the selection to all columns to the right, translating cell references (e.g., `A1` becomes `B1`, `C1`, etc.)

In **visual row mode** (`V`), `q` fills entire rows. In **visual column mode** (`Ctrl+v`), `Q` fills entire columns.


### Command Mode

| Command | Action |
|---------|--------|
| `:w` | Save file |
| `:q` | Quit (fails if unsaved changes) |
| `:q!` | Force quit without saving |
| `:wq` | Save and quit |
| `:addcol` | Add column after current |
| `:delcol` | Delete current column |
| `:header` | Toggle header mode |
| `:calc` | Evaluate all formulas |
| `:sort` | Sort rows by current column (ascending) |
| `:sortd` | Sort rows by current column (descending) |
| `:sortr` | Sort columns by current row (ascending) |
| `:sortrd` | Sort columns by current row (descending) |
| `:[NUMBER]` | Jump to row NUMBER |
| `:[CELL]` | Jump to CELL (e.g., `:A1`, `:B5`) |

## Sorting

Sort data by navigating to the column (or row) you want to sort by, then use `:sort` or `:sortd`.

**Automatic type detection**: Tabular probes the column to determine if it contains numeric or text data:
- **Numeric sort**: If the majority of non-empty cells are numbers, sorting is done numerically
- **Text sort**: Otherwise, sorting is case-insensitive alphabetical

**Header preservation**: When header mode is enabled (default), the first row is kept in place during sorting.

**Examples**:
- Navigate to the "Score" column and type `:sort` to sort scores low-to-high
- Use `:sortd` to sort high-to-low (descending)
- Use `:sortr` to rearrange columns based on a row's values

## Undo/Redo

All editing operations can be undone and redone:
- `u` - Undo last change
- `Ctrl+r` - Redo last undone change

The undo history tracks cell edits, row/column insertions and deletions, drag fills, and more.

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
- Current mode (NORMAL/INSERT/COMMAND/VISUAL)
- File name
- `[+]` indicator if there are unsaved changes
- Cursor position in Excel format (e.g., A1, B2, AA15)
