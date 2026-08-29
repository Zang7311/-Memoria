// 《铃·记忆体》打开外部链接命令（收尾工程师新增）
// 供前端跳转外部链接（GitHub 主页 / DeepSeek 官网 / Hermes 技能目录等），
// 复用 tauri-plugin-opener（已在 lib.rs 注册），用系统默认浏览器打开。
use crate::error::AppError;
use tauri_plugin_opener::OpenerExt;

/// 用系统默认浏览器打开指定 URL
#[tauri::command]
pub fn open_url(url: String, app: tauri::AppHandle) -> Result<(), AppError> {
    log::info!("[open_url] 打开链接：{url}");
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| AppError::ConfigError(format!("打开链接失败：{e}")))?;
    Ok(())
}
