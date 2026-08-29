// 《铃·记忆体》IPC：悬浮球可见性控制（AI-6 任务 10 / 任务 2）
// 悬浮球窗口由 setup 创建（label = "floating-ball"），此命令控制显示/隐藏
use crate::error::AppError;
use crate::types::SetFloatingBallVisibilityRequest;
use tauri::{AppHandle, Manager};

/// 显示/隐藏悬浮球
#[tauri::command]
pub fn set_floating_ball_visibility(
    app: AppHandle,
    request: SetFloatingBallVisibilityRequest,
) -> Result<(), AppError> {
    if let Some(ball) = app.get_webview_window("floating-ball") {
        if request.visible {
            let _ = ball.show();
            log::info!("[floating-ball] 悬浮球已显示");
        } else {
            let _ = ball.hide();
            log::info!("[floating-ball] 悬浮球已隐藏");
        }
    }
    Ok(())
}
