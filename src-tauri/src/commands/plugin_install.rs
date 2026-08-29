// 《铃·记忆体》AI-5 插件命令：安装插件（本地路径 / Git URL）
use crate::error::AppError;
use crate::types::{InstallPluginRequest, Plugin};

/// 安装插件：source 为本地目录路径或 Git 仓库 URL
/// 新安装插件默认禁用，需用户在插件管理中查看权限并启用
#[tauri::command]
pub async fn install_plugin(req: InstallPluginRequest) -> Result<Plugin, AppError> {
    let source = req.source.trim().to_string();
    if source.is_empty() {
        return Err(AppError::PluginInstallError("安装源不能为空".into()));
    }

    let is_git = source.starts_with("http://")
        || source.starts_with("https://")
        || source.ends_with(".git")
        || source.contains("github.com")
        || source.contains("git@");

    let plugin = if is_git {
        crate::plugin::with_manager(|m| m.install_from_git(&source))
    } else {
        crate::plugin::with_manager(|m| m.install_from_path(&source))
    }?;

    log::info!("install_plugin：{} 安装成功", plugin.name);
    Ok(plugin)
}
