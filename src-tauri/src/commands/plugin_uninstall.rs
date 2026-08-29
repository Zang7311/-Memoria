// 《铃·记忆体》AI-5 插件命令：卸载插件
use crate::error::AppError;
use crate::types::UninstallPluginRequest;

/// 卸载插件（删除插件目录 + 从注册表移除；内置插件不可卸载）
#[tauri::command]
pub fn uninstall_plugin(req: UninstallPluginRequest) -> Result<(), AppError> {
    crate::plugin::with_manager(|m| m.uninstall(&req.plugin_id))?;
    log::info!("uninstall_plugin：{}", req.plugin_id);
    Ok(())
}
