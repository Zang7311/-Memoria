// 《铃·记忆体》桌面交互与系统集成模块（AI-6）
// 包含：系统托盘、悬浮球、屏幕监测、工具箱、全局快捷键、开机自启动
pub mod autostart;
pub mod hotkey;
pub mod monitor;
pub mod toolbox;
pub mod tray;

use std::path::PathBuf;

/// 用户数据目录（%APPDATA%/ling-memoria-v10，与 AI-5 插件注册表目录保持一致）
/// v1.0：目录名统一由 config::app_dir() 决定，与主线数据隔离。
pub fn data_dir() -> PathBuf {
    let dir = crate::config::app_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// 屏幕监测规则文件路径
pub fn monitor_rules_path() -> PathBuf {
    data_dir().join("monitor_rules.json")
}

/// 用户自定义工具箱条目文件路径
pub fn toolbox_items_path() -> PathBuf {
    data_dir().join("toolbox_items.json")
}
