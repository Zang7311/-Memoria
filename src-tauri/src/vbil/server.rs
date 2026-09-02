// 《铃·记忆体》VBIL 模块 —— TCP 服务端 + 协议解析 + 消息路由 + 心跳检测
//
// 职责（开发者一）：
//   1. tokio::net::TcpListener 监听 127.0.0.1 指定端口（冲突自动递增）
//   2. 每连接一个 tokio::spawn 任务，JSON + 换行符分隔解析
//   3. 按消息 type 路由到对应处理
//   4. 每 30 秒心跳，连续 2 次无 pong 则移除
//   5. 事件经内部通道转发给开发者二（规则引擎），并暴露 send_action 供其回发
//
// 边界：不含规则引擎/响应策略（开发者二）；不含窗口扫描/设置页（开发者二）。

use crate::vbil::client_manager::ClientManager;
use crate::vbil::config;
use crate::vbil::types::{
    generate_message_id, now_iso8601, parse_message, IncomingEvent, VBILMessage, DEFAULT_PORT,
    EVENT_DEDUP_WINDOW_SECS, HEARTBEAT_INTERVAL_SECS, HEARTBEAT_TIMEOUT_SECS, PROTOCOL_VERSION,
};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, Mutex};

/// 事件通道发送端类型（开发者二订阅接收端）
pub type EventSender = mpsc::UnboundedSender<IncomingEvent>;

