use mlua::{Lua, Result as LuaResult, Function, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::mode::Mode;
use crate::mode::visual::SelectionInfo;
use crate::ui::canvas::CanvasColor;
use crate::numeric::calctype::CalcType;
use crate::util::CalcError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginType {
    Command,
    Function,
}

pub struct PluginManager {
    lua: Lua,
    commands: HashMap<String, String>, // command_name -> script content
    functions: HashMap<String, String>, // function_name -> script content (uppercase keys)
    prompt_results: HashMap<String, String>, // question -> answer for deferred prompts
}

pub struct PluginContext {
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub row_count: usize,
    pub col_count: usize,
    pub selection: SelectionInfo,
}

#[derive(Clone)]
#[allow(dead_code)]
pub enum PluginAction {
    SetCell { row: usize, col: usize, value: String },
    InsertRow { at: usize },
    DeleteRow { at: usize },
    InsertCol { at: usize },
    DeleteCol { at: usize },
    // Canvas actions
    CanvasClear,
    CanvasShow,
    CanvasHide,
    CanvasSetTitle { title: String },
    CanvasAddText { text: String },
    CanvasAddHeader { text: String },
    CanvasAddSeparator,
    CanvasAddBlank,
    CanvasAddStyledText { text: String, fg: Option<CanvasColor>, bg: Option<CanvasColor>, bold: bool },
    CanvasAddImage { rows: Vec<String>, title: Option<String> },
    // Prompt action - requests user input
    PromptRequest { question: String, default: String },
}

pub struct PluginResult {
    pub actions: Vec<PluginAction>,
    pub message: Option<String>
}

impl PluginManager {
    pub fn new() -> Self {
        let lua = Lua::new();
        Self {
            lua,
            commands: HashMap::new(),
            functions: HashMap::new(),
            prompt_results: HashMap::new(),
        }
    }

    /// Store a prompt result for use in the next plugin execution
    #[allow(dead_code)]
    pub fn set_prompt_result(&mut self, question: &str, answer: String) {
        self.prompt_results.insert(question.to_string(), answer);
    }

    /// Clear all prompt results
    #[allow(dead_code)]
    pub fn clear_prompt_results(&mut self) {
        self.prompt_results.clear();
    }

    pub fn load_plugins(&mut self) -> LuaResult<Vec<String>> {
        let mut loaded = Vec::new();

        // Look for plugins in ~/.config/tabular/plugins/
        let plugin_dir = dirs_plugin_path();

        if !plugin_dir.exists() {
            return Ok(loaded);
        }

        if let Ok(entries) = fs::read_dir(&plugin_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "lua") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        match self.register_plugin(&content) {
                            Ok(Some(name)) => loaded.push(name),
                            Ok(None) => {}
                            Err(_) => {}
                        }
                    }
                }
            }
        }

        Ok(loaded)
    }

    fn register_plugin(&mut self, script: &str) -> LuaResult<Option<String>> {
        let chunk = self.lua.load(script);
        let result: Value = chunk.eval()?;

        if let Value::Table(table) = result {
            let name: Option<String> = table.get("name").ok();
            let plugin_type: Option<String> = table.get("type").ok();
            if let Some(plugin_name) = name {
                let ptype = match plugin_type.as_deref() {
                    Some("function") => PluginType::Function,
                    _ => PluginType::Command,
                };
                match ptype {
                    PluginType::Function => {
                        self.functions.insert(plugin_name.to_uppercase(), script.to_string());
                    }
                    PluginType::Command => {
                        self.commands.insert(plugin_name.clone(), script.to_string());
                    }
                }
                return Ok(Some(plugin_name));
            }
        }
        Ok(None)
    }

    pub fn has_command(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    pub fn list_commands(&self) -> Vec<&String> {
        self.commands.keys().collect()
    }

    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(&name.to_uppercase())
    }

    pub fn list_functions(&self) -> Vec<&String> {
        self.functions.keys().collect()
    }

    /// Create a generic function that is handled by dispatch
    fn add_default_command(&self, actions_table: mlua::Table, name: String) -> LuaResult<mlua::Function> {
        self.lua.create_function(move |lua, at: usize| {
            let action = lua.create_table()?;
            action.set("type", name.clone())?;
            action.set("at", at)?;
            let len = actions_table.len()? + 1;
            actions_table.set(len, action)?;
            Ok(())
        })
    }

    /// Execute a function plugin with evaluated arguments.
    /// Returns a single CalcType value. Function plugins cannot mutate cells.
    pub fn execute_function(
        &self,
        name: &str,
        args: &[CalcType],
    ) -> Result<CalcType, CalcError> {
        let upper_name = name.to_uppercase();
        let script = match self.functions.get(&upper_name) {
            Some(s) => s,
            None => return Err(CalcError::EvalError(format!("Unknown function plugin: {}", name))),
        };

        // Create args table for Lua
        let args_table = self.lua.create_table().map_err(lua_to_calc_error)?;
        for (i, arg) in args.iter().enumerate() {
            match arg {
                CalcType::Int(n) => args_table.set(i + 1, *n).map_err(lua_to_calc_error)?,
                CalcType::Float(f) => args_table.set(i + 1, *f).map_err(lua_to_calc_error)?,
                CalcType::Str(s) => args_table.set(i + 1, s.as_str()).map_err(lua_to_calc_error)?,
                CalcType::Bool(b) => args_table.set(i + 1, *b).map_err(lua_to_calc_error)?,
            }
        }

        // Load and execute the plugin
        let chunk = self.lua.load(script);
        let plugin: Value = chunk.eval().map_err(lua_to_calc_error)?;

        if let Value::Table(table) = plugin {
            if let Ok(compute_fn) = table.get::<Function>("compute") {
                let result: Value = compute_fn.call(args_table).map_err(lua_to_calc_error)?;
                return lua_value_to_calctype(result);
            }
            return Err(CalcError::EvalError(format!(
                "Function plugin '{}' has no compute() function", name
            )));
        }

        Err(CalcError::EvalError(format!(
            "Function plugin '{}' did not return a table", name
        )))
    }

    pub fn execute(
        &self,
        command: &str,
        args: &[String],
        ctx: &PluginContext,
        get_cell: impl Fn(usize, usize) -> Option<String>,
    ) -> LuaResult<PluginResult> {
        let script = match self.commands.get(command) {
            Some(s) => s,
            None => return Ok(PluginResult {
                actions: vec![],
                message: Some(format!("Unknown plugin command: {}", command))
            }),
        };

        // Create the context table
        let ctx_table = self.lua.create_table()?;
        ctx_table.set("cursor_row", ctx.cursor_row + 1)?; // 1-indexed for Lua
        ctx_table.set("cursor_col", ctx.cursor_col + 1)?;
        ctx_table.set("row_count", ctx.row_count)?;
        ctx_table.set("col_count", ctx.col_count)?;

        // Add selection info (1-indexed for Lua)
        if ctx.selection.mode.is_visual() {
            let sel_table = self.lua.create_table()?;
            sel_table.set("start_row", ctx.selection.start_row + 1)?;
            sel_table.set("start_col", ctx.selection.start_col + 1)?;
            sel_table.set("end_row", ctx.selection.end_row + 1)?;
            sel_table.set("end_col", ctx.selection.end_col + 1)?;
            sel_table.set("mode", match &ctx.selection.mode {
                Mode::Visual => "visual",
                Mode::VisualRow => "visual_row",
                Mode::VisualCol => "visual_col",
                _other => "none",
            })?;
            ctx_table.set("selection", sel_table)?;
        }

        // Create args table
        let args_table = self.lua.create_table()?;
        for (i, arg) in args.iter().enumerate() {
            args_table.set(i + 1, arg.as_str())?;
        }

        // Collect all cell values upfront for the get_cell function
        // This avoids lifetime issues with closures
        let mut cell_cache: HashMap<(usize, usize), String> = HashMap::new();
        for row in 0..ctx.row_count {
            for col in 0..ctx.col_count {
                if let Some(val) = get_cell(row, col) {
                    cell_cache.insert((row, col), val);
                }
            }
        }
        let cell_cache_for_range = cell_cache.clone();
        let cell_cache_for_type = cell_cache.clone();

        // Create an overlay table to hold pending writes (visible to Lua, not to actual data)
        let overlay = self.lua.create_table()?;

        // Create get_cell function that checks overlay first, then falls back to cache
        let overlay_for_get = overlay.clone();
        let get_cell_fn = self.lua.create_function(move |_, (row, col): (usize, usize)| {
            // First check the overlay for pending writes
            let overlay_key = format!("{}:{}", row, col);
            if let Ok(val) = overlay_for_get.get::<String>(overlay_key.as_str()) {
                return Ok(val);
            }
            // Fall back to original cache (convert to 0-indexed)
            let key = (row.saturating_sub(1), col.saturating_sub(1));
            Ok(cell_cache.get(&key).cloned().unwrap_or_default())
        })?;

        // Create actions table to collect results
        let actions_table = self.lua.create_table()?;

        // Create set_cell function that writes to overlay AND records the action
        let overlay_for_set = overlay.clone();
        let actions_ref = actions_table.clone();
        let set_cell_fn = self.lua.create_function(move |lua, (row, col, value): (usize, usize, String)| {
            // Store in overlay so subsequent get_cell calls see it
            let overlay_key = format!("{}:{}", row, col);
            overlay_for_set.set(overlay_key, value.clone())?;

            // Record the action for later application
            let action = lua.create_table()?;
            action.set("type", "set_cell")?;
            action.set("row", row)?;
            action.set("col", col)?;
            action.set("value", value)?;
            let len = actions_ref.len()? + 1;
            actions_ref.set(len, action)?;
            Ok(())
        })?;

        let insert_row_fn = self.add_default_command(actions_table.clone(), "insert_row".to_string())?;
        let insert_col_fn = self.add_default_command(actions_table.clone(), "insert_col".to_string())?;
        let delete_row_fn = self.add_default_command(actions_table.clone(), "delete_row".to_string())?;
        let delete_col_fn = self.add_default_command(actions_table.clone(), "delete_col".to_string())?;
        let canvas_clear_fn = self.add_default_command(actions_table.clone(), "canvas_clear".to_string())?;
        let canvas_show_fn = self.add_default_command(actions_table.clone(), "canvas_show".to_string())?;
        let canvas_hide_fn = self.add_default_command(actions_table.clone(), "canvas_hide".to_string())?;
        let canvas_set_title_fn = self.add_default_command(actions_table.clone(), "canvas_set_title".to_string())?;
        let canvas_set_text_fn = self.add_default_command(actions_table.clone(), "canvas_set_text".to_string())?;
        let canvas_add_text_fn = self.add_default_command(actions_table.clone(), "canvas_add_text".to_string())?;
        let canvas_add_separator_fn = self.add_default_command(actions_table.clone(), "canvas_add_separator".to_string())?;
        let canvas_add_blank_fn = self.add_default_command(actions_table.clone(), "canvas_add_blank".to_string())?;
        let canvas_add_header_fn = self.add_default_command(actions_table.clone(), "canvas_add_header".to_string())?;

        // Create message holder
        let message_table = self.lua.create_table()?;
        let msg_ref = message_table.clone();
        let set_message_fn = self.lua.create_function(move |_, msg: String| {
            msg_ref.set("msg", msg)?;
            Ok(())
        })?;

        // canvas.add_styled_text(text, fg, bg, bold) function
        let actions_ref14 = actions_table.clone();
        
        let canvas_add_styled_text_fn = self.lua.create_function(move |lua, (text, fg, bg, bold): (String, Option<String>, Option<String>, Option<bool>)| {
            let action = lua.create_table()?;
            action.set("type", "canvas_add_styled_text")?;
            action.set("text", text)?;
            if let Some(fg_color) = fg {
                action.set("fg", fg_color)?;
            }
            if let Some(bg_color) = bg {
                action.set("bg", bg_color)?;
            }
            action.set("bold", bold.unwrap_or(false))?;
            let len = actions_ref14.len()? + 1;
            actions_ref14.set(len, action)?;
            Ok(())
        })?;

        // get_selection() function - returns selection bounds or nil
        let sel_start_row = ctx.selection.start_row;
        let sel_start_col = ctx.selection.start_col;
        let sel_end_row = ctx.selection.end_row;
        let sel_end_col = ctx.selection.end_col;
        let sel_is_visual = ctx.selection.mode.is_visual();
        let sel_mode = ctx.selection.mode.clone();
        let get_selection_fn = self.lua.create_function(move |lua, ()| {
            if !sel_is_visual {
                return Ok(Value::Nil);
            }
            let result = lua.create_table()?;
            result.set("start_row", sel_start_row + 1)?;
            result.set("start_col", sel_start_col + 1)?;
            result.set("end_row", sel_end_row + 1)?;
            result.set("end_col", sel_end_col + 1)?;
            result.set("mode", match &sel_mode {
                Mode::Visual => "visual",
                Mode::VisualRow => "visual_row",
                Mode::VisualCol => "visual_col",
                _other => "none",
            })?;
            Ok(Value::Table(result))
        })?;

        // get_range(r1, c1, r2, c2) function - returns 2D table of values
        let get_range_fn = self.lua.create_function(move |lua, (r1, c1, r2, c2): (usize, usize, usize, usize)| {
            let result = lua.create_table()?;
            let start_row = r1.saturating_sub(1);
            let start_col = c1.saturating_sub(1);
            let end_row = r2.saturating_sub(1);
            let end_col = c2.saturating_sub(1);

            for (i, row) in (start_row..=end_row).enumerate() {
                let row_table = lua.create_table()?;
                for (j, col) in (start_col..=end_col).enumerate() {
                    let value = cell_cache_for_range.get(&(row, col)).cloned().unwrap_or_default();
                    row_table.set(j + 1, value)?;
                }
                result.set(i + 1, row_table)?;
            }
            Ok(result)
        })?;

        // get_column_type(col) function - returns "numeric" or "text"
        let type_row_count = ctx.row_count;
        let get_column_type_fn = self.lua.create_function(move |_, col: usize| {
            let col_idx = col.saturating_sub(1);
            let mut numeric_count = 0usize;
            let mut total_count = 0usize;

            for row in 0..type_row_count {
                if let Some(value) = cell_cache_for_type.get(&(row, col_idx)) {
                    if !value.is_empty() {
                        total_count += 1;
                        if value.parse::<f64>().is_ok() {
                            numeric_count += 1;
                        }
                    }
                }
            }

            // If more than half are numeric, consider it numeric
            if total_count > 0 && numeric_count * 2 >= total_count {
                Ok("numeric".to_string())
            } else {
                Ok("text".to_string())
            }
        })?;

        // prompt(question, default) function - requests user input
        let prompt_results = self.prompt_results.clone();
        let actions_ref15 = actions_table.clone();
        let prompt_fn = self.lua.create_function(move |lua, (question, default): (String, Option<String>)| {
            // Check if we have a stored result for this question
            if let Some(answer) = prompt_results.get(&question) {
                return Ok(Value::String(lua.create_string(answer)?));
            }

            // No result yet, queue a prompt request and return nil
            let action = lua.create_table()?;
            action.set("type", "prompt_request")?;
            action.set("question", question)?;
            action.set("default", default.unwrap_or_default())?;
            let len = actions_ref15.len()? + 1;
            actions_ref15.set(len, action)?;
            Ok(Value::Nil)
        })?;

        // save_data(key, value) function - persistent plugin storage
        let actions_ref16 = actions_table.clone();
        let save_data_fn = self.lua.create_function(move |lua, (key, value): (String, String)| {
            let action = lua.create_table()?;
            action.set("type", "save_data")?;
            action.set("key", key)?;
            action.set("value", value)?;
            let len = actions_ref16.len()? + 1;
            actions_ref16.set(len, action)?;
            Ok(())
        })?;

        // load_data(key) function - load from persistent storage
        let load_data_fn = self.lua.create_function(move |lua, key: String| {
            // Inline the load logic to avoid closure scoping issues
            let safe_key: String = key.chars()
                .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
                .collect();

            let data_dir = if let Some(home) = std::env::var_os("HOME") {
                std::path::PathBuf::from(home).join(".config/tabular/data")
            } else {
                std::path::PathBuf::from(".config/tabular/data")
            };

            let path = data_dir.join(&safe_key);
            match std::fs::read_to_string(&path) {
                Ok(value) => Ok(Value::String(lua.create_string(&value)?)),
                Err(_) => Ok(Value::Nil),
            }
        })?;

        // Create canvas sub-API
        let canvas_api = self.lua.create_table()?;
        canvas_api.set("clear", canvas_clear_fn)?;
        canvas_api.set("show", canvas_show_fn)?;
        canvas_api.set("hide", canvas_hide_fn)?;
        canvas_api.set("set_title", canvas_set_title_fn)?;
        canvas_api.set("add_text", canvas_add_text_fn)?;
        canvas_api.set("add_header", canvas_add_header_fn)?;
        canvas_api.set("add_separator", canvas_add_separator_fn)?;
        canvas_api.set("add_blank", canvas_add_blank_fn)?;
        canvas_api.set("add_styled_text", canvas_add_styled_text_fn)?;

        // Create tabular API table
        let api = self.lua.create_table()?;
        api.set("ctx", ctx_table)?;
        api.set("args", args_table)?;
        api.set("get_cell", get_cell_fn)?;
        api.set("set_cell", set_cell_fn)?;
        api.set("insert_row", insert_row_fn)?;
        api.set("delete_row", delete_row_fn)?;
        api.set("insert_col", insert_col_fn)?;
        api.set("delete_col", delete_col_fn)?;
        api.set("set_message", set_message_fn)?;
        api.set("canvas", canvas_api)?;
        api.set("get_selection", get_selection_fn)?;
        api.set("get_range", get_range_fn)?;
        api.set("get_column_type", get_column_type_fn)?;
        api.set("prompt", prompt_fn)?;
        api.set("save_data", save_data_fn)?;
        api.set("load_data", load_data_fn)?;

        self.lua.globals().set("tabular", api)?;

        // Load and execute the plugin
        let chunk = self.lua.load(script);
        let plugin: Value = chunk.eval()?;

        if let Value::Table(table) = plugin {
            if let Ok(run_fn) = table.get::<Function>("run") {
                run_fn.call::<()>(())?;
            }
        }

        // Collect actions from Lua
        let mut actions = Vec::new();
        for i in 1..=actions_table.len()? {
            if let Ok(action) = actions_table.get::<mlua::Table>(i) {
                let action_type: String = action.get("type")?;
                match action_type.as_str() {
                    "set_cell" => {
                        let row: usize = action.get("row")?;
                        let col: usize = action.get("col")?;
                        let value: String = action.get("value")?;
                        actions.push(PluginAction::SetCell {
                            row: row.saturating_sub(1),
                            col: col.saturating_sub(1),
                            value,
                        });
                    }
                    "insert_row" => {
                        let at: usize = action.get("at")?;
                        actions.push(PluginAction::InsertRow { at: at.saturating_sub(1) });
                    }
                    "delete_row" => {
                        let at: usize = action.get("at")?;
                        actions.push(PluginAction::DeleteRow { at: at.saturating_sub(1) });
                    }
                    "insert_col" => {
                        let at: usize = action.get("at")?;
                        actions.push(PluginAction::InsertCol { at: at.saturating_sub(1) });
                    }
                    "delete_col" => {
                        let at: usize = action.get("at")?;
                        actions.push(PluginAction::DeleteCol { at: at.saturating_sub(1) });
                    }
                    // Canvas actions
                    "canvas_clear" => {
                        actions.push(PluginAction::CanvasClear);
                    }
                    "canvas_show" => {
                        actions.push(PluginAction::CanvasShow);
                    }
                    "canvas_hide" => {
                        actions.push(PluginAction::CanvasHide);
                    }
                    "canvas_set_title" => {
                        let title: String = action.get("title")?;
                        actions.push(PluginAction::CanvasSetTitle { title });
                    }
                    "canvas_add_text" => {
                        let text: String = action.get("text")?;
                        actions.push(PluginAction::CanvasAddText { text });
                    }
                    "canvas_add_header" => {
                        let text: String = action.get("text")?;
                        actions.push(PluginAction::CanvasAddHeader { text });
                    }
                    "canvas_add_separator" => {
                        actions.push(PluginAction::CanvasAddSeparator);
                    }
                    "canvas_add_blank" => {
                        actions.push(PluginAction::CanvasAddBlank);
                    }
                    "canvas_add_styled_text" => {
                        let text: String = action.get("text")?;
                        let fg: Option<String> = action.get("fg").ok();
                        let bg: Option<String> = action.get("bg").ok();
                        let bold: bool = action.get("bold").unwrap_or(false);
                        actions.push(PluginAction::CanvasAddStyledText {
                            text,
                            fg: fg.and_then(|s| CanvasColor::from_str(&s)),
                            bg: bg.and_then(|s| CanvasColor::from_str(&s)),
                            bold,
                        });
                    }
                    "prompt_request" => {
                        let question: String = action.get("question")?;
                        let default: String = action.get("default").unwrap_or_default();
                        actions.push(PluginAction::PromptRequest { question, default });
                    }
                    "save_data" => {
                        let key: String = action.get("key")?;
                        let value: String = action.get("value")?;
                        // Inline save logic
                        let safe_key: String = key.chars()
                            .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
                            .collect();
                        let data_dir = if let Some(home) = std::env::var_os("HOME") {
                            PathBuf::from(home).join(".config/tabular/data")
                        } else {
                            PathBuf::from(".config/tabular/data")
                        };
                        let _ = fs::create_dir_all(&data_dir);
                        let path = data_dir.join(&safe_key);
                        let _ = fs::write(&path, &value);
                    }
                    _ => {}
                }
            }
        }

        // Get message if set
        let message: Option<String> = message_table.get("msg").ok();

        Ok(PluginResult { actions, message })
    }
}

fn lua_to_calc_error(e: mlua::Error) -> CalcError {
    CalcError::EvalError(format!("Plugin error: {}", e))
}

fn lua_value_to_calctype(value: Value) -> Result<CalcType, CalcError> {
    match value {
        Value::Integer(n) => Ok(CalcType::Int(n)),
        Value::Number(f) => Ok(CalcType::Float(f)),
        Value::String(s) => Ok(CalcType::Str(
            s.to_str().map_err(|e| CalcError::EvalError(format!("Invalid UTF-8: {}", e)))?.to_string()
        )),
        Value::Boolean(b) => Ok(CalcType::Bool(b)),
        _ => Err(CalcError::EvalError(
            "Function plugin must return a number, string, or boolean".to_string()
        )),
    }
}

fn dirs_plugin_path() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".config/tabular/plugins")
    } else {
        PathBuf::from(".config/tabular/plugins")
    }
}

pub fn plugin_dir() -> PathBuf {
    dirs_plugin_path()
}

