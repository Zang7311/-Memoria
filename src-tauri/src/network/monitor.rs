// 《铃·记忆体》网络连通性监测（AI-8 任务 8）
//
// 每 30 秒检测网络连通性（ping 8.8.8.8 / reqwest HEAD 百度）。
// 状态变化时 emit "network-status-changed" 事件通知前端；
// 断网时自动通知对话引擎切换至脚本模式，联网后恢复用户设定模式。
use crate::config::store as config_store;
use crate::types::NetworkStatus;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

/// 监测间隔（秒，任务书 30 秒）
pub const MONITOR_INTERVAL_SECS: u64 = 30;
/// 单次探测超时（秒）
const PROBE_TIMEOUT_SECS: u64 = 5;

/// 当前网络状态（进程内缓存）
static STATUS: Mutex<NetworkStatus> = Mutex::new(NetworkStatus::Unknown);
/// 断网前的用户模式（恢复用）
static PRE_OFFLINE_MODE: Mutex<Option<String>> = Mutex::new(None);
/// 监测任务是否已启动（幂等）
static MONITOR_STARTED: AtomicBool = AtomicBool::new(false);

/// 读取当前网络状态
pub fn get_network_status() -> NetworkStatus {
    *STATUS.lock().unwrap()
}

/// 启动网络监测后台任务（lib.rs setup 调用，幂等）
pub fn spawn_monitor(app: AppHandle) {
    if MONITOR_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        log::info!("[network] 网络监测已启动（每 {MONITOR_INTERVAL_SECS} 秒）");
        loop {
            let new_status = probe().await;
            let old = *STATUS.lock().unwrap();
            if new_status != old {
                log::info!("[network] 网络状态变化：{old:?} → {new_status:?}");
                *STATUS.lock().unwrap() = new_status;
                let _ = app.emit("network-status-changed", serde_json::json!({
                    "status": match new_status {
                        NetworkStatus::Online => "online",
                        NetworkStatus::Offline => "offline",
                        NetworkStatus::Unknown => "unknown",
                    }
                }));
                // 断网 → 切换脚本模式；联网 → 恢复
                apply_mode_switch(new_status);
            }
            tokio::time::sleep(std::time::Duration::from_secs(MONITOR_INTERVAL_SECS)).await;
        }
    });
}

/// 探测连通性：优先 reqwest HEAD 百度（国内可用），失败则 ping 8.8.8.8
async fn probe() -> NetworkStatus {
    // 方式 2：HEAD 请求（国内环境用百度）
    if reqwest_ok("https://www.baidu.com").await {
        return NetworkStatus::Online;
    }
    // 方式 1：ping 8.8.8.8（Windows）
    if ping_ok("8.8.8.8").await {
        return NetworkStatus::Online;
    }
    NetworkStatus::Offline
}

/// reqwest HEAD 探测（3 秒超时）
async fn reqwest_ok(url: &str) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(PROBE_TIMEOUT_SECS))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    match client.head(url).send().await {
        Ok(r) => r.status().is_success() || r.status().is_redirection(),
        Err(_) => false,
    }
}

/// ping 探测（Windows：ping -n 1 -w 3000）
async fn ping_ok(host: &str) -> bool {
    #[cfg(windows)]
    {
        let output = tokio::process::Command::new("ping")
            .args(["-n", "1", "-w", "3000", host])
            .output()
            .await;
        match output {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = host;
        false
    }
}

/// 断网时把云端模式切到离线可用模式 / 联网后恢复用户模式
///
/// v1.0 变更：内置 0.5B/1.5B 与 script 本身就完全离线，断网无需动它们；
/// 只有 api 模式需要兜底，且兜底目标改为内置 0.5B（真对话），而不是旧的模板回复。
fn apply_mode_switch(status: NetworkStatus) {
    match status {
        NetworkStatus::Offline => {
            let cfg = config_store::get_config();
            let mode = crate::types::ModelMode::parse(&cfg.model_mode);
            // 已是离线可用模式（local_0b / local_1b / script）→ 无需切换
            if !mode.is_offline() {
                *PRE_OFFLINE_MODE.lock().unwrap() = Some(cfg.model_mode.clone());
                let fallback = crate::types::ModelMode::Local0b.as_str();
                let mut updates = std::collections::HashMap::new();
                updates.insert(
                    "model_mode".to_string(),
                    serde_json::Value::String(fallback.into()),
                );
                if let Err(e) = config_store::update(&updates) {
                    log::warn!("[network] 切换离线模式失败：{e}");
                } else {
                    log::info!("[network] 断网，已自动切换至内置 0.5B（{fallback}）");
                }
            }
        }
        NetworkStatus::Online => {
            let mut lock = PRE_OFFLINE_MODE.lock().unwrap();
            if let Some(mode) = lock.take() {
                let mut updates = std::collections::HashMap::new();
                updates.insert(
                    "model_mode".to_string(),
                    serde_json::Value::String(mode.clone()),
                );
                if let Err(e) = config_store::update(&updates) {
                    log::warn!("[network] 恢复模式 {mode} 失败：{e}");
                } else {
                    log::info!("[network] 联网恢复，已切回 {mode} 模式");
                }
            }
        }
        NetworkStatus::Unknown => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_starts_unknown() {
        assert_eq!(*STATUS.lock().unwrap(), NetworkStatus::Unknown);
    }
}
