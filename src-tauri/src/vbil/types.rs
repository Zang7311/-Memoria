// 《铃·记忆体》VBIL 模块 —— 协议数据模型
//
// 虚拟形象互联层（VBIL）协议 VBIL/0.1：
//   - 传输：TCP（127.0.0.1），JSON + 换行符分隔
//   - 所有消息通用字段：protocol / message_id / type / timestamp
//   - 8 种消息类型：register / event / action / ack / result / ping / pong / unregister
//
// 本文件只定义数据结构与解析，不含任何业务逻辑（规则引擎、响应策略属开发者二）。

use serde::{Deserialize, Serialize};

/// 协议版本标识（固定）
pub const PROTOCOL_VERSION: &str = "VBIL/0.1";

/// 默认监听端口（被占用时自动递增 54548、54549……）
pub const DEFAULT_PORT: u16 = 54547;

/// 心跳探测间隔（秒）
pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// 心跳响应超时（秒）
pub const HEARTBEAT_TIMEOUT_SECS: u64 = 3;

/// 连续未响应次数阈值（超过则判定超时移除）
pub const MISSED_PONG_LIMIT: u32 = 2;

/// 事件去重窗口（秒）
pub const EVENT_DEDUP_WINDOW_SECS: u64 = 2;

/// 生成服务端消息的 message_id：uuid v4 前 8 位 + 时间戳后 6 位（HHMMSS）
/// 示例：a1b2c3d4_143030
pub fn generate_message_id() -> String {
    let short = &uuid::Uuid::new_v4().simple().to_string()[..8];
    let timepart = chrono::Utc::now().format("%H%M%S").to_string();
    format!("{}_{}", short, timepart)
}

/// 生成当前 UTC 时间的 ISO 8601 格式字符串（如 2026-09-03T14:30:00Z）
pub fn now_iso8601() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// 协议解析错误
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// JSON 反序列化失败
    #[error("JSON 解析失败：{0}")]
    Json(#[from] serde_json::Error),

    /// 协议版本不匹配
    #[error("协议版本不匹配：{0}")]
    Protocol(String),

    /// 缺少 message_id
    #[error("缺少 message_id")]
    MissingMessageId,
}

/// VBIL 消息（协议层统一结构，serde 按 type 字段分派到对应 variant）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum VBILMessage {
    /// 客户端注册（建立连接后首先发送，表明身份与能力）
    Register {
        protocol: String,
        message_id: String,
        timestamp: String,
        /// 实例唯一 ID（客户端启动时生成，推荐 UUID）
        id: String,
        /// 显示名称
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// 客户端版本号
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        /// 能力清单（event.send / event.receive / text.display / speech / animation）
        #[serde(skip_serializing_if = "Option::is_none")]
        capabilities: Option<Vec<String>>,
    },
    /// 事件通知（客户端向铃发送状态变化）
    Event {
        protocol: String,
        message_id: String,
        timestamp: String,
        /// 发送方实例 ID
        id: String,
        /// 事件类型（startup / idle / active / speaking / listening / shutdown）
        event: String,
        /// 扩展数据对象
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
    },
    /// 动作请求（铃向特定客户端发送）
    Action {
        protocol: String,
        message_id: String,
        timestamp: String,
        /// 目标实例 ID
        target: String,
        /// 动作类型（show_text / speak / play_animation 等）
        action: String,
        /// 动作参数
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
    },
    /// 确认接收
    Ack {
        protocol: String,
        message_id: String,
        timestamp: String,
        /// 对应消息的 message_id
        in_reply_to: String,
        /// true / false
        success: bool,
        /// 失败时的错误信息
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// 执行结果（接收方执行完 action 后返回）
    Result {
        protocol: String,
        message_id: String,
        timestamp: String,
        in_reply_to: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
    },
    /// 心跳探测（服务端 -> 客户端）
    Ping {
        protocol: String,
        message_id: String,
        timestamp: String,
    },
    /// 心跳响应（客户端 -> 服务端）
    Pong {
        protocol: String,
        message_id: String,
        timestamp: String,
        in_reply_to: String,
    },
    /// 主动注销（客户端主动下线）
    Unregister {
        protocol: String,
        message_id: String,
        timestamp: String,
        id: String,
    },
}

