// 《铃·记忆体》AI-5 插件命令：权限粒度控制（类似 Android 权限）
use crate::error::AppError;
use crate::types::{Plugin, SetPermissionRequest};

/// 授予/收回插件某个权限（高风险 system 默认禁用，需用户显式开启）
#[tauri::command]
pub fn set_plugin_permission(req: SetPermissionRequest) -> Result<Plugin, AppError> {
    let plugin =
        crate::plugin::with_manager(|m| m.set_permission(&req.plugin_id, &req.permission, req.allow))?;
    Ok(plugin)
}
