// 《铃·记忆体》数据传输（AI-8 任务 3）
//
// TCP 可靠传输，端口 54546。帧协议见 payload.rs：
//   [4-byte 大端长度][JSON SyncEnvelope]
//
// 协议流程（客户端拉取模式，v2 强制挑战-应答认证）：
//   0a. 服务端 → 客户端：Challenge 帧（一次性随机 nonce）
//   0b. 客户端 → 服务端：Auth 帧（HMAC-SHA256(派生密钥, 域前缀||nonce)）
//       服务端校验失败 → 立即断开，不泄露任何记忆数据
//   1. 客户端 → 服务端：Request 帧（device_id / set_name / last_sync_time）
//   2. 服务端 → 客户端：0..N 个 Payload 帧（每批 ≤100 条，加密 + SHA-256 校验）
//   3. 服务端 → 客户端：Response 帧（发送完成汇总，synced_count = 发送总数）
//   4. 客户端 → 服务端：Response 帧（接收回执确认，synced_count = 实际写入数）
//
// 增量同步：客户端携带本机该记忆集最后一条时间戳，服务端只回传更新的记忆。
//
// 安全（修复"局域网任意设备可拉取全部记忆"）：
//   旧版 pairing_code 是「固定串 + 固定密钥」的静态密文，且服务端只判断能否解密、
//   不校验明文内容 —— 局域网抓一次包即可永久重放。现改为每连接一次性 nonce 的
//   挑战-应答，未持有相同主密码派生密钥的设备无法产生有效 MAC，认证在读取
//   任何记忆之前完成，失败即断连。
use crate::context::MEMORY_WRITER_LOCK;
use crate::error::AppError;
use crate::memory::storage;
use crate::sync::conflict::{self, filter_incremental, merge_memories};
use crate::sync::encryption;
use crate::sync::payload::{
    compute_auth_mac, decode_frame, encode_frame, generate_nonce_b64, sha256_hex, verify_auth_mac,
    SyncAuth, SyncEnvelope, MAX_BATCH,
};
use crate::types::{
    Memory, StartSyncRequest, StartSyncResponse, SyncDevice, SyncHistoryEntry, SyncPayload,
    SyncProgressEvent, SyncRequest, SyncResponse,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex as TokioMutex, Semaphore};
use tokio::time::timeout;

/// 数据传输端口（任务书固定 54546）
pub const DATA_TRANSFER_PORT: u16 = 54546;
/// 单帧/单批读超时（秒）
const FRAME_TIMEOUT_SECS: u64 = 60;
/// 连接超时（秒）
const CONNECT_TIMEOUT_SECS: u64 = 5;
/// 最大并发同步连接数
const MAX_CONCURRENT_CONNECTIONS: usize = 4;
/// 每 IP 每分钟最多允许的连接次数
const RATE_LIMIT_PER_MIN: u32 = 10;

/// 监听中标志（防止重复启动）
static LISTENING: AtomicBool = AtomicBool::new(false);

/// 每 IP 速率状态：(本分钟内连接次数, 分钟起始时刻)
type RateMap = Arc<TokioMutex<HashMap<String, (u32, std::time::Instant)>>>;

fn make_rate_map() -> RateMap {
    Arc::new(TokioMutex::new(HashMap::new()))
}

/// 检查并更新速率限制；返回 true 表示允许，false 表示拒绝
async fn rate_check(map: &RateMap, ip: &str) -> bool {
    let mut guard = map.lock().await;
    let now = std::time::Instant::now();
    let entry = guard.entry(ip.to_string()).or_insert((0, now));
    // 新的一分钟窗口则重置计数
    if entry.1.elapsed() >= Duration::from_secs(60) {
        *entry = (0, now);
    }
    if entry.0 >= RATE_LIMIT_PER_MIN {
        return false;
    }
    entry.0 += 1;
    true
}

// ==================== 服务端：常驻监听 ====================

