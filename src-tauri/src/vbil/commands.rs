// 《铃·记忆体》VBIL 模块 —— Tauri 命令（前端设置页 invoke 调用）

use crate::vbil::{config, scanner, server};
use serde_json::Value;

/// 扫描疑似虚拟形象窗口
#[tauri::command]
pub fn scan_windows() -> Vec<scanner::SuspectedAvatar> {
    scanner::scan_windows()
}

/// 获取 VBIL 完整状态（enabled / mode / port / whitelist）
#[tauri::command]
pub fn get_vbil_status() -> Value {
    let cfg = config::read_config();
    serde_json::json!({
        "enabled": cfg.enabled,
        "mode": cfg.mode,
        "port": server::get_port(),
        "whitelist": cfg.whitelist,
    })
}

/// 设置总开关
#[tauri::command]
pub fn set_vbil_enabled(enabled: bool) {
    config::set_vbil_enabled(enabled);
}

/// 设置响应模式（off / rule_only / ai）
#[tauri::command]
pub fn set_vbil_mode(mode: String) {
    config::set_mode(&mode);
}

/// 设置白名单（覆盖式）
#[tauri::command]
pub fn set_whitelist(ids: Vec<String>) {
    let mut cfg = config::read_config();
    cfg.whitelist = ids;
    config::save_config(&cfg);
    log::info!("[vbil] 白名单已更新：{} 项", cfg.whitelist.len());
}

/// 获取在线客户端列表
#[tauri::command]
pub async fn get_online_clients() -> Vec<server::OnlineClient> {
    server::list_online_clients().await
}
