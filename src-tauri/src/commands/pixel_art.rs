// 《铃·记忆体》像素画板保存命令（批次3）
// 前端 canvas.toDataURL 生成 PNG data URL → 保存为桌面 PNG
use crate::error::AppError;
use base64::Engine;

/// 保存像素画（data URL）为 PNG 到桌面，返回保存路径
#[tauri::command]
pub fn save_pixel_art(data_url: String) -> Result<String, AppError> {
    let b64 = data_url
        .split(',')
        .nth(1)
        .ok_or_else(|| AppError::InternalError("无效的图片数据".into()))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| AppError::InternalError(format!("图片解码失败：{e}")))?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = std::env::var("USERPROFILE")
        .map(|p| format!("{}\\Desktop\\pixel_art_{}.png", p, ts))
        .unwrap_or_else(|_| format!("pixel_art_{}.png", ts));
    std::fs::write(&path, &bytes)
        .map_err(|e| AppError::InternalError(format!("保存失败：{e}")))?;
    Ok(path)
}
