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
| `selection` | table/nil | Selection info if in visual mode (see below) |

#### Selection Info (`tabular.ctx.selection`)

When in visual mode, `tabular.ctx.selection` contains:

| Field | Type | Description |
|-------|------|-------------|
| `start_row` | number | Selection start row (1-indexed) |
| `start_col` | number | Selection start column (1-indexed) |
| `end_row` | number | Selection end row (1-indexed) |
| `end_col` | number | Selection end column (1-indexed) |
| `mode` | string | One of: `"visual"`, `"visual_row"`, `"visual_col"` |

### Arguments (`tabular.args`)

A table containing command arguments passed after the plugin name. For example, `:my-plugin foo bar` would set:

```lua
tabular.args[1] = "foo"
tabular.args[2] = "bar"
```

### Core Functions

| Function | Description |
|----------|-------------|
| `get_cell(row, col)` | Get the value of a cell (1-indexed) |
| `set_cell(row, col, value)` | Set the value of a cell (1-indexed) |
| `insert_row(at)` | Insert a new row at position (1-indexed) |
| `delete_row(at)` | Delete the row at position (1-indexed) |
| `insert_col(at)` | Insert a new column at position (1-indexed) |
| `delete_col(at)` | Delete the column at position (1-indexed) |
| `set_message(msg)` | Display a message in the status bar |

### Selection & Range Functions

| Function | Description |
|----------|-------------|
| `get_selection()` | Returns selection bounds table or nil if not in visual mode |
| `get_range(r1, c1, r2, c2)` | Returns a 2D table of cell values for the given range (1-indexed) |
| `get_column_type(col)` | Returns `"numeric"` or `"text"` based on column content |

#### get_selection()

Returns a table with selection bounds if in visual mode, or `nil` otherwise:

```lua
local sel = tabular.get_selection()
if sel then
    print(sel.start_row, sel.start_col, sel.end_row, sel.end_col)
    print(sel.mode) -- "visual", "visual_row", or "visual_col"
end
```

#### get_range(r1, c1, r2, c2)

Returns a 2D table of values. Useful for batch operations:

```lua
local data = tabular.get_range(1, 1, 3, 2)
-- data[1][1] = cell at row 1, col 1
-- data[1][2] = cell at row 1, col 2
-- data[2][1] = cell at row 2, col 1
-- etc.
```

### Persistent Storage

Plugins can store data that persists between sessions:

| Function | Description |
|----------|-------------|
| `save_data(key, value)` | Save a string value to persistent storage |
| `load_data(key)` | Load a value from storage, returns string or nil |

Data is stored in `~/.config/tabular/data/` with each key as a separate file.

```lua
-- Save state
tabular.save_data("my_plugin_config", "some_value")

-- Load state
local config = tabular.load_data("my_plugin_config")
if config then
    print("Loaded: " .. config)
end
```

### User Input (Prompt)

| Function | Description |
|----------|-------------|
| `prompt(question, default)` | Request input from user, returns answer or nil |

The prompt function works with deferred execution. On first call it returns `nil` and queues a prompt. Tabular will show the prompt, collect user input, and re-run the plugin with the answer available.

```lua
local answer = tabular.prompt("Enter a value:", "default")
if answer then
    -- User provided input, proceed
    tabular.set_message("You entered: " .. answer)
else
    -- Waiting for user input, plugin will be re-run
    return
end
```

### Canvas API (`tabular.canvas`)

Plugins can display rich output using the canvas overlay:

| Function | Description |
|----------|-------------|
| `canvas.clear()` | Clear all canvas content |
| `canvas.show()` | Show the canvas overlay |
| `canvas.hide()` | Hide the canvas overlay |
| `canvas.set_title(title)` | Set the canvas title |
| `canvas.add_text(text)` | Add a line of text |
| `canvas.add_header(text)` | Add a styled header line |
| `canvas.add_separator()` | Add a horizontal separator |
| `canvas.add_blank()` | Add a blank line |
| `canvas.add_styled_text(text, fg, bg, bold)` | Add styled text with colors |

#### Colors for add_styled_text

Available colors: `"black"`, `"red"`, `"green"`, `"yellow"`, `"blue"`, `"magenta"`, `"cyan"`, `"white"`, `"gray"`

```lua
tabular.canvas.add_styled_text("Error!", "red", nil, true)  -- Red, bold
tabular.canvas.add_styled_text("Success", "green", nil, false)  -- Green
tabular.canvas.add_styled_text("Highlight", "black", "yellow", false)  -- Black on yellow
```

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

### Column Statistics with Canvas

Displays statistics for the current column using the canvas:

