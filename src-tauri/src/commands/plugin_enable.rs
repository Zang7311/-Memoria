// 《铃·记忆体》AI-5 插件命令：启用/禁用插件
use crate::error::AppError;
use crate::types::{Plugin, TogglePluginRequest};

/// 启用插件
#[tauri::command]
pub fn enable_plugin(req: TogglePluginRequest) -> Result<Plugin, AppError> {
    let plugin = crate::plugin::with_manager(|m| m.set_enabled(&req.plugin_id, true))?;
    Ok(plugin)
}

/// 禁用插件
#[tauri::command]
pub fn disable_plugin(req: TogglePluginRequest) -> Result<Plugin, AppError> {
    let plugin = crate::plugin::with_manager(|m| m.set_enabled(&req.plugin_id, false))?;
    Ok(plugin)
}
