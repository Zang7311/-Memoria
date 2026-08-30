// 《铃·记忆体》设备发现（AI-8 任务 1/2）
//
// 1. UDP 广播发现：向 255.255.255.255:54545 广播 { device_id, device_name, version, timestamp }
//    监听回复，维护设备列表；10 秒无心跳移除离线设备。
// 2. 手动连接备选：跳过 UDP，直接 TCP 探测目标 IP:port，成功即加入列表（source=manual）。
//
// 设备列表持久化：~/.铃记忆体/sync_devices.json
use crate::error::AppError;
use crate::sync::conflict;
use crate::types::{DiscoverDevicesResponse, SyncDevice};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;

/// 广播地址（任务书：255.255.255.255:54545，端口可配置）
pub const BROADCAST_ADDR: &str = "255.255.255.255:54545";
/// 设备离线超时（10 秒）
pub const DEVICE_TIMEOUT_SECS: i64 = 10;

/// 发现报文（广播内容）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryBeacon {
    pub device_id: String,
    pub device_name: String,
    pub version: String,
    pub timestamp: String,
}

/// 本机设备列表（内存 + 落盘）
static DEVICES: LazyLock<Mutex<HashMap<String, SyncDevice>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 设备列表文件路径
fn devices_path() -> std::path::PathBuf {
    crate::config::data_dir().join("sync_devices.json")
}

