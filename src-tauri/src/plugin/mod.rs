// 《铃·记忆体》AI-5 插件系统模块
// 独立于对话引擎与记忆系统运行，通过 IPC 命令与主应用交互。
pub mod hermes_compat;
pub mod loader;
pub mod manager;
pub mod permissions;
pub mod runner;
pub mod sandbox;

use std::sync::Mutex;

use tauri::AppHandle;

use manager::PluginManager;

/// 全局插件管理器（应用启动时初始化一次）
pub static PLUGIN_MANAGER: Mutex<Option<PluginManager>> = Mutex::new(None);

/// 初始化插件管理器（在 tauri setup 中调用）
pub fn init(app: &AppHandle) {
    let mgr = PluginManager::init(app);
    *PLUGIN_MANAGER.lock().unwrap() = Some(mgr);
    log::info!("插件管理器初始化完成");
}

/// 获取插件管理器并执行操作（持锁期间禁止 await，注意锁粒度）
pub fn with_manager<T>(f: impl FnOnce(&mut PluginManager) -> T) -> T {
    let mut guard = PLUGIN_MANAGER.lock().unwrap();
    if guard.is_none() {
        // 兜底：未初始化时用空目录构造（正常流程不会走到）
        *guard = Some(PluginManager::new_with_dirs(
            manager::app_data_dir().join("plugins"),
            std::path::PathBuf::from("src-tauri/plugins"),
            std::path::PathBuf::from("src-tauri/hermes_plugins"),
        ));
    }
    f(guard.as_mut().unwrap())
}
