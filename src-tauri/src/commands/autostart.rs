// 《铃·记忆体》IPC：开机自启动（AI-6 任务 9 / 设置页开关）
use crate::desktop::autostart;
use crate::error::AppError;
use crate::types::{GetAutostartResponse, SetAutostartRequest};

/// 设置开机自启动（写入/删除注册表 Run 键）
#[tauri::command]
pub fn set_autostart(request: SetAutostartRequest) -> Result<(), AppError> {
    autostart::set_autostart(request.enabled)
}

/// 查询开机自启动状态
#[tauri::command]
pub fn get_autostart() -> GetAutostartResponse {
    GetAutostartResponse {
        enabled: autostart::is_autostart_enabled(),
    }
}
