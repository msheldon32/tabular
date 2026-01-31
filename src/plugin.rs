use mlua::{Lua, Result as LuaResult, Function, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub struct PluginManager {
    lua: Lua,
    commands: HashMap<String, String>, // command_name -> script content
}

pub struct CommandContext {
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub row_count: usize,
    pub col_count: usize,
}

#[derive(Clone)]
pub enum PluginAction {
    SetCell { row: usize, col: usize, value: String },
    InsertRow { at: usize },
    DeleteRow { at: usize },
    InsertCol { at: usize },
    DeleteCol { at: usize },
}

pub struct PluginResult {
    pub actions: Vec<PluginAction>,
    pub message: Option<String>,
}

impl PluginManager {
    pub fn new() -> Self {
        let lua = Lua::new();
        Self {
            lua,
            commands: HashMap::new(),
        }
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
            if let Some(cmd_name) = name {
                self.commands.insert(cmd_name.clone(), script.to_string());
                return Ok(Some(cmd_name));
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

    pub fn execute(
        &self,
        command: &str,
        args: &[String],
        ctx: &CommandContext,
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

        // Create insert_row function
        let actions_ref2 = actions_table.clone();
        let insert_row_fn = self.lua.create_function(move |lua, at: usize| {
            let action = lua.create_table()?;
            action.set("type", "insert_row")?;
            action.set("at", at)?;
            let len = actions_ref2.len()? + 1;
            actions_ref2.set(len, action)?;
            Ok(())
        })?;

        // Create delete_row function
        let actions_ref3 = actions_table.clone();
        let delete_row_fn = self.lua.create_function(move |lua, at: usize| {
            let action = lua.create_table()?;
            action.set("type", "delete_row")?;
            action.set("at", at)?;
            let len = actions_ref3.len()? + 1;
            actions_ref3.set(len, action)?;
            Ok(())
        })?;

        // Create insert_col function
        let actions_ref4 = actions_table.clone();
        let insert_col_fn = self.lua.create_function(move |lua, at: usize| {
            let action = lua.create_table()?;
            action.set("type", "insert_col")?;
            action.set("at", at)?;
            let len = actions_ref4.len()? + 1;
            actions_ref4.set(len, action)?;
            Ok(())
        })?;

        // Create delete_col function
        let actions_ref5 = actions_table.clone();
        let delete_col_fn = self.lua.create_function(move |lua, at: usize| {
            let action = lua.create_table()?;
            action.set("type", "delete_col")?;
            action.set("at", at)?;
            let len = actions_ref5.len()? + 1;
            actions_ref5.set(len, action)?;
            Ok(())
        })?;

        // Create message holder
        let message_table = self.lua.create_table()?;
        let msg_ref = message_table.clone();
        let set_message_fn = self.lua.create_function(move |_, msg: String| {
            msg_ref.set("msg", msg)?;
            Ok(())
        })?;

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
                    _ => {}
                }
            }
        }

        // Get message if set
        let message: Option<String> = message_table.get("msg").ok();

        Ok(PluginResult { actions, message })
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
