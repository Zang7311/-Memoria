// 《铃·记忆体》IPC：获取当前前台窗口信息（AI-6 任务 10）
use crate::desktop::monitor;
use crate::error::AppError;
use crate::types::GetWindowInfoResponse;

/// 获取当前前台窗口信息（应用名 + 标题 + 全屏状态）
#[tauri::command]
pub fn get_window_info() -> Result<GetWindowInfoResponse, AppError> {
    let info = monitor::detect_window()?;
    Ok(GetWindowInfoResponse { info })
}
