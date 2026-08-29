// 《铃·记忆体》AI-5 插件命令：列出所有插件
use crate::error::AppError;
use crate::types::ListPluginsResponse;

/// 列出所有已安装插件（含内置插件与终端命令）
#[tauri::command]
pub fn list_plugins() -> Result<ListPluginsResponse, AppError> {
    let plugins = crate::plugin::with_manager(|m| m.list());
    log::info!("list_plugins：共 {} 个插件", plugins.len());
    Ok(ListPluginsResponse { plugins })
}
