// 《铃·记忆体》清空日志（AI-7 任务 10）
use crate::error::AppError;
use crate::logs;

/// 清空主日志文件内容
#[tauri::command]
pub fn clear_logs() -> Result<(), AppError> {
    logs::writer::clear()
        .map_err(|e| AppError::LogWriteError(format!("清空日志失败：{e}")))?;
    Ok(())
}