impl VBILMessage {
    /// 取通用字段 message_id（链路追踪）
    pub fn message_id(&self) -> &str {
        match self {
            VBILMessage::Register { message_id, .. }
            | VBILMessage::Event { message_id, .. }
            | VBILMessage::Action { message_id, .. }
            | VBILMessage::Ack { message_id, .. }
            | VBILMessage::Result { message_id, .. }
            | VBILMessage::Ping { message_id, .. }
            | VBILMessage::Pong { message_id, .. }
            | VBILMessage::Unregister { message_id, .. } => message_id,
        }
    }

    /// 取消息类型名（用于日志）
    pub fn kind(&self) -> &'static str {
        match self {
            VBILMessage::Register { .. } => "register",
            VBILMessage::Event { .. } => "event",
            VBILMessage::Action { .. } => "action",
            VBILMessage::Ack { .. } => "ack",
            VBILMessage::Result { .. } => "result",
            VBILMessage::Ping { .. } => "ping",
            VBILMessage::Pong { .. } => "pong",
            VBILMessage::Unregister { .. } => "unregister",
        }
    }
}

/// 解析一行 JSON 为 VBILMessage
///
/// 解析成功后校验协议版本与 message_id 必填项。
pub fn parse_message(line: &str) -> Result<VBILMessage, ParseError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(ParseError::Json(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "空行",
        ))));
    }
    let msg: VBILMessage = serde_json::from_str(trimmed)?;

    // 校验协议版本
    match &msg {
        VBILMessage::Register { protocol, .. }
        | VBILMessage::Event { protocol, .. }
        | VBILMessage::Action { protocol, .. }
        | VBILMessage::Ack { protocol, .. }
        | VBILMessage::Result { protocol, .. }
        | VBILMessage::Ping { protocol, .. }
        | VBILMessage::Pong { protocol, .. }
        | VBILMessage::Unregister { protocol, .. } => {
            if protocol != PROTOCOL_VERSION {
                return Err(ParseError::Protocol(protocol.clone()));
            }
        }
    }

    // message_id 在反序列化时已由 serde 强制必填（无 default），这里兜底判断空串
    if msg.message_id().is_empty() {
        return Err(ParseError::MissingMessageId);
    }

    Ok(msg)
}

/// 转发给规则引擎（开发者二）的入站事件
#[derive(Debug, Clone)]
pub struct IncomingEvent {
    /// 客户端 id
    pub from: String,
    /// 事件类型
    pub event: String,
    /// 扩展数据
    pub data: Option<serde_json::Value>,
    /// 原始时间戳
    pub timestamp: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_register() -> &'static str {
        r#"{
            "protocol": "VBIL/0.1",
            "message_id": "a1b2c3d4_143022",
            "type": "register",
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "SSP桌宠",
            "version": "1.0.0",
            "capabilities": ["event.send", "text.display"],
            "timestamp": "2026-09-03T14:30:00Z"
        }"#
    }

    #[test]
    fn parse_register() {
        let msg = parse_message(sample_register()).unwrap();
        match msg {
            VBILMessage::Register { id, name, version, capabilities, .. } => {
                assert_eq!(id, "550e8400-e29b-41d4-a716-446655440000");
                assert_eq!(name.as_deref(), Some("SSP桌宠"));
                assert_eq!(version.as_deref(), Some("1.0.0"));
                assert_eq!(capabilities.as_deref().unwrap().len(), 2);
            }
            other => panic!("应为 Register，实际为 {}", other.kind()),
        }
    }

    #[test]
    fn parse_ping() {
        let line = r#"{"protocol":"VBIL/0.1","message_id":"a1b2c3d4_143040","type":"ping","timestamp":"2026-09-03T14:30:30Z"}"#;
        let msg = parse_message(line).unwrap();
        assert!(matches!(msg, VBILMessage::Ping { .. }));
        assert_eq!(msg.message_id(), "a1b2c3d4_143040");
    }

    #[test]
    fn parse_rejects_wrong_protocol() {
        let line = r#"{"protocol":"VBIL/9.9","message_id":"x","type":"ping","timestamp":"2026-09-03T14:30:30Z"}"#;
        let err = parse_message(line).unwrap_err();
        assert!(matches!(err, ParseError::Protocol(_)));
    }

    #[test]
    fn parse_rejects_bad_json() {
        assert!(parse_message("not json").is_err());
    }

    #[test]
    fn message_id_format() {
        let id = generate_message_id();
        // 8 位 hex + '_' + 6 位时间
        assert_eq!(id.len(), 15);
        assert_eq!(id.as_bytes()[8], b'_');
    }
}
