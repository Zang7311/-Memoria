// 《铃·记忆体》系统信息（AI-7 任务 9/10）
use crate::diagnostic::collect_system_info;
use crate::error::AppError;
use crate::types::SystemInfoResponse;

/// 返回系统信息（CPU/内存/磁盘/OS/应用版本），诊断面板展示
#[tauri::command]
pub fn get_system_info() -> Result<SystemInfoResponse, AppError> {
    let info = collect_system_info()?;
    Ok(SystemInfoResponse { info })
}