/// 加载已保存的设备列表（启动时）
pub fn load_devices() {
    let loaded: Vec<SyncDevice> = std::fs::read_to_string(devices_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let mut map = DEVICES.lock().unwrap();
    for d in loaded {
        map.insert(d.device_id.clone(), d);
    }
}

/// 持久化设备列表
fn persist_devices() {
    let map = DEVICES.lock().unwrap();
    let list: Vec<SyncDevice> = map.values().cloned().collect();
    drop(map);
    if let Ok(s) = serde_json::to_string_pretty(&list) {
        if let Ok(dir) = crate::config::ensure_data_dir() {
            let _ = std::fs::write(dir.join("sync_devices.json"), s);
        }
    }
}

/// UDP 广播发现（阻塞调用方至 timeout_secs 秒，收集回复）
/// 先发广播，再持续监听回复；同时把仍在线的历史设备保留。
pub async fn discover_devices(timeout_secs: u64) -> Result<DiscoverDevicesResponse, AppError> {
    conflict::set_status("discovering", 0.0, Some("正在扫描局域网设备…".into()));

    let socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| AppError::DiscoveryError(format!("UDP 绑定失败：{e}")))?;
    // 允许广播
    let _ = socket.set_broadcast(true);

    let cfg = conflict::get_config();
    let beacon = DiscoveryBeacon {
        device_id: cfg.device_id.clone(),
        device_name: cfg.device_name.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    let payload = serde_json::to_string(&beacon)?;

    // 发送 3 次广播（间隔 500ms），提高命中率
    for _ in 0..3 {
        let _ = socket.send_to(payload.as_bytes(), BROADCAST_ADDR).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // 持续接收回复直到超时
    let mut buf = [0u8; 1024];
    let deadline = Duration::from_secs(timeout_secs);
    let mut received: Vec<SyncDevice> = Vec::new();
    let result = timeout(deadline, async {
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((n, addr)) => {
                    if let Ok(text) = std::str::from_utf8(&buf[..n]) {
                        if let Ok(b) = serde_json::from_str::<DiscoveryBeacon>(text) {
                            // 忽略自己
                            if b.device_id != cfg.device_id {
                                received.push(SyncDevice {
                                    device_id: b.device_id,
                                    device_name: b.device_name,
                                    ip: addr.ip().to_string(),
                                    port: crate::sync::DATA_TRANSFER_PORT,
                                    last_seen: b.timestamp,
                                    source: "udp".into(),
                                });
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
    })
    .await;

    // 无论是否超时，把收到的设备合并进列表
    let mut map = DEVICES.lock().unwrap();
    for d in received {
        map.insert(d.device_id.clone(), d);
    }
    // 剔除超时离线设备（last_seen 超过 10 秒）
    let now = chrono::Utc::now().timestamp();
    map.retain(|_, d| {
        chrono::DateTime::parse_from_rfc3339(&d.last_seen)
            .map(|t| now - t.timestamp() <= DEVICE_TIMEOUT_SECS)
            .unwrap_or(true)
    });
    let list: Vec<SyncDevice> = map.values().cloned().collect();
    drop(map);
    persist_devices();

    conflict::set_status("idle", 0.0, None);
    let _ = result; // 超时也正常返回
    Ok(DiscoverDevicesResponse { devices: list })
}

/// 手动添加设备：TCP 探测目标 IP:port（54546），成功则加入列表（source=manual）
pub async fn add_manual_device(ip: &str, port: u16) -> Result<SyncDevice, AppError> {
    let addr = format!("{ip}:{port}");
    // 直接 TCP 连接探测（tokio TcpStream）
    let connect = tokio::net::TcpStream::connect(&addr);
    match timeout(Duration::from_secs(5), connect).await {
        Ok(Ok(_stream)) => {
            let device = SyncDevice {
                device_id: format!("manual-{ip}-{port}"),
                device_name: format!("手动设备 {ip}:{port}"),
                ip: ip.to_string(),
                port,
                last_seen: chrono::Utc::now().to_rfc3339(),
                source: "manual".into(),
            };
            let mut map = DEVICES.lock().unwrap();
            map.insert(device.device_id.clone(), device.clone());
            drop(map);
            persist_devices();
            log::info!("[sync] 手动添加设备成功：{addr}");
            Ok(device)
        }
        Ok(Err(e)) => Err(AppError::SyncError(format!(
            "无法连接到 {addr}：{e}（UDP 广播可能被防火墙阻断，请检查端口）"
        ))),
        Err(_) => Err(AppError::SyncError(format!("连接 {addr} 超时"))),
    }
}

/// 按 device_id 查设备（不存在返回错误）
pub fn find_device(device_id: &str) -> Result<SyncDevice, AppError> {
    DEVICES
        .lock()
        .unwrap()
        .get(device_id)
        .cloned()
        .ok_or_else(|| AppError::SyncError(format!("未找到设备 {device_id}，请先扫描或手动添加")))
}

/// 当前设备列表（直接读取）
pub fn list_devices() -> Vec<SyncDevice> {
    DEVICES.lock().unwrap().values().cloned().collect()
}

/// 从 SocketAddr 更新设备最后发现时间（心跳刷新）
pub fn touch_device(device_id: &str, addr: &SocketAddr) {
    let mut map = DEVICES.lock().unwrap();
    if let Some(d) = map.get_mut(device_id) {
        d.last_seen = chrono::Utc::now().to_rfc3339();
        if !d.ip.is_empty() {
            d.ip = addr.ip().to_string();
        }
    }
}

/// —— 常驻 UDP 广播响应器（修复：设备发现缺"响应方"）——
/// 之前 discover_devices 只发广播 + 收回复，但没有任何进程监听 54545 应答，
/// 导致两台机器互相搜不到。此函数在 54545 常驻监听，收到其他设备的发现广播
/// 后，立即回包自己的设备信息（device_id/name/version/timestamp），
/// 使对方 discover_devices 的 recv_from 能收到应答。
static RESPONDING: AtomicBool = AtomicBool::new(false);

/// 启动 UDP 广播响应器（lib.rs setup 调用，幂等）
pub fn spawn_responder() {
    if RESPONDING.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let socket = match UdpSocket::bind("0.0.0.0:54545").await {
            Ok(s) => s,
            Err(e) => {
                log::error!("[sync] UDP 响应器绑定 0.0.0.0:54545 失败：{e}");
                RESPONDING.store(false, Ordering::SeqCst);
                return;
            }
        };
        let _ = socket.set_broadcast(true);
        log::info!("[sync] UDP 广播响应器已监听 0.0.0.0:54545");
        let mut buf = [0u8; 1024];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((n, addr)) => {
                    if let Ok(text) = std::str::from_utf8(&buf[..n]) {
                        if let Ok(b) = serde_json::from_str::<DiscoveryBeacon>(text) {
                            // 忽略自己发出的广播，避免自应答
                            let cfg = conflict::get_config();
                            if b.device_id == cfg.device_id {
                                continue;
                            }
                            // 回包自己的设备信息
                            let reply = DiscoveryBeacon {
                                device_id: cfg.device_id.clone(),
                                device_name: cfg.device_name.clone(),
                                version: env!("CARGO_PKG_VERSION").to_string(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                            };
                            if let Ok(payload) = serde_json::to_string(&reply) {
                                let _ = socket.send_to(payload.as_bytes(), addr).await;
                            }
                        }
                    }
                }
                Err(e) => log::debug!("[sync] UDP 响应器 recv 失败：{e}"),
            }
        }
    });
}
