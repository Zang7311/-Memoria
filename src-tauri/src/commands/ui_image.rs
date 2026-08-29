// 《铃·记忆体》UI 图片保存命令（背景图/头像选图用）
// 前端用 HTML file input + FileReader 读成 data URL → 本命令保存到应用数据目录 ui_assets/
// webview 用 convertFileSrc(路径) 经 asset 协议加载，解决本地路径无法直接显示的问题
use crate::error::AppError;
use base64::Engine;

/// 保存 UI 图片（背景图/头像），返回保存的绝对路径
/// prefix: 文件名前缀（如 bg / avatar）
#[tauri::command]
pub fn save_ui_image(data_url: String, prefix: String) -> Result<String, AppError> {
    let b64 = data_url.split(',').nth(1).unwrap_or(&data_url);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| AppError::InternalError(format!("图片解码失败：{e}")))?;
    let base = crate::config::store::get_config().data_path.clone();
    let dir = format!("{}\\ui_assets", base);
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::InternalError(format!("创建图片目录失败：{e}")))?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = format!("{}\\ui_assets\\{}_{}.png", base, prefix, ts);
    std::fs::write(&path, &bytes)
        .map_err(|e| AppError::InternalError(format!("保存图片失败：{e}")))?;
    Ok(path)
}