/// 启动 TCP 监听（lib.rs setup 调用，幂等）
pub fn spawn_listener(app: AppHandle) {
    if LISTENING.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let listener = match TcpListener::bind(("0.0.0.0", DATA_TRANSFER_PORT)).await {
            Ok(l) => l,
            Err(e) => {
                log::error!("[sync] TCP 监听 {DATA_TRANSFER_PORT} 失败：{e}");
                LISTENING.store(false, Ordering::SeqCst);
                return;
            }
        };
        log::info!("[sync] 同步服务已监听 0.0.0.0:{DATA_TRANSFER_PORT}");

        // 并发连接限制：最多同时处理 MAX_CONCURRENT_CONNECTIONS 个同步请求
        let sem = Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS));
        // 每 IP 速率限制
        let rate_map = make_rate_map();

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let ip = addr.ip().to_string();

                    // 速率检查（每 IP 每分钟最多 RATE_LIMIT_PER_MIN 次）
                    if !rate_check(&rate_map, &ip).await {
                        log::warn!("[sync] 速率限制：拒绝来自 {ip} 的连接");
                        continue;
                    }

                    // 尝试获取并发槽位（非阻塞：满了直接拒绝，不排队）
                    let permit = match Arc::clone(&sem).try_acquire_owned() {
                        Ok(p) => p,
                        Err(_) => {
                            log::warn!("[sync] 并发上限（{MAX_CONCURRENT_CONNECTIONS}），拒绝来自 {ip} 的连接");
                            continue;
                        }
                    };

                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _permit = permit; // 连接处理完后自动释放槽位
                        if let Err(e) = handle_incoming(stream, addr, app).await {
                            log::warn!("[sync] 处理来自 {addr} 的连接失败：{e}");
                        }
                    });
                }
                Err(e) => log::warn!("[sync] accept 失败：{e}"),
            }
        }
    });
}

