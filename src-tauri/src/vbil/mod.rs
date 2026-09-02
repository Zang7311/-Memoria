// 《铃·记忆体》VBIL 模块 —— 模块导出
//
// 虚拟形象互联层（VBIL）：本地 TCP 服务，让其他桌面虚拟形象通过标准协议与铃通信。
// 子模块：
//   - types.rs          协议数据模型（VBIL/0.1）
//   - client_manager.rs 客户端管理（在线列表 + 心跳）
//   - server.rs         TCP 服务端 + 解析 + 路由 + 心跳检测
//   - config.rs         端口与配置（vbil.json 读写）
//   - rules.rs          本地规则引擎
//   - responder.rs      响应桥接（off / rule_only / ai 三模式）
//   - scanner.rs        形象扫描器（枚举窗口）

pub mod client_manager;
pub mod commands;
pub mod config;
pub mod responder;
pub mod rules;
pub mod scanner;
pub mod server;
pub mod types;

// 客户端管理
pub use client_manager::{ClientInfo, ClientManager};

// 端口与配置
pub use config::{
    get_mode, get_vbil_status, get_whitelist, is_allowed, read_port_config, set_mode,
    set_vbil_enabled, write_port_config, VbilConfig,
};

// 规则引擎
pub use rules::{match_event, RuleMatch, VbilRule};

// 响应桥接
pub use responder::spawn_responder;

// 形象扫描
pub use scanner::{scan_windows, SuspectedAvatar};

// TCP 服务端与对外接口
pub use server::{
    get_port, init, list_online_clients, recv_event, send_action, spawn_listener, EventSender,
    OnlineClient, VbilError,
};

// 协议数据模型与工具
pub use types::{
    generate_message_id, now_iso8601, parse_message, IncomingEvent, ParseError, VBILMessage,
    DEFAULT_PORT, PROTOCOL_VERSION,
};
