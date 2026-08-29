// 《铃·记忆体》IPC：全局快捷键（AI-6 任务 10）
use crate::desktop::hotkey;
use crate::error::AppError;
use crate::types::RegisterHotkeyRequest;
use tauri::AppHandle;

/// 注册全局快捷键（默认 Ctrl+Alt+L 呼出主窗口；可自定义）
#[tauri::command]
pub fn register_hotkey(
    app: AppHandle,
    request: RegisterHotkeyRequest,
) -> Result<crate::types::RegisterHotkeyResponse, AppError> {
    hotkey::register(&app, &request.accelerator)
}

/// 注销全部全局快捷键
#[tauri::command]
pub fn unregister_hotkey(app: AppHandle) {
    hotkey::unregister_all(&app);
}
