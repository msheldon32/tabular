# Tabular Plugin System

Tabular supports extending functionality through Lua plugins. Plugins can add custom commands that manipulate table data.

## Plugin Location

Plugins are loaded from:

```
~/.config/tabular/plugins/
```

Any file with a `.lua` extension in this directory will be loaded automatically when Tabular starts.

## Commands

| Command | Description |
|---------|-------------|
| `:plugins` | List all loaded plugins |
| `:<plugin_name>` | Run a plugin command |
| `:<plugin_name> arg1 arg2` | Run a plugin with arguments |

## Plugin Structure

A plugin must return a table with a `name` field and a `run` function:

```lua
return {
    name = "my-plugin",
    run = function()
        -- Plugin logic here
    end
}
```

The `name` field registers the command name. After loading, the plugin can be invoked with `:my-plugin`.

## Tabular API

Plugins access Tabular through the global `tabular` table, which provides:

### Context (`tabular.ctx`)

| Field | Type | Description |
|-------|------|-------------|
| `cursor_row` | number | Current cursor row (1-indexed) |
| `cursor_col` | number | Current cursor column (1-indexed) |
| `row_count` | number | Total number of rows |
| `col_count` | number | Total number of columns |

### Arguments (`tabular.args`)

A table containing command arguments passed after the plugin name. For example, `:my-plugin foo bar` would set:

```lua
tabular.args[1] = "foo"
tabular.args[2] = "bar"
```

### Functions

| Function | Description |
|----------|-------------|
| `get_cell(row, col)` | Get the value of a cell (1-indexed) |
| `set_cell(row, col, value)` | Set the value of a cell (1-indexed) |
| `insert_row(at)` | Insert a new row at position (1-indexed) |
| `delete_row(at)` | Delete the row at position (1-indexed) |
| `insert_col(at)` | Insert a new column at position (1-indexed) |
| `delete_col(at)` | Delete the column at position (1-indexed) |
| `set_message(msg)` | Display a message in the status bar |

All row/column indices are **1-indexed** to match Lua conventions.

## Example Plugins

### Clear Current Row

Clears all cells in the current row:

```lua
return {
    name = "clearrow",
    run = function()
        local row = tabular.ctx.cursor_row
        for col = 1, tabular.ctx.col_count do
            tabular.set_cell(row, col, "")
        end
        tabular.set_message("Cleared row " .. row)
    end
}
```

### Fill Column with Value

Fills the current column with a specified value:

```lua
return {
    name = "fillcol",
    run = function()
        local value = tabular.args[1] or ""
        local col = tabular.ctx.cursor_col
        for row = 1, tabular.ctx.row_count do
            tabular.set_cell(row, col, value)
        end
        tabular.set_message("Filled column with: " .. value)
    end
}
```

Usage: `:fillcol Hello`

### Sum Column

Calculates the sum of numeric values in the current column and displays it:

```lua
return {
    name = "sumcol",
    run = function()
        local col = tabular.ctx.cursor_col
        local sum = 0
        for row = 1, tabular.ctx.row_count do
            local val = tonumber(tabular.get_cell(row, col))
            if val then
                sum = sum + val
            end
        end
        tabular.set_message("Sum: " .. sum)
    end
}
```

### Duplicate Row

Duplicates the current row below:

```lua
return {
    name = "duprow",
    run = function()
        local row = tabular.ctx.cursor_row
        tabular.insert_row(row + 1)
        for col = 1, tabular.ctx.col_count do
            local val = tabular.get_cell(row, col)
            tabular.set_cell(row + 1, col, val)
        end
        tabular.set_message("Duplicated row " .. row)
    end
}
```

### Swap Columns

Swaps the current column with the next one:

```lua
return {
    name = "swapcol",
    run = function()
        local col = tabular.ctx.cursor_col
        if col >= tabular.ctx.col_count then
            tabular.set_message("Cannot swap: no column to the right")
            return
        end
        for row = 1, tabular.ctx.row_count do
            local a = tabular.get_cell(row, col)
            local b = tabular.get_cell(row, col + 1)
            tabular.set_cell(row, col, b)
            tabular.set_cell(row, col + 1, a)
        end
        tabular.set_message("Swapped columns " .. col .. " and " .. (col + 1))
    end
}
```

## Notes

- Plugin changes are integrated with the undo system
- Plugins can read values written by `set_cell` in the same execution via `get_cell`
- Row/column insertions and deletions take effect after the plugin completes
- Invalid operations (out of bounds, etc.) are silently ignored
