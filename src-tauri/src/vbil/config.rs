// 《铃·记忆体》VBIL 模块 —— 端口与配置（vbil.json 读写）
//
// 配置路径：%APPDATA%/Memoria/vbil.json
// 字段：enabled（总开关）、port（监听端口）、protocol、mode（off/rule_only/ai）、whitelist（客户端白名单）

use crate::vbil::types::{DEFAULT_PORT, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// VBIL 配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VbilConfig {
    /// VBIL 总开关（业务层是否响应事件，默认关闭）
    #[serde(default)]
    pub enabled: bool,
    /// 监听端口
    #[serde(default = "default_port")]
    pub port: u16,
    /// 协议版本
    #[serde(default = "default_protocol")]
    pub protocol: String,
    /// 响应模式：off / rule_only / ai
    #[serde(default = "default_mode")]
    pub mode: String,
    /// 白名单客户端 ID（空 = 全部允许）
    #[serde(default)]
    pub whitelist: Vec<String>,
}

fn default_port() -> u16 {
    DEFAULT_PORT
}
fn default_protocol() -> String {
    PROTOCOL_VERSION.to_string()
}
fn default_mode() -> String {
    "rule_only".to_string()
}

impl Default for VbilConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            protocol: PROTOCOL_VERSION.to_string(),
            mode: "rule_only".to_string(),
            whitelist: Vec::new(),
        }
    }
}

/// 配置文件路径
fn config_path() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    PathBuf::from(appdata).join("Memoria").join("vbil.json")
}

/// 读取配置（文件不存在或损坏返回默认值）
pub fn read_config() -> VbilConfig {
    match std::fs::read_to_string(config_path()) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => VbilConfig::default(),
    }
}

/// 保存配置（失败仅记日志，不阻塞）
pub fn save_config(cfg: &VbilConfig) {
    let path = config_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match serde_json::to_string_pretty(cfg) {
        Ok(s) => {
            if let Err(e) = std::fs::write(&path, s) {
                log::warn!("[vbil] 写入 vbil.json 失败：{e}");
            }
        }
        Err(e) => log::warn!("[vbil] 序列化配置失败：{e}"),
    }
}

/// 写入当前端口（server 端口变更时调用）
pub fn write_port_config(port: u16) {
    let mut cfg = read_config();
    cfg.port = port;
    save_config(&cfg);
    log::info!("[vbil] 端口 {} 已写入 vbil.json", port);
}

/// 读取端口（调试 / 端口选择起点）
pub fn read_port_config() -> Option<u16> {
    Some(read_config().port)
}

/// 获取 VBIL 是否启用
pub fn get_vbil_status() -> bool {
    read_config().enabled
}

/// 设置启用状态（前端设置页调用）
pub fn set_vbil_enabled(enabled: bool) {
    let mut cfg = read_config();
    cfg.enabled = enabled;
    save_config(&cfg);
    log::info!("[vbil] 总开关设为 {}", enabled);
}

/// 获取响应模式
pub fn get_mode() -> String {
    read_config().mode
}

/// 设置响应模式（实时生效）
pub fn set_mode(mode: &str) {
    let mut cfg = read_config();
    cfg.mode = mode.to_string();
    save_config(&cfg);
    log::info!("[vbil] 响应模式设为 {}", mode);
}

/// 获取白名单
pub fn get_whitelist() -> Vec<String> {
    read_config().whitelist
}

/// 检查客户端是否被允许（空白名单 = 全部允许）
pub fn is_allowed(client_id: &str) -> bool {
    let cfg = read_config();
    cfg.whitelist.is_empty() || cfg.whitelist.iter().any(|x| x == client_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let c = VbilConfig::default();
        assert!(!c.enabled);
        assert_eq!(c.port, DEFAULT_PORT);
        assert_eq!(c.mode, "rule_only");
        assert!(c.whitelist.is_empty());
    }

    #[test]
    fn whitelist_allows_all_when_empty() {
        let c = VbilConfig::default();
        assert!(c.whitelist.is_empty());
        // 空白名单时任意客户端都允许（逻辑在 is_allowed，这里只验证字段默认）
        assert_eq!(c.whitelist.len(), 0);
    }

    #[test]
    fn deserialize_missing_fields_uses_defaults() {
        let json = r#"{"port": 54550}"#;
        let c: VbilConfig = serde_json::from_str(json).unwrap();
        assert_eq!(c.port, 54550);
        assert!(!c.enabled);
        assert_eq!(c.mode, "rule_only");
    }
}