/// 处理一次入站同步（服务端角色：应答方）
async fn handle_incoming(
    mut stream: TcpStream,
    addr: SocketAddr,
    app: AppHandle,
) -> Result<(), AppError> {
    log::info!("[sync] 收到来自 {addr} 的同步连接");

    // ==================== 强制认证（在任何数据读取之前）====================
    // 顺序很关键：先要求本机已设置主密码并解锁，再挑战-应答，
    // 通过后才允许进入业务流程。任一步失败立即返回 → 连接被 drop 断开。
    let cfg = crate::config::store::get_config();
    if !cfg.has_master_password {
        return Err(AppError::SyncError(
            "同步要求先设置主密码，请在设置中设置主密码后再试".into(),
        ));
    }
    let key = crate::config::encryption::get_key()
        .map_err(|_| AppError::SyncError("同步要求先解锁主密码".into()))?;

    // 0a. 下发一次性挑战 nonce
    let nonce = generate_nonce_b64();
    stream
        .write_all(&encode_frame(&SyncEnvelope::Challenge {
            nonce: nonce.clone(),
        })?)
        .await?;

    // 0b. 校验应答 MAC —— 未配对设备在此被拒，拿不到任何记忆
    let auth = match read_frame_timeout(&mut stream).await? {
        SyncEnvelope::Auth { auth } => auth,
        _ => {
            log::warn!("[sync] 拒绝 {addr}：未按协议发送认证应答（可能是旧版客户端或未授权设备）");
            return Err(AppError::SyncError(
                "认证失败：首帧必须是挑战应答（请升级双端铃·记忆体至同一版本）".into(),
            ));
        }
    };
    if !verify_auth_mac(&key, &nonce, &auth.mac) {
        log::warn!(
            "[sync] 拒绝 {addr}：认证失败（设备 {}），双端主密码不一致或为未授权设备",
            auth.device_id
        );
        return Err(AppError::SyncError(
            "认证失败：双端主密码不一致，该设备未配对".into(),
        ));
    }
    log::info!("[sync] {addr}（设备 {}）认证通过", auth.device_id);

    conflict::set_status("syncing", 0.0, Some(format!("正在响应 {addr} 的同步…")));

    // 1. 读 SyncRequest（此时对端身份已确认）
    let request = match read_frame_timeout(&mut stream).await? {
        SyncEnvelope::Request { request } => request,
        _ => return Err(AppError::SyncError("认证后的首个帧必须是 SyncRequest".into())),
    };

    // 记忆集名校验：防止 ../ 穿越读到同步目录之外的索引文件
    let set_name = sanitize_set_name(&request.set_name)?;
    let total_memories = storage::read_all(&storage::set_index_path(Some(&set_name)))?;
    let to_send = filter_incremental(&total_memories, request.last_sync_time.as_deref());
    log::info!(
        "[sync] 应答 {addr} 的记忆集 {set_name}：发送 {} 条（增量={}）",
        to_send.len(),
        request.last_sync_time.is_some()
    );

    // 2. 分批加密发送（每批 MAX_BATCH 条）
    let mut sent = 0usize;
    for chunk in to_send.chunks(MAX_BATCH) {
        let plain = serde_json::to_string(chunk)?;
        let encrypted = encryption::encrypt_memories(&plain)?;
        let payload = SyncPayload {
            device_id: conflict::get_config().device_id.clone(),
            set_name: set_name.clone(),
            encrypted_data: encrypted,
            checksum: sha256_hex(&plain),
            incremental: request.last_sync_time.is_some(),
        };
        stream
            .write_all(&encode_frame(&SyncEnvelope::Payload { payload })?)
            .await?;
        sent += chunk.len();
        conflict::set_status(
            "syncing",
            (sent as f32) / (to_send.len().max(1) as f32),
            Some(format!("正在发送 {sent}/{} 条…", to_send.len())),
        );
        let _ = app.emit(
            "sync-progress",
            SyncProgressEvent {
                current: sent,
                total: to_send.len(),
                phase: "send".into(),
            },
        );
    }

    // 3. 发送完成汇总帧
    let summary = SyncResponse {
        success: true,
        message: format!("已发送 {} 条记忆", sent),
        synced_count: sent,
        conflict_resolved: false,
    };
    stream
        .write_all(&encode_frame(&SyncEnvelope::Response { response: summary })?)
        .await?;

    // 4. 等待客户端回执
    match read_frame_timeout(&mut stream).await? {
        SyncEnvelope::Response { response } => {
            let msg = response.message.clone();
            conflict::record_history(SyncHistoryEntry {
                time: chrono::Utc::now().to_rfc3339(),
                device: addr.ip().to_string(),
                set_name,
                success: response.success,
                message: msg.clone(),
                synced_count: response.synced_count,
            });
            conflict::set_status(
                "done",
                1.0,
                Some(format!("对方已接收 {} 条", response.synced_count)),
            );
            let _ = app.emit(
                "sync-progress",
                SyncProgressEvent {
                    current: response.synced_count,
                    total: response.synced_count,
                    phase: "done".into(),
                },
            );
            log::info!("[sync] 应答 {addr} 完成：{msg}");
        }
        _ => log::warn!("[sync] 未收到有效回执"),
    }
    Ok(())
}

// ==================== 客户端：发起同步 ====================

