# Key Bindings

## Normal Mode

### Navigation
| Key | Action |
|-----|--------|
| `h` / `←` | Move left |
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `l` / `→` | Move right |
| `gg` | Jump to first row |
| `G` | Jump to last row |
| `[N]G` | Jump to row N (e.g., `10G` jumps to row 10) |
| `0` / `^` | Jump to first column |
| `$` | Jump to last column |
| `Ctrl+d` | Half page down |
| `Ctrl+u` | Half page up |
| `Ctrl+f` | Full page down |
| `Ctrl+b` | Full page up |

**Count prefix**: Most navigation keys accept a count prefix. For example:
- `5j` moves down 5 rows
- `10l` moves right 10 columns
- `3k` moves up 3 rows

### Editing
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
| `"x` | Select register x for next yank/paste |

**Count prefix for bulk operations**:
- `5dr` deletes 5 rows starting from cursor
- `3dc` deletes 3 columns starting from cursor
- `5yr` yanks 5 rows (paste will insert all 5)
- `3yc` yanks 3 columns

### Visual Modes
| Key | Action |
|-----|--------|
| `v` | Enter visual mode (select cells) |
| `V` | Enter visual row mode (select rows) |
| `Ctrl+v` | Enter visual column mode (select columns) |
| `yy` | Yank current selection |
| `dd` | Delete current selection |

### Search
| Key | Action |
|-----|--------|
| `/` | Start search |
| `n` | Jump to next match |
| `N` | Jump to previous match |

### Other
| Key | Action |
|-----|--------|
| `:` | Enter command mode |
| `q` | Quit (if no unsaved changes) |
| `Ctrl+c` | Force quit |

## Insert Mode

| Key | Action |
|-----|--------|
| `Escape` / `Ctrl+[` | Save cell, return to normal mode |
| `Enter` | Save cell, return to normal mode |
| `Backspace` | Delete character |
| Any character | Insert character |

**Tip:** Use `Ctrl+[` instead of `Escape` for faster mode switching (avoids terminal escape sequence delay).


## Visual Mode

All visual modes support navigation keys (`h`, `j`, `k`, `l`, etc.) to extend the selection.

| Key | Action |
|-----|--------|
| `y` | Yank (copy) selection |
| `x` | Clear selection |
| `q` | Drag down (fill from top row) |
| `Q` | Drag right (fill from left column) |
| `Escape` / `Ctrl+[` | Cancel and return to normal mode |

### Formatting

> **Note:** Formatting commands **modify the underlying cell values**, not just how they are displayed. These operations may be lossy (e.g., rounding) and permanently change the contents of the cells. You can undo them with `u`.

Format commands start with `f` and modify the selected cells:

| Key | Action | Example |
|-----|--------|---------|
| `ff` | Reset to default (plain number) | `$1,234.56` -> `1234.56` |
| `f,` | Add comma separators | `1234567.89` -> `1,234,567.89` |
| `f$` | Format as currency | `1234.56` -> `$1,234.56` |
| `fe` | Format as scientific notation | `0.00123` -> `1.23e-3` |
| `f%` | Format as percentage | `0.15` -> `15%` |

In **visual row mode** (`V`), formatting applies to entire rows. In **visual column mode** (`Ctrl+v`), formatting applies to entire columns.

Non-numeric cells are left unchanged. All format operations can be undone with `u`.

### Drag Fill

The drag feature works like spreadsheet drag-fill:
- **`q` (drag down)**: Copies the first row of the selection to all rows below, translating cell references (e.g., `A1` becomes `A2`, `A3`, etc.)
- **`Q` (drag right)**: Copies the first column of the selection to all columns to the right, translating cell references (e.g., `A1` becomes `B1`, `C1`, etc.)

In **visual row mode** (`V`), `q` fills entire rows. In **visual column mode** (`Ctrl+v`), `Q` fills entire columns.
