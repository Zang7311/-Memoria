// 《铃·记忆体》全局快捷键（AI-6 任务 8 + v0.6 悬浮球扩展）
//  - 使用官方 tauri-plugin-global-shortcut（底层 RegisterHotKey）
//  - 主热键（默认 Ctrl+Alt+L）呼出/隐藏主窗口，可在设置页自定义
//  - 悬浮球固定热键（不可自定义，设置页更换主热键时保留）：
//      Ctrl+Alt+B  显示/隐藏悬浮球
//      Ctrl+Alt+Q  唤起悬浮球并打开「快速提问」
//  - 注册冲突（如被其他程序占用）返回友好错误
use crate::error::AppError;
use crate::types::RegisterHotkeyResponse;
use tauri::{AppHandle, Emitter, Manager};

/// 默认主热键（呼出/隐藏主窗口，可在设置页自定义）
pub const DEFAULT_ACCELERATOR: &str = "Ctrl+Alt+L";
/// 悬浮球显隐热键（固定）
pub const BALL_TOGGLE_ACCELERATOR: &str = "Ctrl+Alt+B";
/// 悬浮球快速提问热键（固定）
pub const BALL_ASK_ACCELERATOR: &str = "Ctrl+Alt+Q";

/// 注册全局快捷键（先注销旧的，再注册主热键 + 悬浮球固定热键）
/// accelerator 示例："Ctrl+Alt+L"、"CommandOrControl+Shift+M"
pub fn register(app: &AppHandle, accelerator: &str) -> Result<RegisterHotkeyResponse, AppError> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let shortcuts = app.global_shortcut();

    // 注销旧的，避免重复注册
    let _ = shortcuts.unregister_all();

    let mut last_err: Option<String> = None;

    // ① 主窗口热键（可自定义）：呼出/隐藏主窗口
    if let Ok(acc) = accelerator.parse::<tauri_plugin_global_shortcut::Shortcut>() {
        let result = shortcuts.on_shortcut(acc, move |app, _shortcut, _event| {
            if let Some(win) = app.get_webview_window("main") {
                if win.is_visible().unwrap_or(false) {
                    let _ = win.hide();
                } else {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        });
        if let Err(e) = result {
            last_err = Some(e.to_string());
        }
    } else {
        last_err = Some(format!("快捷键格式错误：{accelerator}"));
    }

    // ② 悬浮球显隐（固定 Ctrl+Alt+B，不与主热键冲突时注册）
    if accelerator != BALL_TOGGLE_ACCELERATOR {
        if let Ok(acc) = BALL_TOGGLE_ACCELERATOR.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            let result = shortcuts.on_shortcut(acc, move |app, _shortcut, _event| {
                if let Some(ball) = app.get_webview_window("floating-ball") {
                    if ball.is_visible().unwrap_or(false) {
                        let _ = ball.hide();
                    } else {
                        let _ = ball.set_always_on_top(true);
                        let _ = ball.show();
                    }
                    let _ = app.emit("floating-ball-toggled", ());
                }
            });
            if let Err(e) = result {
                last_err = Some(e.to_string());
            }
        }
    }

    // ③ 悬浮球快速提问（固定 Ctrl+Alt+Q）：显示悬浮球并通知其打开提问卡
    if accelerator != BALL_ASK_ACCELERATOR {
        if let Ok(acc) = BALL_ASK_ACCELERATOR.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            let result = shortcuts.on_shortcut(acc, move |app, _shortcut, _event| {
                if let Some(ball) = app.get_webview_window("floating-ball") {
                    let _ = ball.set_always_on_top(true);
                    let _ = ball.show();
                }
                let _ = app.emit("ball-hotkey-ask", ());
            });
            if let Err(e) = result {
                last_err = Some(e.to_string());
            }
        }
    }

    match last_err {
        Some(msg) => {
            log::error!("[hotkey] 部分快捷键注册失败：{msg}");
            Err(AppError::HotkeyError(format!("快捷键注册失败（可能被其他程序占用）：{msg}")))
        }
        None => {
            log::info!(
                "[hotkey] 全局快捷键已注册：主[{accelerator}] 球显隐[{BALL_TOGGLE_ACCELERATOR}] 快速提问[{BALL_ASK_ACCELERATOR}]"
            );
            Ok(RegisterHotkeyResponse {
                registered: true,
                accelerator: accelerator.to_string(),
            })
        }
    }
}

/// 注销全部全局快捷键
pub fn unregister_all(app: &AppHandle) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let _ = app.global_shortcut().unregister_all();
    log::info!("[hotkey] 已注销全部全局快捷键");
}