```lua
return {
    name = "colstats",
    run = function()
        local col = tabular.ctx.cursor_col
        local col_type = tabular.get_column_type(col)

        tabular.canvas.clear()
        tabular.canvas.set_title("Column Statistics")

        if col_type == "numeric" then
            local sum, count, min, max = 0, 0, nil, nil
            for row = 1, tabular.ctx.row_count do
                local val = tonumber(tabular.get_cell(row, col))
                if val then
                    sum = sum + val
                    count = count + 1
                    min = min and math.min(min, val) or val
                    max = max and math.max(max, val) or val
                end
            end

            tabular.canvas.add_header("Numeric Column")
            tabular.canvas.add_text("Count: " .. count)
            tabular.canvas.add_text("Sum: " .. sum)
            if count > 0 then
                tabular.canvas.add_text("Average: " .. (sum / count))
                tabular.canvas.add_text("Min: " .. min)
                tabular.canvas.add_text("Max: " .. max)
            end
        else
            tabular.canvas.add_header("Text Column")
            local count = 0
            for row = 1, tabular.ctx.row_count do
                if tabular.get_cell(row, col) ~= "" then
                    count = count + 1
                end
            end
            tabular.canvas.add_text("Non-empty cells: " .. count)
        end

        tabular.canvas.show()
    end
}
```

### Selection Sum

Sums values in the current visual selection:

```lua
return {
    name = "selsum",
    run = function()
        local sel = tabular.get_selection()
        if not sel then
            tabular.set_message("No selection - use visual mode first")
            return
        end

        local data = tabular.get_range(sel.start_row, sel.start_col, sel.end_row, sel.end_col)
        local sum = 0
        for _, row in ipairs(data) do
            for _, cell in ipairs(row) do
                local val = tonumber(cell)
                if val then sum = sum + val end
            end
        end

        tabular.set_message("Selection sum: " .. sum)
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

## Function Plugins

In addition to command plugins, Tabular supports **function plugins** that extend the formula system. Function plugins are called from cell formulas (e.g. `=SIN(A1)`) and return a single value. They cannot mutate cells or use the canvas.

### Structure

A function plugin returns a table with `type = "function"` and a `functions` list declaring which functions it provides. Each function name maps to a Lua function that receives an `args` table of evaluated values and returns a single number, string, or boolean.

```lua
return {
    name = "trig",
    type = "function",
    functions = {"sin", "cos", "tan"},
    sin = function(args)
        return math.sin(args[1])
    end,
    cos = function(args)
        return math.cos(args[1])
    end,
    tan = function(args)
        return math.tan(args[1])
    end,
}
```

A single plugin file can define multiple functions. The `functions` list controls which names are registered — each entry must match a function field on the same table.

### Usage in Formulas

Once loaded, function plugins are available in any cell formula:

```
=SIN(3.14159)
=COS(A1)
=SIN(A1) + COS(B1)
```

Function names are case-insensitive in formulas (`sin`, `SIN`, and `Sin` all work).

### Arguments

Arguments are passed as a Lua table. Cell references and expressions are evaluated before the plugin receives them:

| Formula | args[1] | args[2] |
|---------|---------|---------|
| `=MYFUNC(5)` | `5` | — |
| `=MYFUNC(A1)` | value of A1 | — |
| `=MYFUNC(A1+B1, 10)` | sum of A1+B1 | `10` |

Arguments can be numbers (int or float), strings, or booleans.

### Return Values

A function plugin must return one of:

- **number** — stored as Int or Float
- **string** — stored as Str
- **boolean** — stored as Bool

Returning `nil` or a table is an error.

### Differences from Command Plugins

| | Command Plugin | Function Plugin |
|-|----------------|-----------------|
| **Triggered by** | `:command` in command mode | `=FUNC()` in a cell formula |
| **Defined with** | `run` function | Named functions + `functions` list |
| **Can mutate cells** | Yes | No |
| **Can use canvas** | Yes | No |
| **Can insert/delete rows/cols** | Yes | No |
| **Returns** | Actions (set_cell, etc.) | A single value |
| **Multiple per file** | No (one command per plugin) | Yes (one file can define many functions) |

### Example: Unit Conversion

```lua
return {
    name = "convert",
    type = "function",
    functions = {"mi2km", "km2mi", "f2c", "c2f"},
    mi2km = function(args)
        return args[1] * 1.60934
    end,
    km2mi = function(args)
        return args[1] / 1.60934
    end,
    f2c = function(args)
        return (args[1] - 32) * 5 / 9
    end,
    c2f = function(args)
        return args[1] * 9 / 5 + 32
    end,
}
```

Usage: `=MI2KM(A1)`, `=F2C(98.6)`

## Notes

- Plugin changes are integrated with the undo system
- Plugins can read values written by `set_cell` in the same execution via `get_cell`
- Row/column insertions and deletions take effect after the plugin completes
- Invalid operations (out of bounds, etc.) are silently ignored
- Canvas output is view-only and not recorded in undo history
- Persistent data is stored per-key in `~/.config/tabular/data/`
- Function plugins participate in formula evaluation via `:calc` — they are called during the normal calculation pass
