use mlua::{Function, Result as LuaResult, Value};
use std::path::PathBuf;

use crate::mode::Mode;

use super::{PluginContext, PluginManager};

pub(crate) fn data_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".config/tabular/data")
    } else {
        PathBuf::from(".config/tabular/data")
    }
}

pub(crate) fn sanitize_key(key: &str) -> String {
    key.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}

impl PluginManager {
    /// Create a command that takes a usize argument (insert_row, delete_row, etc.)
    fn add_usize_command(&self, actions_table: mlua::Table, name: String) -> LuaResult<Function> {
        self.lua.create_function(move |lua, at: usize| {
            let action = lua.create_table()?;
            action.set("type", name.clone())?;
            action.set("at", at)?;
            let len = actions_table.len()? + 1;
            actions_table.set(len, action)?;
            Ok(())
        })
    }

    /// Create a command that takes no arguments (canvas_clear, canvas_show, etc.)
    fn add_void_command(&self, actions_table: mlua::Table, name: String) -> LuaResult<Function> {
        self.lua.create_function(move |_, ()| {
            let action = actions_table.raw_lua().create_table()?;
            action.set("type", name.clone())?;
            let len = actions_table.len()? + 1;
            actions_table.set(len, action)?;
            Ok(())
        })
    }

    /// Create a command that takes a string argument (canvas_set_title, canvas_add_text, etc.)
    fn add_string_command(&self, actions_table: mlua::Table, name: String) -> LuaResult<Function> {
        self.lua.create_function(move |_, text: String| {
            let action = actions_table.raw_lua().create_table()?;
            action.set("type", name.clone())?;
            action.set("text", text)?;
            let len = actions_table.len()? + 1;
            actions_table.set(len, action)?;
            Ok(())
        })
    }

    pub(crate) fn build_api(
        &self,
        args: &[String],
        ctx: &PluginContext,
        get_cell: impl Fn(usize, usize) -> Option<String>,
    ) -> LuaResult<(mlua::Table, mlua::Table)> {
        let ctx_table = self.build_context_table(ctx)?;
        let args_table = self.lua.create_table()?;
        for (i, arg) in args.iter().enumerate() {
            args_table.set(i + 1, arg.as_str())?;
        }

        let cell_store = self.build_cell_store(ctx, &get_cell)?;
        let actions_table = self.lua.create_table()?;

        let (get_cell_fn, set_cell_fn, get_range_fn, get_column_type_fn) =
            self.build_cell_fns(&cell_store, &actions_table, ctx.row_count)?;

        let insert_row_fn = self.add_usize_command(actions_table.clone(), "insert_row".to_string())?;
        let insert_col_fn = self.add_usize_command(actions_table.clone(), "insert_col".to_string())?;
        let delete_row_fn = self.add_usize_command(actions_table.clone(), "delete_row".to_string())?;
        let delete_col_fn = self.add_usize_command(actions_table.clone(), "delete_col".to_string())?;

        let (set_message_fn, message_table) = self.build_message_fn()?;
        let get_selection_fn = self.build_selection_fn(ctx)?;
        let canvas_api = self.build_canvas_api(&actions_table)?;
        let prompt_fn = self.build_prompt_fn(&actions_table)?;
        let (save_data_fn, load_data_fn) = self.build_data_fns(&actions_table)?;

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

        Ok((actions_table, message_table))
    }

    fn build_context_table(&self, ctx: &PluginContext) -> LuaResult<mlua::Table> {
        let ctx_table = self.lua.create_table()?;
        ctx_table.set("cursor_row", ctx.cursor_row + 1)?;
        ctx_table.set("cursor_col", ctx.cursor_col + 1)?;
        ctx_table.set("row_count", ctx.row_count)?;
        ctx_table.set("col_count", ctx.col_count)?;

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

        Ok(ctx_table)
    }

    fn build_cell_store(
        &self,
        ctx: &PluginContext,
        get_cell: &impl Fn(usize, usize) -> Option<String>,
    ) -> LuaResult<mlua::Table> {
        let cell_store = self.lua.create_table()?;
        for row in 0..ctx.row_count {
            for col in 0..ctx.col_count {
                if let Some(val) = get_cell(row, col) {
                    let key = format!("{}:{}", row, col);
                    cell_store.set(key, val)?;
                }
            }
        }
        Ok(cell_store)
    }

