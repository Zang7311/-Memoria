// 《铃·记忆体》全局快捷键（AI-6 任务 8）
//  - 使用官方 tauri-plugin-global-shortcut（底层 RegisterHotKey）
//  - 默认 Ctrl+Alt+L 呼出/隐藏主窗口，可在设置页自定义
//  - 注册冲突（如被其他程序占用）返回友好错误
use crate::error::AppError;
use crate::types::RegisterHotkeyResponse;
use tauri::{AppHandle, Manager};

/// 默认快捷键
pub const DEFAULT_ACCELERATOR: &str = "Ctrl+Alt+L";

/// 注册全局快捷键（先注销旧的，再注册新的）
/// accelerator 示例："Ctrl+Alt+L"、"CommandOrControl+Shift+M"
pub fn register(app: &AppHandle, accelerator: &str) -> Result<RegisterHotkeyResponse, AppError> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let shortcuts = app.global_shortcut();

    // 注销旧的，避免重复注册
    let _ = shortcuts.unregister_all();

    let acc: tauri_plugin_global_shortcut::Shortcut = accelerator
        .parse()
        .map_err(|e| AppError::HotkeyError(format!("快捷键格式错误：{e}")))?;

    // 回调：显示/隐藏主窗口
    let acc_clone = acc;
    let result = shortcuts.on_shortcut(acc_clone, move |app, _shortcut, _event| {
        if let Some(win) = app.get_webview_window("main") {
            if win.is_visible().unwrap_or(false) {
                let _ = win.hide();
            } else {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }
    });

    match result {
        Ok(()) => {
            log::info!("[hotkey] 全局快捷键已注册：{accelerator}");
            Ok(RegisterHotkeyResponse {
                registered: true,
                accelerator: accelerator.to_string(),
            })
        }
        Err(e) => {
            log::error!("[hotkey] 快捷键注册失败：{e}");
            Err(AppError::HotkeyError(format!(
                "快捷键注册失败（可能被其他程序占用）：{e}"
            )))
        }
    }
}

/// 注销全部全局快捷键
pub fn unregister_all(app: &AppHandle) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let _ = app.global_shortcut().unregister_all();
    log::info!("[hotkey] 已注销全部全局快捷键");
}
