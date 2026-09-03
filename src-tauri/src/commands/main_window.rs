// 《铃·记忆体》IPC：确保主窗口存在（v0.6 悬浮球「打开主窗口/快速提问/设置」入口）
// 背景：悬浮球窗口与托盘存在时，用户关闭主窗口并不会退出应用；
//       此后悬浮球若再调 getByLabel('main') 会拿到 null → 按钮全部无反应。
//       此命令在 main 不存在时按初始配置重建。
use crate::error::AppError;
use tauri::{AppHandle, Manager};

/// 确保主窗口存在；若已被关闭则重建（与 tauri.conf.json 的 main 窗口一致）
#[tauri::command]
pub fn ensure_main_window(app: AppHandle) -> Result<(), AppError> {
    if app.get_webview_window("main").is_none() {
        tauri::WebviewWindowBuilder::new(
            &app,
            "main",
            tauri::WebviewUrl::App("index.html".into()),
        )
        .title("铃·记忆体")
        .inner_size(1200.0, 720.0)
        .min_inner_size(1200.0, 720.0)
        .build()?;
        log::info!("[main-window] 主窗口已重建");
    }
    Ok(())
}