/// 发起同步（start_sync 命令调用）—— 从目标设备拉取记忆
pub async fn start_sync(
    req: StartSyncRequest,
    app: AppHandle,
) -> Result<StartSyncResponse, AppError> {
    // 目标地址：手动 IP 优先，否则查设备表
    let (target_ip, target_port) = if let Some(ip) = req.manual_ip.as_deref() {
        (ip.to_string(), req.manual_port.unwrap_or(DATA_TRANSFER_PORT))
    } else {
        let dev: SyncDevice = crate::sync::discovery::find_device(&req.target_device)?;
        (dev.ip.clone(), dev.port)
    };
    let addr = format!("{target_ip}:{target_port}");

    conflict::set_status("syncing", 0.0, Some(format!("正在连接 {addr}…")));
    log::info!("[sync] 开始同步：目标 {addr}，记忆集 {}", req.set_name);

    // 连接（超时保护）
    let stream = timeout(
        Duration::from_secs(CONNECT_TIMEOUT_SECS),
        TcpStream::connect(&addr),
    )
    .await
    .map_err(|_| {
        conflict::set_status("error", 0.0, Some("连接超时".into()));
        AppError::SyncError(format!("连接 {addr} 超时"))
    })?
    .map_err(|e| {
        conflict::set_status("error", 0.0, Some(format!("连接失败：{e}")));
        AppError::SyncError(format!("连接 {addr} 失败：{e}"))
    })?;
    let mut stream = stream;

    // 客户端同样要求主密码已设置并解锁（认证凭据来源）
    let sync_key = crate::config::encryption::get_key()
        .map_err(|_| AppError::SyncError("同步要求先解锁主密码".into()))?;
    {
        let cfg = crate::config::store::get_config();
        if !cfg.has_master_password {
            return Err(AppError::SyncError(
                "同步要求先设置主密码，请在设置中设置主密码后再试".into(),
            ));
        }
    }

    // 0. 认证握手：读服务端挑战 → 回 HMAC 应答
    let nonce = match read_frame_timeout(&mut stream).await? {
        SyncEnvelope::Challenge { nonce } => nonce,
        _ => {
            conflict::set_status("error", 0.0, Some("对端未要求认证".into()));
            return Err(AppError::SyncError(
                "对端未发送认证挑战，可能是旧版本，请升级双端铃·记忆体至同一版本".into(),
            ));
        }
    };
    let device_id = conflict::get_config().device_id.clone();
    let auth = SyncAuth {
        device_id: device_id.clone(),
        mac: compute_auth_mac(&sync_key, &nonce),
    };
    stream
        .write_all(&encode_frame(&SyncEnvelope::Auth { auth })?)
        .await?;

    // 1. 发送 SyncRequest（增量基准 = 本机该记忆集最后一条的时间戳）
    let request = SyncRequest {
        device_id,
        set_name: req.set_name.clone(),
        last_sync_time: last_sync_time(&req.set_name),
        pairing_code: None,
    };
    stream
        .write_all(&encode_frame(&SyncEnvelope::Request { request })?)
        .await?;

    // 2. 循环接收 Payload 批 → 解密 → 校验 → 合并写存储
    let policy = conflict::get_config().conflict_policy;
    let mut total_received = 0usize;
    let mut total_written = 0usize;
    let mut conflict_count = 0usize;

    loop {
        let env = read_frame_timeout(&mut stream).await?;
        match env {
            SyncEnvelope::Payload { payload } => {
                // 解密
                let plain = encryption::decrypt_memories(&payload.encrypted_data)?;
                // 校验和验证（SHA-256）
                if sha256_hex(&plain) != payload.checksum {
                    conflict::set_status("error", 0.0, Some("校验和不匹配，数据可能被篡改".into()));
                    return Err(AppError::ChecksumMismatch(format!(
                        "期望 {} 实际 {}",
                        payload.checksum,
                        sha256_hex(&plain)
                    )));
                }
                let remote: Vec<Memory> = serde_json::from_str(&plain)?;
                total_received += remote.len();

                // 合并冲突 + 原子写存储（带全局写锁）
                // 服务端回传的 set_name 也要校验：防恶意对端用 ../ 让本机写到目录外
                let safe_set = sanitize_set_name(&payload.set_name)?;
                let path = storage::set_index_path(Some(&safe_set));
                let local = storage::read_all(&path)?;
                let (merged, conflicts, written) = merge_memories(&local, &remote, policy);
                conflict_count += conflicts;
                if written > 0 {
                    write_merged_atomic(&path, &merged)?;
                }
                total_written += written;

                conflict::set_status(
                    "syncing",
                    (total_written as f32) / (total_received.max(1) as f32),
                    Some(format!("已接收 {total_received} 条…")),
                );
                let _ = app.emit(
                    "sync-progress",
                    SyncProgressEvent {
                        current: total_received,
                        total: 0,
                        phase: "receive".into(),
                    },
                );
            }
            SyncEnvelope::Response { response } => {
                // 服务端发送完成汇总帧
                log::info!("[sync] 服务端汇总：{}", response.message);
                break;
            }
            SyncEnvelope::Request { .. } => {
                return Err(AppError::SyncError("协议错误：服务端不应发送 Request".into()));
            }
            SyncEnvelope::Challenge { .. } | SyncEnvelope::Auth { .. } => {
                return Err(AppError::SyncError(
                    "协议错误：认证已完成，不应再收到握手帧".into(),
                ));
            }
        }
    }

    // 3. 回发接收回执
    let msg = format!(
        "同步完成：接收 {total_received} 条，写入 {total_written} 条，冲突 {conflict_count} 处已按策略解决"
    );
    let receipt = SyncResponse {
        success: true,
        message: msg.clone(),
        synced_count: total_written,
        conflict_resolved: conflict_count > 0,
    };
    let _ = stream
        .write_all(&encode_frame(&SyncEnvelope::Response { response: receipt })?)
        .await;

    conflict::record_history(SyncHistoryEntry {
        time: chrono::Utc::now().to_rfc3339(),
        device: format!("{target_ip}:{target_port}"),
        set_name: req.set_name.clone(),
        success: true,
        message: msg.clone(),
        synced_count: total_written,
    });
    conflict::set_status("done", 1.0, Some(msg.clone()));
    let _ = app.emit(
        "sync-progress",
        SyncProgressEvent {
            current: total_written,
            total: total_written,
            phase: "done".into(),
        },
    );
    log::info!("[sync] 同步完成：{msg}");

    Ok(StartSyncResponse {
        success: true,
        message: msg,
        synced_count: total_written,
    })
}

