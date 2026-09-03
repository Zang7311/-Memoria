// 《铃·记忆体》系统托盘（AI-6 任务 1）
//  - 常驻右下角托盘图标（复用 icons/icon.png）
//  - 右键菜单：显示主窗口 / 悬浮球模式 / 暂停监测 / 退出
//  - 左键单击：显示/隐藏主窗口
use crate::desktop::monitor;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

/// 初始化系统托盘（应用 setup 时调用）
pub fn init_tray(app: &AppHandle) {
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)
        .expect("创建托盘菜单失败");
    let ball = MenuItem::with_id(app, "ball", "悬浮球模式", true, None::<&str>)
        .expect("创建托盘菜单失败");
    let pause = MenuItem::with_id(app, "pause", "暂停监测", true, None::<&str>)
        .expect("创建托盘菜单失败");
    let ct_on = MenuItem::with_id(app, "click_through_on", "鼠标穿透：开", true, None::<&str>)
        .expect("创建托盘菜单失败");
    let ct_off = MenuItem::with_id(app, "click_through_off", "鼠标穿透：关", true, None::<&str>)
        .expect("创建托盘菜单失败");
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)
        .expect("创建托盘菜单失败");
    let menu = Menu::with_items(app, &[&show, &ball, &pause, &ct_on, &ct_off, &quit])
        .expect("创建托盘菜单失败");

    // 托盘图标：打包自带的 icon.png（RGBA，Tauri 2 直接支持）
    let icon = tauri::image::Image::from_bytes(include_bytes!("../../icons/icon.png"))
        .expect("加载托盘图标失败");

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("铃·记忆体")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "ball" => toggle_floating_ball(app),
            "pause" => toggle_monitoring(app),
            "click_through_on" => {
                let _ = crate::commands::ball::set_click_through(app, true);
            }
            "click_through_off" => {
                let _ = crate::commands::ball::set_click_through(app, false);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // 左键单击 → 显示/隐藏主窗口
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("main") {
                    if win.is_visible().unwrap_or(false) {
                        let _ = win.hide();
                    } else {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }
            }
        })
        .build(app)
        .expect("创建系统托盘失败");
}

/// 显示主窗口并聚焦
fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// 切换悬浮球显示/隐藏（悬浮球窗口由 setup 创建，label = "floating-ball"）
fn toggle_floating_ball(app: &AppHandle) {
    if let Some(ball) = app.get_webview_window("floating-ball") {
        if ball.is_visible().unwrap_or(false) {
            let _ = ball.hide();
        } else {
            // 显示前强制置顶（防主窗口激活后悬浮球置顶位丢失被盖）
            let _ = ball.set_always_on_top(true);
            let _ = ball.show();
            let _ = ball.set_focus();
        }
        let _ = app.emit("floating-ball-toggled", ());
    }
}

/// 切换屏幕监测开关（并广播状态变化）
fn toggle_monitoring(app: &AppHandle) {
    let next = !monitor::is_monitoring();
    let ok = monitor::set_monitoring(next, None);
    let _ = app.emit("monitoring-changed", next && ok);
}
