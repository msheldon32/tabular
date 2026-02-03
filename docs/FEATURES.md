# Features

## Undo/Redo

All editing operations can be undone and redone:
- `u` - Undo last change
- `Ctrl+r` - Redo last undone change

The undo history tracks cell edits, row/column insertions and deletions, drag fills, sorting, and more.

## Search

Press `/` to enter search mode. Type a pattern and press `Enter` to search. The search is case-insensitive and matches any part of cell content.

| Key | Action |
|-----|--------|
| `/` | Start search |
| `Enter` | Execute search and jump to first match |
| `n` | Jump to next match |
| `N` | Jump to previous match |
| `Escape` | Cancel search |

The status bar shows the current match position (e.g., `[3/15] matches`).

## Registers

Tabular uses a vim-style register system for yank and paste operations.

### Available Registers

| Register | Description |
|----------|-------------|
| `""` | Unnamed register (default) - used when no register specified |
| `"a` - `"z` | Named registers - for storing multiple clips |
| `"0` | Yank register - holds last yanked content (not affected by deletes) |
| `"_` | Black hole register - discards content |
| `"+` | System clipboard register |

### Usage

Type `"` followed by the register name before a yank or paste command:

| Example | Action |
|---------|--------|
| `"ayy` | Yank current row into register `a` |
| `"ap` | Paste from register `a` |
| `"_dd` | Delete row without affecting any register |
| `"+yy` | Yank row to system clipboard |
| `"+p` | Paste from system clipboard |
| `"0p` | Paste last yanked content (even after deletes) |

### Behavior

- **Yank operations** (`y`, `yy`, `yr`, `yc`) update both the specified register and the yank register (`"0`)
- **Delete operations** (`d`, `dd`, `dr`, `dc`, `x`) update the unnamed register but not the yank register
- **Black hole register** (`"_`) discards content entirely
- **Named registers** (`"a`-`"z`) persist until overwritten

## Formulas

Cells starting with `=` are treated as formulas. Run `:calc` to evaluate all formulas and replace them with results.

Formulas recognize formatted numbers in cell references:
- Currency values like `$1,234.56` are read as `1234.56`
- Percentages like `15%` are read as `0.15`

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
```

**Note:** Formulas are evaluated once and replaced with values. No circular references allowed.

## Display

- **Row numbers**: Displayed on the left (1, 2, 3...)
- **Column letters**: Excel-style letters at the top (A, B, C... Z, AA, AB...)
- **Header mode**: First row is highlighted as a header (on by default, toggle with `:header`)
- **Scrolling**: Large tables scroll automatically as you navigate

### Display Precision

Control how many decimal places are shown for numeric values:

| Command | Effect |
|---------|--------|
| `:prec 2` | Display numbers with 2 decimal places |
| `:prec 0` | Display numbers as integers (rounded) |
| `:prec` or `:prec auto` | Display numbers as stored (default) |

**Note:** Display precision only affects how numbers are shown—it does not modify the underlying cell values.

## Themes

Tabular supports color themes. Switch themes with `:theme [name]`:

| Theme | Description |
|-------|-------------|
| `dark` | Default dark theme |
| `light` | Light theme for light terminals |
| `solarized-dark` | Solarized dark color scheme |

Custom themes can be defined in TOML files. See [styles.md](styles.md) for details.

## Status Bar

The status bar shows:
- Current mode (NORMAL/INSERT/COMMAND/VISUAL)
- File name
- `[+]` indicator if there are unsaved changes
- Cursor position in Excel format (e.g., A1, B2, AA15)
- Filter indicator when filtering is active
- Search match count when searching