// ==================== 工具 ====================

/// 读一帧（带超时）
async fn read_frame_timeout(stream: &mut TcpStream) -> Result<SyncEnvelope, AppError> {
    timeout(
        Duration::from_secs(FRAME_TIMEOUT_SECS),
        read_frame(stream),
    )
    .await
    .map_err(|_| AppError::SyncError("等待数据超时".into()))?
}

/// 读取一帧：先读 4 字节长度（大端），再读完整 JSON
async fn read_frame(stream: &mut TcpStream) -> Result<SyncEnvelope, AppError> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(|e| AppError::SyncError(format!("读取帧头失败：{e}")))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len == 0 || len > 64 * 1024 * 1024 {
        return Err(AppError::SyncError(format!("非法帧长度 {len}")));
    }
    let mut body = vec![0u8; len];
    stream
        .read_exact(&mut body)
        .await
        .map_err(|e| AppError::SyncError(format!("读取帧体失败：{e}")))?;
    // 重组完整帧（长度前缀 + 体）交给 decode_frame
    let mut frame = Vec::with_capacity(4 + len);
    frame.extend_from_slice(&len_buf);
    frame.extend_from_slice(&body);
    decode_frame(&frame)
}

/// 校验并归一化记忆集名称，防路径穿越
///
/// `storage::set_index_path` 会把 set_name 直接 join 到记忆根目录下，
/// 所以来自网络的名称必须先过滤：只允许字母、数字、下划线、连字符、点与中日韩字符，
/// 且不得含路径分隔符、`..`、盘符冒号。
fn sanitize_set_name(raw: &str) -> Result<String, AppError> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(AppError::SyncError("记忆集名称为空".into()));
    }
    if name.len() > 64 {
        return Err(AppError::SyncError("记忆集名称过长".into()));
    }
    if name.contains('/')
        || name.contains('\\')
        || name.contains(':')
        || name.contains('\0')
        || name == ".."
        || name == "."
        || name.contains("..")
    {
        return Err(AppError::SyncError(format!(
            "非法记忆集名称（含路径分隔符或上级目录）：{name}"
        )));
    }
    if name
        .chars()
        .any(|c| c.is_control() || matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
    {
        return Err(AppError::SyncError(format!(
            "非法记忆集名称（含控制字符或保留符号）：{name}"
        )));
    }
    Ok(name.to_string())
}

