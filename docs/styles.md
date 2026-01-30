# Tabular Style System

Tabular supports customizable color themes through a flexible style system. You can use built-in themes or create your own custom themes using TOML configuration files.

## Built-in Themes

Three themes are included:

- **dark** (default) - Dark background with bright accents
- **light** - Light background suitable for light terminal themes
- **solarized-dark** - Based on the popular Solarized color scheme

Switch themes at runtime with `:theme <name>`:

```
:theme dark
:theme light
:theme solarized-dark
```

List available themes with `:themes`.

## Custom Themes

Create custom themes by writing a TOML file with the theme configuration.

### Theme File Structure

```toml
name = "my-custom-theme"

# Table cells
[cell]
fg = "white"

[cell_cursor]
fg = "white"
bg = "blue"
bold = true

[cell_selection]
fg = "white"
bg = "darkgray"

[cell_match]
fg = "black"
bg = "yellow"

# Headers
[header_col]
fg = "cyan"
bold = true

[header_row]
fg = "green"
bold = true

[row_number]
fg = "darkgray"

[row_number_cursor]
fg = "yellow"
bold = true

# Status bar
[status_bar]
fg = "white"
bg = "darkgray"

[status_mode_normal]
fg = "black"
bg = "blue"
bold = true

[status_mode_insert]
fg = "black"
bg = "green"
bold = true

[status_mode_visual]
fg = "black"
bg = "magenta"
bold = true

[status_mode_command]
fg = "black"
bg = "yellow"
bold = true

# Messages
[message_info]
fg = "white"

[message_warning]
fg = "yellow"

[message_error]
fg = "red"
bold = true

# Command line
[command_line]
fg = "white"

[command_prompt]
fg = "cyan"

# Grid
show_grid = false

[grid]
fg = "darkgray"
```

### Color Specification

Colors can be specified in three formats:

#### Named Colors

Use standard terminal color names:

```toml
fg = "red"
bg = "blue"
```

Available named colors:
- `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `gray`
- `darkgray`, `lightred`, `lightgreen`, `lightyellow`, `lightblue`, `lightmagenta`, `lightcyan`, `white`
- `reset` (use terminal default)

#### RGB Colors

Specify exact RGB values as an array:

```toml
fg = [255, 128, 0]   # Orange
bg = [30, 30, 30]    # Dark gray
```

#### 256-Color Palette

Use the terminal's 256-color palette by index:

```toml
fg = 208   # Orange in 256-color palette
bg = 236   # Dark gray
```

### Style Properties

Each element style supports these properties:

| Property | Type | Description |
|----------|------|-------------|
| `fg` | color | Foreground (text) color |
| `bg` | color | Background color |
| `bold` | bool | Bold text |
| `italic` | bool | Italic text |
| `underline` | bool | Underlined text |
| `dim` | bool | Dimmed/faint text |

All properties are optional. Omitted properties use terminal defaults.

### Styleable Elements

| Element | Description |
|---------|-------------|
| `cell` | Default cell style |
| `cell_cursor` | Cell under cursor |
| `cell_selection` | Selected cells in visual mode |
| `cell_match` | Cells matching search pattern |
| `header_col` | Column header letters (A, B, C...) |
| `header_row` | First row when header mode is on |
| `row_number` | Row numbers on the left |
| `row_number_cursor` | Row number of current row |
| `status_bar` | Status bar background |
| `status_mode_normal` | Mode indicator in normal mode |
| `status_mode_insert` | Mode indicator in insert mode |
| `status_mode_visual` | Mode indicator in visual modes |
| `status_mode_command` | Mode indicator in command/search mode |
| `message_info` | Info messages |
| `message_warning` | Warning messages |
| `message_error` | Error messages |
| `command_line` | Command line text |
| `command_prompt` | Command line prompt (`:` or `/`) |
| `grid` | Grid lines (when enabled) |

### Global Settings

| Setting | Type | Description |
|---------|------|-------------|
| `name` | string | Theme name |
| `show_grid` | bool | Show grid lines by default |

## Example: High Contrast Theme

```toml
name = "high-contrast"

[cell]
fg = "white"

[cell_cursor]
fg = "black"
bg = "white"
bold = true

[cell_selection]
fg = "black"
bg = "cyan"

[cell_match]
fg = "black"
bg = "yellow"
bold = true

[header_col]
fg = "cyan"
bold = true
underline = true

[header_row]
fg = "green"
bold = true

[row_number]
fg = "gray"

[row_number_cursor]
fg = "white"
bold = true

[status_bar]
fg = "black"
bg = "white"

[status_mode_normal]
fg = "white"
bg = "blue"
bold = true

[status_mode_insert]
fg = "white"
bg = "green"
bold = true

[status_mode_visual]
fg = "white"
bg = "red"
bold = true

[status_mode_command]
fg = "black"
bg = "yellow"
bold = true

[message_info]
fg = "white"

[message_warning]
fg = "yellow"
bold = true

[message_error]
fg = "red"
bold = true
underline = true

[command_line]
fg = "white"

[command_prompt]
fg = "cyan"
bold = true

show_grid = true

[grid]
fg = "darkgray"
```

## Example: Solarized Light

```toml
name = "solarized-light"

# Solarized palette
# base03  = [0, 43, 54]
# base02  = [7, 54, 66]
# base01  = [88, 110, 117]
# base00  = [101, 123, 131]
# base0   = [131, 148, 150]
# base1   = [147, 161, 161]
# base2   = [238, 232, 213]
# base3   = [253, 246, 227]
# yellow  = [181, 137, 0]
# orange  = [203, 75, 22]
# red     = [220, 50, 47]
# magenta = [211, 54, 130]
# violet  = [108, 113, 196]
# blue    = [38, 139, 210]
# cyan    = [42, 161, 152]
# green   = [133, 153, 0]

[cell]
fg = [101, 123, 131]

[cell_cursor]
fg = [253, 246, 227]
bg = [38, 139, 210]
bold = true

[cell_selection]
fg = [101, 123, 131]
bg = [238, 232, 213]

[cell_match]
fg = [253, 246, 227]
bg = [181, 137, 0]

[header_col]
fg = [38, 139, 210]
bold = true

[header_row]
fg = [133, 153, 0]
bold = true

[row_number]
fg = [147, 161, 161]

[row_number_cursor]
fg = [181, 137, 0]
bold = true

[status_bar]
fg = [101, 123, 131]
bg = [238, 232, 213]

[status_mode_normal]
fg = [253, 246, 227]
bg = [38, 139, 210]
bold = true

[status_mode_insert]
fg = [253, 246, 227]
bg = [133, 153, 0]
bold = true

[status_mode_visual]
fg = [253, 246, 227]
bg = [211, 54, 130]
bold = true

[status_mode_command]
fg = [253, 246, 227]
bg = [181, 137, 0]
bold = true

[message_info]
fg = [101, 123, 131]

[message_warning]
fg = [203, 75, 22]

[message_error]
fg = [220, 50, 47]
bold = true

[command_line]
fg = [101, 123, 131]

[command_prompt]
fg = [42, 161, 152]

show_grid = true

[grid]
fg = [147, 161, 161]
```
