// 《铃·记忆体》应用生命周期命令
use crate::desktop::native_pet;
use tauri::AppHandle;
use tauri::Manager;

/// 彻底退出应用：关闭全部窗口并终止进程（解决悬浮球「退出」仅关主窗口残留进程的问题）
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    // 先把托管的桌宠收掉，免得应用没了桌宠还在桌面上
    native_pet::shutdown();
    // 关闭所有已创建的 WebviewWindow
    for (_, win) in app.webview_windows() {
        let _ = win.close();
    }
    app.exit(0);
}
