// 《铃·记忆体》IPC：悬浮球鼠标穿透控制（v0.6 悬浮球重构）
//  - 穿透状态统一在 Rust 侧维护（static），悬浮球右键菜单 / 托盘菜单走同一入口
//  - 开启后窗口忽略全部鼠标事件（看视频/全屏时不打扰），托盘菜单可一键恢复
//  - 状态变化通过事件 ball-click-through-changed 广播给所有窗口（同步 UI 显示）
use crate::error::AppError;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

/// 当前穿透状态（进程内全局，命令与托盘共享）
static CLICK_THROUGH: AtomicBool = AtomicBool::new(false);

/// 核心实现：改窗口行为 + 更新状态 + 广播（托盘菜单与命令统一调用）
pub fn set_click_through(app: &AppHandle, enabled: bool) -> Result<(), AppError> {
    if let Some(ball) = app.get_webview_window("floating-ball") {
        ball.set_ignore_cursor_events(enabled)?;
    }
    CLICK_THROUGH.store(enabled, Ordering::SeqCst);
    let _ = app.emit("ball-click-through-changed", enabled);
    log::info!(
        "[floating-ball] 鼠标穿透：{}",
        if enabled { "开" } else { "关" }
    );
    Ok(())
}

/// IPC：切换悬浮球鼠标穿透（悬浮球右键菜单 / 后续设置页调用），返回最新状态
#[tauri::command]
pub fn set_floating_ball_click_through(
    app: AppHandle,
    enabled: bool,
) -> Result<bool, AppError> {
    set_click_through(&app, enabled)?;
    Ok(CLICK_THROUGH.load(Ordering::SeqCst))
}

/// IPC：查询当前穿透状态（主窗口 / 悬浮球初始化 UI 时调用）
#[tauri::command]
pub fn get_floating_ball_click_through() -> bool {
    CLICK_THROUGH.load(Ordering::SeqCst)
}