/// 本机某记忆集最后一条记忆的时间戳（增量同步基准）
/// 仅返回符合 ISO 8601 格式的时间戳，防止非法值混入同步请求
fn last_sync_time(set_name: &str) -> Option<String> {
    let path = storage::set_index_path(Some(set_name));
    let all = storage::read_all(&path).ok()?;
    all.iter()
        .map(|m| &m.timestamp)
        .filter(|ts| chrono::DateTime::parse_from_rfc3339(ts).is_ok())
        .max()
        .cloned()
}

/// 带全局写锁的整表原子写入（同步合并后落盘）
fn write_merged_atomic(path: &std::path::PathBuf, memories: &[Memory]) -> Result<(), AppError> {
    let _guard = MEMORY_WRITER_LOCK
        .lock()
        .map_err(|_| AppError::MemoryError("记忆写锁获取失败".into()))?;
    storage::atomic_write_index(path, memories)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 记忆集名_合法值通过() {
        for ok in ["default", "工作", "my-set", "set_1", "v1.0"] {
            assert!(sanitize_set_name(ok).is_ok(), "应通过：{ok}");
        }
        // 前后空白被裁剪
        assert_eq!(sanitize_set_name("  default  ").unwrap(), "default");
    }

    #[test]
    fn 记忆集名_路径穿越被拒() {
        for bad in [
            "../secret",
            "..\\secret",
            "a/b",
            "a\\b",
            "C:evil",
            "..",
            ".",
            "foo..bar",
        ] {
            assert!(sanitize_set_name(bad).is_err(), "应拒绝：{bad}");
        }
    }

    #[test]
    fn 记忆集名_空与超长被拒() {
        assert!(sanitize_set_name("").is_err());
        assert!(sanitize_set_name("   ").is_err());
        assert!(sanitize_set_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn 记忆集名_控制字符与保留符号被拒() {
        for bad in ["a\0b", "a\nb", "a<b", "a>b", "a|b", "a?b", "a*b", "a\"b"] {
            assert!(sanitize_set_name(bad).is_err(), "应拒绝：{bad}");
        }
    }

    #[test]
    fn 认证_未配对设备无法伪造mac() {
        // 服务端密钥（主人的主密码派生）与攻击者密钥不同
        let server_key = crate::config::encryption::derive_key("主人的密码", b"fixed-salt-16b!!");
        let attacker_key = crate::config::encryption::derive_key("猜的密码", b"fixed-salt-16b!!");
        let nonce = generate_nonce_b64();
        // 攻击者用自己的密钥算 MAC → 服务端校验必须失败
        let forged = compute_auth_mac(&attacker_key, &nonce);
        assert!(!verify_auth_mac(&server_key, &nonce, &forged));
        // 同密码派生的合法设备可通过
        let legit = compute_auth_mac(&server_key, &nonce);
        assert!(verify_auth_mac(&server_key, &nonce, &legit));
    }

    #[test]
    fn 认证_抓包重放对新连接无效() {
        let key = crate::config::encryption::derive_key("主人的密码", b"fixed-salt-16b!!");
        // 攻击者抓到第一次连接的 (nonce, mac)
        let sniffed_nonce = generate_nonce_b64();
        let sniffed_mac = compute_auth_mac(&key, &sniffed_nonce);
        // 新连接服务端下发新 nonce，重放的 mac 失效
        let fresh_nonce = generate_nonce_b64();
        assert_ne!(sniffed_nonce, fresh_nonce);
        assert!(!verify_auth_mac(&key, &fresh_nonce, &sniffed_mac));
    }

    #[test]
    fn 认证_帧序列化往返() {
        let key = [4u8; 32];
        let nonce = generate_nonce_b64();
        let env = SyncEnvelope::Auth {
            auth: SyncAuth {
                device_id: "dev-x".into(),
                mac: compute_auth_mac(&key, &nonce),
            },
        };
        let frame = encode_frame(&env).unwrap();
        match decode_frame(&frame).unwrap() {
            SyncEnvelope::Auth { auth } => {
                assert!(verify_auth_mac(&key, &nonce, &auth.mac));
            }
            _ => panic!("类型不匹配"),
        }
    }
}
