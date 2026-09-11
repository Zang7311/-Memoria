// 《铃·记忆体》应用生命周期命令
use tauri::AppHandle;
use tauri::Manager;

/// 彻底退出应用：关闭全部窗口并终止进程（解决悬浮球「退出」仅关主窗口残留进程的问题）
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    // 关闭所有已创建的 WebviewWindow
    for (_, win) in app.webview_windows() {
        let _ = win.close();
    }
    app.exit(0);
}