    fn build_cell_fns(
        &self,
        store: &mlua::Table,
        actions: &mlua::Table,
        row_count: usize,
    ) -> LuaResult<(Function, Function, Function, Function)> {
        // get_cell (1-indexed from Lua)
        let store_for_get = store.clone();
        let get_cell_fn = self.lua.create_function(move |_, (row, col): (usize, usize)| {
            let key = format!("{}:{}", row.saturating_sub(1), col.saturating_sub(1));
            Ok(store_for_get.get::<String>(key.as_str()).unwrap_or_default())
        })?;

        // set_cell updates store AND records action
        let store_for_set = store.clone();
        let actions_ref = actions.clone();
        let set_cell_fn = self.lua.create_function(move |lua, (row, col, value): (usize, usize, String)| {
            let key = format!("{}:{}", row.saturating_sub(1), col.saturating_sub(1));
            store_for_set.set(key, value.clone())?;

            let action = lua.create_table()?;
            action.set("type", "set_cell")?;
            action.set("row", row)?;
            action.set("col", col)?;
            action.set("value", value)?;
            let len = actions_ref.len()? + 1;
            actions_ref.set(len, action)?;
            Ok(())
        })?;

        // get_range(r1, c1, r2, c2)
        let store_for_range = store.clone();
        let get_range_fn = self.lua.create_function(move |lua, (r1, c1, r2, c2): (usize, usize, usize, usize)| {
            let result = lua.create_table()?;
            let start_row = r1.saturating_sub(1);
            let start_col = c1.saturating_sub(1);
            let end_row = r2.saturating_sub(1);
            let end_col = c2.saturating_sub(1);

            for (i, row) in (start_row..=end_row).enumerate() {
                let row_table = lua.create_table()?;
                for (j, col) in (start_col..=end_col).enumerate() {
                    let key = format!("{}:{}", row, col);
                    let value: String = store_for_range.get::<String>(key.as_str()).unwrap_or_default();
                    row_table.set(j + 1, value)?;
                }
                result.set(i + 1, row_table)?;
            }
            Ok(result)
        })?;

        // get_column_type(col)
        let store_for_type = store.clone();
        let get_column_type_fn = self.lua.create_function(move |_, col: usize| {
            let col_idx = col.saturating_sub(1);
            let mut numeric_count = 0usize;
            let mut total_count = 0usize;

            for row in 0..row_count {
                let key = format!("{}:{}", row, col_idx);
                if let Ok(value) = store_for_type.get::<String>(key.as_str()) {
                    if !value.is_empty() {
                        total_count += 1;
                        if value.parse::<f64>().is_ok() {
                            numeric_count += 1;
                        }
                    }
                }
            }

            if total_count > 0 && numeric_count * 2 >= total_count {
                Ok("numeric".to_string())
            } else {
                Ok("text".to_string())
            }
        })?;

        Ok((get_cell_fn, set_cell_fn, get_range_fn, get_column_type_fn))
    }

    fn build_selection_fn(&self, ctx: &PluginContext) -> LuaResult<Function> {
        let sel_start_row = ctx.selection.start_row;
        let sel_start_col = ctx.selection.start_col;
        let sel_end_row = ctx.selection.end_row;
        let sel_end_col = ctx.selection.end_col;
        let sel_is_visual = ctx.selection.mode.is_visual();
        let sel_mode = ctx.selection.mode;
        self.lua.create_function(move |lua, ()| {
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
        })
    }

    fn build_canvas_api(&self, actions: &mlua::Table) -> LuaResult<mlua::Table> {
        let canvas_clear_fn = self.add_void_command(actions.clone(), "canvas_clear".to_string())?;
        let canvas_show_fn = self.add_void_command(actions.clone(), "canvas_show".to_string())?;
        let canvas_hide_fn = self.add_void_command(actions.clone(), "canvas_hide".to_string())?;
        let canvas_set_title_fn = self.add_string_command(actions.clone(), "canvas_set_title".to_string())?;
        let canvas_add_text_fn = self.add_string_command(actions.clone(), "canvas_add_text".to_string())?;
        let canvas_add_header_fn = self.add_string_command(actions.clone(), "canvas_add_header".to_string())?;
        let canvas_add_separator_fn = self.add_void_command(actions.clone(), "canvas_add_separator".to_string())?;
        let canvas_add_blank_fn = self.add_void_command(actions.clone(), "canvas_add_blank".to_string())?;

        let actions_ref = actions.clone();
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
            let len = actions_ref.len()? + 1;
            actions_ref.set(len, action)?;
            Ok(())
        })?;

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

        Ok(canvas_api)
    }

    fn build_data_fns(&self, actions: &mlua::Table) -> LuaResult<(Function, Function)> {
        let save_data_actions_ref = actions.clone();
        let save_data_fn = self.lua.create_function(move |lua, (key, value): (String, String)| {
            let action = lua.create_table()?;
            action.set("type", "save_data")?;
            action.set("key", key)?;
            action.set("value", value)?;
            let len = save_data_actions_ref.len()? + 1;
            save_data_actions_ref.set(len, action)?;
            Ok(())
        })?;

        let load_data_fn = self.lua.create_function(move |lua, key: String| {
            let safe_key = sanitize_key(&key);
            let dir = data_dir();
            let path = dir.join(&safe_key);
            match std::fs::read_to_string(&path) {
                Ok(value) => Ok(Value::String(lua.create_string(&value)?)),
                Err(_) => Ok(Value::Nil),
            }
        })?;

        Ok((save_data_fn, load_data_fn))
    }

    fn build_prompt_fn(&self, actions: &mlua::Table) -> LuaResult<Function> {
        let prompt_results = self.prompt_results.clone();
        let prompt_actions_ref = actions.clone();
        self.lua.create_function(move |lua, (question, default): (String, Option<String>)| {
            if let Some(answer) = prompt_results.get(&question) {
                return Ok(Value::String(lua.create_string(answer)?));
            }

            let action = lua.create_table()?;
            action.set("type", "prompt_request")?;
            action.set("question", question)?;
            action.set("default", default.unwrap_or_default())?;
            let len = prompt_actions_ref.len()? + 1;
            prompt_actions_ref.set(len, action)?;
            Ok(Value::Nil)
        })
    }

    fn build_message_fn(&self) -> LuaResult<(Function, mlua::Table)> {
        let message_table = self.lua.create_table()?;
        let msg_ref = message_table.clone();
        let set_message_fn = self.lua.create_function(move |_, msg: String| {
            msg_ref.set("msg", msg)?;
            Ok(())
        })?;
        Ok((set_message_fn, message_table))
    }
}