/// VBIL 模块错误
#[derive(Debug, thiserror::Error)]
pub enum VbilError {
    /// 模块尚未初始化
    #[error("VBIL 模块未初始化")]
    NotInitialized,
    /// 目标客户端不存在
    #[error("客户端不存在：{0}")]
    ClientNotFound(String),
    /// 序列化失败
    #[error("序列化失败：{0}")]
    Serialize(#[from] serde_json::Error),
}

/// 单个连接的句柄（写入端 + 断开信号）
struct ConnectionHandle {
    writer_tx: mpsc::UnboundedSender<String>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

/// 全局共享状态
struct SharedState {
    manager: Arc<ClientManager>,
    event_tx: EventSender,
    writers: Mutex<HashMap<String, ConnectionHandle>>,
    port: AtomicU16,
    /// 事件去重缓存：(id, event) -> 最近一次出现时刻
    dedup: Mutex<HashMap<(String, String), Instant>>,
}

impl SharedState {
    /// 事件去重：同一 id + 同一 event 在窗口期内重复则丢弃
    async fn is_duplicate(&self, id: &str, event: &str) -> bool {
        let key = (id.to_string(), event.to_string());
        let mut map = self.dedup.lock().await;
        let now = Instant::now();
        // 顺带清理过期条目，避免无限增长
        map.retain(|_, t| {
            now.duration_since(*t) < Duration::from_secs(EVENT_DEDUP_WINDOW_SECS)
        });
        if map.contains_key(&key) {
            return true;
        }
        map.insert(key, now);
        false
    }
}

/// 全局共享状态（init 时设置）
static STATE: OnceLock<Arc<SharedState>> = OnceLock::new();
/// 事件通道接收端（开发者二订阅）
static EVENT_RX: OnceLock<Mutex<mpsc::UnboundedReceiver<IncomingEvent>>> = OnceLock::new();
/// 监听中标志（防止重复启动）
static LISTENING: AtomicBool = AtomicBool::new(false);

// ==================== 动态端口发现 ====================
// 端口读写已由 config.rs 接管（开发者二），server 通过 config::read_port_config / config::write_port_config 调用。

/// 选择可用端口：从上次端口或默认 54547 起，冲突则递增绑定
async fn select_port() -> (TcpListener, u16) {
    let start = config::read_port_config().unwrap_or(DEFAULT_PORT);
    let mut port = start;
    loop {
        match TcpListener::bind(("127.0.0.1", port)).await {
            Ok(l) => return (l, port),
            Err(_) => {
                log::warn!("[vbil] 端口 {} 被占用，尝试 {}", port, port + 1);
                port += 1;
            }
        }
    }
}

// ==================== 对外接口 ====================

/// 初始化模块（lib.rs setup 调用）：创建全局状态与事件通道
pub fn init() {
    let (tx, rx) = mpsc::unbounded_channel::<IncomingEvent>();
    let state = Arc::new(SharedState {
        manager: Arc::new(ClientManager::new()),
        event_tx: tx,
        writers: Mutex::new(HashMap::new()),
        port: AtomicU16::new(DEFAULT_PORT),
        dedup: Mutex::new(HashMap::new()),
    });
    let _ = EVENT_RX.set(Mutex::new(rx));
    let _ = STATE.set(state);
    log::info!("[vbil] 模块初始化完成");
}

/// 启动 TCP 监听（lib.rs setup 调用，幂等）
pub fn spawn_listener(app: AppHandle) {
    let _ = app; // VBIL 暂不直接发前端事件，保留 AppHandle 以备后续扩展
    if LISTENING.swap(true, Ordering::SeqCst) {
        return;
    }
    let state = match STATE.get() {
        Some(s) => s.clone(),
        None => {
            log::error!("[vbil] 未初始化，无法启动监听");
            return;
        }
    };

    tauri::async_runtime::spawn(async move {
        let (listener, port) = select_port().await;
        state.port.store(port, Ordering::SeqCst);
        config::write_port_config(port);
        log::info!("[vbil] 服务监听 127.0.0.1:{}", port);

        // 心跳检测任务
        let hb_state = state.clone();
        tokio::spawn(async move { heartbeat_loop(hb_state).await });

        // accept 循环
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    log::debug!("[vbil] 新连接：{}", addr);
                    let st = state.clone();
                    tokio::spawn(async move {
                        handle_client(stream, st).await;
                    });
                }
                Err(e) => {
                    log::warn!("[vbil] accept 失败：{e}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    });
}

/// 向指定客户端发送动作（开发者二规则引擎调用）
pub async fn send_action(
    client_id: &str,
    action: &str,
    data: serde_json::Value,
) -> Result<(), VbilError> {
    let state = STATE.get().ok_or(VbilError::NotInitialized)?;
    let msg = VBILMessage::Action {
        protocol: PROTOCOL_VERSION.to_string(),
        message_id: generate_message_id(),
        timestamp: now_iso8601(),
        target: client_id.to_string(),
        action: action.to_string(),
        data: Some(data),
    };
    let json = serde_json::to_string(&msg)?;
    if send_to(state, client_id, &json).await {
        Ok(())
    } else {
        Err(VbilError::ClientNotFound(client_id.to_string()))
    }
}

/// 获取当前实际监听端口
pub fn get_port() -> u16 {
    STATE
        .get()
        .map(|s| s.port.load(Ordering::SeqCst))
        .unwrap_or(DEFAULT_PORT)
}

/// 在线客户端（前端展示用，时间格式化为字符串避免依赖 chrono serde）
#[derive(Debug, Clone, serde::Serialize)]
pub struct OnlineClient {
    pub id: String,
    pub name: Option<String>,
    pub capabilities: Vec<String>,
    pub connected_at: String,
    pub missed_pongs: u32,
}

/// 获取在线客户端列表（前端设置页实时展示）
pub async fn list_online_clients() -> Vec<OnlineClient> {
    let state = match STATE.get() {
        Some(s) => s,
        None => return Vec::new(),
    };
    let clients = state.manager.list_clients().await;
    clients
        .into_iter()
        .map(|c| OnlineClient {
            id: c.id,
            name: c.name,
            capabilities: c.capabilities,
            connected_at: c.connected_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            missed_pongs: c.missed_pongs,
        })
        .collect()
}

/// 接收一个入站事件（开发者二规则引擎订阅；模块未初始化时返回 None）
pub async fn recv_event() -> Option<IncomingEvent> {
    EVENT_RX.get()?.lock().await.recv().await
}

// ==================== 内部实现 ====================

/// 向指定客户端发送一行 JSON；返回是否成功投递
async fn send_to(state: &Arc<SharedState>, id: &str, json: &str) -> bool {
    let writers = state.writers.lock().await;
    match writers.get(id) {
        Some(h) => h.writer_tx.send(json.to_string()).is_ok(),
        None => false,
    }
}

/// 断开并清理客户端：移除管理记录 + 关闭连接
async fn disconnect(state: &Arc<SharedState>, id: &str) {
    state.manager.remove_client(id).await;
    if let Some(handle) = state.writers.lock().await.remove(id) {
        if let Some(tx) = handle.shutdown_tx {
            let _ = tx.send(());
        }
    }
}

/// 处理单个客户端连接（读写分离）
async fn handle_client(stream: TcpStream, state: Arc<SharedState>) {
    let (read_half, mut write_half) = stream.into_split();
    let (writer_tx, mut writer_rx) = mpsc::unbounded_channel::<String>();
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    // 写任务：消费 channel，逐行写入 socket
    tokio::spawn(async move {
        while let Some(line) = writer_rx.recv().await {
            if write_half.write_all(line.as_bytes()).await.is_err() {
                break;
            }
            if write_half.write_all(b"\n").await.is_err() {
                break;
            }
        }
    });

    let mut conn_id: Option<String> = None;
    let mut shutdown_tx = Some(shutdown_tx);
    let mut lines = BufReader::new(read_half).lines();

    loop {
        tokio::select! {
            res = lines.next_line() => {
                let line = match res {
                    Ok(Some(l)) => l,
                    Ok(None) | Err(_) => break,
                };
                let msg = match parse_message(&line) {
                    Ok(m) => m,
                    Err(e) => {
                        log::warn!("[vbil] 协议解析失败：{e}（原始：{}）", line);
                        continue;
                    }
                };
                match msg {
                    // register：注册客户端 + 登记连接句柄
                    VBILMessage::Register { id, name, capabilities, .. } => {
                        let caps = capabilities.unwrap_or_default();
                        state.manager.add_client(id.clone(), name.clone(), caps.clone()).await;
                        if let Some(tx) = shutdown_tx.take() {
                            let handle = ConnectionHandle {
                                writer_tx: writer_tx.clone(),
                                shutdown_tx: Some(tx),
                            };
                            state.writers.lock().await.insert(id.clone(), handle);
                        }
                        conn_id = Some(id.clone());
                        log::info!("[vbil] 客户端注册：id={} name={:?} capabilities={:?}", id, name, caps);
                    }
                    other => {
                        route(other, &state, &mut conn_id, &writer_tx).await;
                    }
                }
            }
            _ = &mut shutdown_rx => {
                // 收到断开信号
                break;
            }
        }
    }

    // 连接结束清理
    if let Some(id) = conn_id.take() {
        state.manager.remove_client(&id).await;
        state.writers.lock().await.remove(&id);
        log::info!("[vbil] 客户端断开：id={}", id);
    }
}

/// 消息路由（register 之外的其余消息）
async fn route(
    msg: VBILMessage,
    state: &Arc<SharedState>,
    conn_id: &mut Option<String>,
    writer_tx: &mpsc::UnboundedSender<String>,
) {
    match msg {
        VBILMessage::Event { id, event, data, .. } => {
            if state.is_duplicate(&id, &event).await {
                log::debug!("[vbil] 事件去重丢弃：id={} event={}", id, event);
                return;
            }
            log::info!("[vbil] 事件接收：id={} event={} data={:?}", id, event, data);
            let ev = IncomingEvent {
                from: id,
                event,
                data,
                timestamp: now_iso8601(),
            };
            if state.event_tx.send(ev).is_err() {
                log::warn!("[vbil] 事件通道已关闭，事件丢失");
            }
        }
        VBILMessage::Ack { in_reply_to, success, error, .. } => {
            log::info!(
                "[vbil] ack：in_reply_to={} success={} error={:?}",
                in_reply_to,
                success,
                error
            );
        }
        VBILMessage::Result { in_reply_to, success, data, .. } => {
            log::info!(
                "[vbil] result：in_reply_to={} success={} data={:?}",
                in_reply_to,
                success,
                data
            );
        }
        VBILMessage::Pong { in_reply_to, .. } => {
            if let Some(cid) = conn_id {
                state.manager.update_heartbeat(cid).await;
            }
            log::debug!("[vbil] pong：in_reply_to={}", in_reply_to);
        }
        VBILMessage::Ping { message_id, .. } => {
            // 客户端主动 ping，回 pong
            let pong = VBILMessage::Pong {
                protocol: PROTOCOL_VERSION.to_string(),
                message_id: generate_message_id(),
                timestamp: now_iso8601(),
                in_reply_to: message_id,
            };
            if let Ok(json) = serde_json::to_string(&pong) {
                let _ = writer_tx.send(json);
            }
        }
        VBILMessage::Unregister { id, .. } => {
            log::info!("[vbil] 客户端主动注销：id={}", id);
            disconnect(state, &id).await;
            *conn_id = None;
        }
        VBILMessage::Register { .. } => {
            unreachable!("register 已在 handle_client 中处理");
        }
        VBILMessage::Action { .. } => {
            log::warn!("[vbil] 收到意外的 action 消息（action 应由服务端发出）");
        }
    }
}

/// 心跳检测循环：每 30 秒发 ping，3 秒后未收到 pong 的计一次，连续 2 次移除
async fn heartbeat_loop(state: Arc<SharedState>) {
    let interval = Duration::from_secs(HEARTBEAT_INTERVAL_SECS);
    loop {
        tokio::time::sleep(interval).await;
        let ping_time = Utc::now();
        let clients = state.manager.list_clients().await;

        // 向所有客户端发 ping
        for c in &clients {
            let msg = VBILMessage::Ping {
                protocol: PROTOCOL_VERSION.to_string(),
                message_id: generate_message_id(),
                timestamp: now_iso8601(),
            };
            if let Ok(json) = serde_json::to_string(&msg) {
                let _ = send_to(&state, &c.id, &json).await;
            }
        }

        // 等待 pong 超时
        tokio::time::sleep(Duration::from_secs(HEARTBEAT_TIMEOUT_SECS)).await;

        // 检查未响应者
        for c in &clients {
            if let Some(info) = state.manager.get_client(&c.id).await {
                if info.last_heartbeat < ping_time {
                    // 本轮未收到 pong
                    if let Some(stale) = state.manager.mark_missed_pong(&c.id).await {
                        if stale {
                            log::warn!("[vbil] 心跳超时，移除客户端：id={}", c.id);
                            disconnect(&state, &c.id).await;
                        }
                    }
                }
            }
        }
    }
}
