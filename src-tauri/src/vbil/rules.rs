// 《铃·记忆体》VBIL 模块 —— 本地规则引擎
//
// 规则由本地配置 vbil_rules.json 驱动，不依赖 AI。
// 每条规则：source_id（可 * 表所有）、event_type、cooldown（秒）、response（固定回复，rule_only 用）。
// 输出 should_respond + response_text。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

/// 单条规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VbilRule {
    /// 源客户端 id（* 表示所有客户端）
    pub source_id: String,
    /// 要匹配的事件类型（startup / idle / active / speaking / listening / shutdown）
    pub event_type: String,
    /// 冷却时间（秒），0 表示不冷却
    pub cooldown: u64,
    /// 固定回复内容（rule_only 模式使用）
    pub response: String,
}

/// 规则匹配结果
#[derive(Debug, Clone)]
pub struct RuleMatch {
    /// 是否响应
    pub should_respond: bool,
    /// 固定回复文本（rule_only 模式）
    pub response_text: Option<String>,
}

/// 规则缓存（进程内共享，setup 时加载）
static RULES: LazyLock<Mutex<Vec<VbilRule>>> = LazyLock::new(|| Mutex::new(Vec::new()));
/// 冷却记录：(source_id|event_type) -> 最近触发时刻
static LAST_TRIGGER: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 规则文件路径：%APPDATA%/Memoria/vbil_rules.json
fn rules_path() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    PathBuf::from(appdata).join("Memoria").join("vbil_rules.json")
}

/// 默认规则
pub fn default_rules() -> Vec<VbilRule> {
    vec![
        VbilRule {
            source_id: "*".to_string(),
            event_type: "startup".to_string(),
            cooldown: 60,
            response: "欢迎～".to_string(),
        },
        VbilRule {
            source_id: "*".to_string(),
            event_type: "speaking".to_string(),
            cooldown: 10,
            response: "嗯嗯，我在听～".to_string(),
        },
    ]
}

/// 从文件加载规则；文件不存在时写入默认规则
pub fn load_rules() -> Vec<VbilRule> {
    let path = rules_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| default_rules()),
        Err(_) => {
            // 首次运行：生成默认规则文件
            let d = default_rules();
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(s) = serde_json::to_string_pretty(&d) {
                let _ = std::fs::write(&path, s);
            }
            d
        }
    }
}

/// 初始化规则引擎（setup 时调用）
pub fn init() {
    let rules = load_rules();
    log::info!("[vbil] 规则引擎加载 {} 条规则", rules.len());
    *RULES.lock().unwrap() = rules;
}

/// 匹配事件（供 responder 调用）：读取全局规则与冷却状态
pub fn match_event(source_id: &str, event_type: &str) -> RuleMatch {
    let rules = RULES.lock().unwrap().clone();
    let mut last = LAST_TRIGGER.lock().unwrap();
    match_event_with(&rules, &mut last, source_id, event_type)
}

/// 纯函数版本：给定规则列表 + 冷却状态，匹配事件（便于单元测试，无全局依赖）
///
/// 命中第一条匹配规则即返回；冷却期内返回不响应。
pub fn match_event_with(
    rules: &[VbilRule],
    last_trigger: &mut HashMap<String, Instant>,
    source_id: &str,
    event_type: &str,
) -> RuleMatch {
    for rule in rules {
        // source_id 匹配（* 通配所有）
        if rule.source_id != "*" && rule.source_id != source_id {
            continue;
        }
        // event_type 匹配
        if rule.event_type != event_type {
            continue;
        }
        // 冷却检查
        if rule.cooldown > 0 {
            let key = format!("{}|{}", source_id, event_type);
            if let Some(t) = last_trigger.get(&key) {
                if t.elapsed().as_secs() < rule.cooldown {
                    return RuleMatch {
                        should_respond: false,
                        response_text: None,
                    };
                }
            }
            last_trigger.insert(key, Instant::now());
        }
        return RuleMatch {
            should_respond: true,
            response_text: Some(rule.response.clone()),
        };
    }
    RuleMatch {
        should_respond: false,
        response_text: None,
    }
}

/// 获取规则列表（前端展示/调试）
pub fn get_rules() -> Vec<VbilRule> {
    RULES.lock().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn default_rules_have_two_entries() {
        let d = default_rules();
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].event_type, "startup");
        assert_eq!(d[1].event_type, "speaking");
    }

    #[test]
    fn match_event_hits_rule() {
        let rules = default_rules();
        let mut last = HashMap::new();
        let m = match_event_with(&rules, &mut last, "client-a", "startup");
        assert!(m.should_respond);
        assert_eq!(m.response_text.as_deref(), Some("欢迎～"));
    }

    #[test]
    fn match_event_miss_unknown() {
        let rules = default_rules();
        let mut last = HashMap::new();
        let m = match_event_with(&rules, &mut last, "client-a", "shutdown");
        assert!(!m.should_respond);
        assert!(m.response_text.is_none());
    }

    #[test]
    fn source_id_filter() {
        let rules = vec![VbilRule {
            source_id: "only-a".to_string(),
            event_type: "startup".to_string(),
            cooldown: 0,
            response: "专属".to_string(),
        }];
        let mut last = HashMap::new();
        assert!(match_event_with(&rules, &mut last, "only-a", "startup").should_respond);
        assert!(!match_event_with(&rules, &mut last, "other", "startup").should_respond);
    }

    #[test]
    fn cooldown_blocks_repeat() {
        let rules = vec![VbilRule {
            source_id: "*".to_string(),
            event_type: "idle".to_string(),
            cooldown: 60,
            response: "在呢".to_string(),
        }];
        let mut last = HashMap::new();
        assert!(match_event_with(&rules, &mut last, "client-a", "idle").should_respond);
        // 立即重复：冷却中，不响应
        assert!(!match_event_with(&rules, &mut last, "client-a", "idle").should_respond);
    }
}
