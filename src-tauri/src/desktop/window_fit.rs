// 主窗口尺寸自适应：按当前显示器的逻辑尺寸收一点，避免窗口比屏幕还高
// （高缩放笔记本上 1200×720 会顶到屏幕边缘，底部图标被任务栏挡住、点不到）
use tauri::{AppHandle, Manager};

/// 目标宽高上限：与 tauri.conf.json 里的默认窗口一致
const MAX_W: f64 = 1200.0;
const MAX_H: f64 = 720.0;
/// 安全下限：再小就不像话了
const MIN_W: f64 = 960.0;
const MIN_H: f64 = 560.0;

/// 给主窗口套一个「不超出屏幕」的尺寸，并居中。
/// 显示器足够大时保持默认 1200×720 不动。
pub fn fit_main_window(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    // 取窗口所在显示器（多屏时是它自己那块）
    let Ok(Some(monitor)) = win.current_monitor() else {
        return;
    };
    let scale = monitor.scale_factor();
    if scale <= 0.0 {
        return;
    }
    // Monitor::size() 是物理像素，换算成逻辑像素才能和窗口尺寸对齐
    let screen_w = monitor.size().width as f64 / scale;
    let screen_h = monitor.size().height as f64 / scale;

    // 宽度最多占 94%，高度最多占 88%（给任务栏和标题栏留位置）
    let target_w = (screen_w * 0.94).min(MAX_W).max(MIN_W);
    let target_h = (screen_h * 0.88).min(MAX_H).max(MIN_H);

    // 屏幕上放不下时，宁可再小一点也要保证完整可见
    let target_w = target_w.min(screen_w - 16.0);
    let target_h = target_h.min(screen_h - 56.0);

    if let Err(e) = win.set_size(tauri::LogicalSize::new(target_w, target_h)) {
        log::warn!("[window-fit] 调整主窗口尺寸失败：{e}");
        return;
    }
    let _ = win.center();
    log::info!(
        "[window-fit] 屏幕 {screen_w:.0}×{screen_h:.0}（缩放 {scale}）→ 主窗口 {target_w:.0}×{target_h:.0}"
    );
}
