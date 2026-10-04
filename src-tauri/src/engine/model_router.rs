// 《铃·记忆体》难度路由（engine/model_router.rs）
//
// 目的：省 token。闲聊走「便宜模型」，任务类消息走「能力强的模型」。
//
// 依据 Anthropic《Building Effective Agents》里的 Routing 模式原文：
//   "Routing easy/common questions to smaller, cost-efficient models like Claude Haiku
//    and hard/unusual questions to more capable models ... to optimize for best performance."
//
// 设计原则：
//   1. **默认关闭** —— cheap_model 留空时一律用 api_model，行为与旧版完全一致；
//      只有用户显式填了便宜模型，路由才生效。绝不擅自改变既有行为。
//   2. **默认零额外调用** —— 默认使用本地启发式；只有用户打开 AI 路由开关时，
//      才为选择模型额外发起一次极小的分类调用。
//   3. **拿不准就给强的** —— 宁可多花一点，也不要该干活时派出弱模型。

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;
use crate::types::{AppConfig, ModelSlot};

static CHEAP_SLOT_COUNTERS: OnceLock<Mutex<HashMap<Vec<String>, usize>>> = OnceLock::new();

pub fn main_slot(cfg: &AppConfig) -> Option<&ModelSlot> {
    role_slots(cfg, "main").next()
}

pub fn vision_slot(cfg: &AppConfig) -> Option<&ModelSlot> {
    role_slots(cfg, "vision").next().or_else(|| main_slot(cfg))
}

fn role_slots<'config>(cfg: &'config AppConfig, role: &'config str) -> impl Iterator<Item = &'config ModelSlot> {
    cfg.models.iter().filter(move |slot| slot.enabled
        && slot.roles.iter().any(|tag| tag == role))
}

pub fn next_cheap_slot(cfg: &AppConfig) -> Option<&ModelSlot> {
    let slots: Vec<_> = role_slots(cfg, "cheap").collect();
    if slots.is_empty() { return None; }
    let pool = slots.iter().map(|slot| slot.id.clone()).collect();
    let mut counters = CHEAP_SLOT_COUNTERS.get_or_init(|| Mutex::new(HashMap::new()))
        .lock().unwrap_or_else(|error| error.into_inner());
    if counters.len() >= ROUTER_CACHE_LIMIT && !counters.contains_key(&pool) { counters.clear(); }
    let counter = counters.entry(pool).or_insert(0);
    let slot = slots[*counter % slots.len()];
    *counter = (*counter + 1) % slots.len();
    Some(slot)
}

pub fn slot_ai_router_allowed(cfg: &AppConfig, cheap: Option<&ModelSlot>) -> bool {
    let Some(cheap) = cheap else { return false; };
    if !cheap.enabled || !cheap.roles.iter().any(|role| role == "cheap") { return false; }
    let main = main_slot(cfg);
    main.map(|main| main.id != cheap.id).unwrap_or(true)
        && ai_router_allowed(Some(&cheap.name), main.map(|slot| slot.name.as_str()).unwrap_or(&cfg.api_model))
}

pub fn config_ai_router_allowed(cfg: &AppConfig) -> bool {
    if cfg.models.is_empty() { return ai_router_allowed(cfg.cheap_model.as_deref(), &cfg.api_model); }
    role_slots(cfg, "cheap").any(|slot| slot_ai_router_allowed(cfg, Some(slot)))
}

pub fn pick_slot_with_verdict<'config>(
    cfg: &'config AppConfig, input: &str, has_image: bool, agent_mode: bool,
    cheap: Option<&'config ModelSlot>, ai_enabled: bool, verdict: Option<Verdict>,
) -> Option<&'config ModelSlot> {
    if agent_mode { return main_slot(cfg); }
    if has_image { return vision_slot(cfg); }
    if ai_enabled && slot_ai_router_allowed(cfg, cheap) {
        if let Some(verdict) = verdict {
            if verdict.needs_vision { return vision_slot(cfg); }
            if verdict.easy { return cheap.or_else(|| main_slot(cfg)); }
            return main_slot(cfg);
        }
    }
    if is_chat_only(input) { cheap.or_else(|| main_slot(cfg)) } else { main_slot(cfg) }
}

/// AI 路由的判定结果：难度和是否需要视觉模型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub easy: bool,
    pub needs_vision: bool,
}

// FIFO 缓存上限 64；命中不调整插入顺序，不是 LRU。
const ROUTER_CACHE_LIMIT: usize = 64;
static ROUTER_CACHE: OnceLock<Mutex<HashMap<u64, Verdict>>> = OnceLock::new();
static ROUTER_CACHE_ORDER: OnceLock<Mutex<VecDeque<u64>>> = OnceLock::new();

fn router_cache() -> &'static Mutex<HashMap<u64, Verdict>> {
    ROUTER_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn router_cache_order() -> &'static Mutex<VecDeque<u64>> {
    ROUTER_CACHE_ORDER.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn input_hash(input: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish()
}

fn cached_verdict_fifo(input: &str) -> Option<Verdict> {
    let key = input_hash(input);
    let cache = router_cache().lock().unwrap_or_else(|e| e.into_inner());
    cache.get(&key).copied()
}

fn cache_verdict_fifo(input: &str, verdict: Verdict) {
    let key = input_hash(input);
    let mut cache = router_cache().lock().unwrap_or_else(|e| e.into_inner());
    let mut order = router_cache_order().lock().unwrap_or_else(|e| e.into_inner());

    if cache.contains_key(&key) {
        cache.insert(key, verdict);
        return;
    }

    if cache.len() >= ROUTER_CACHE_LIMIT {
        if let Some(oldest) = order.pop_front() {
            cache.remove(&oldest);
        }
    }
    cache.insert(key, verdict);
    order.push_back(key);
}

/// 解析 AI 返回的难度和视觉结论。
pub fn parse_verdict(raw: &str) -> Option<Verdict> {
    let lower = raw.to_lowercase();
    let tokens: Vec<&str> = lower
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect();
    let has_easy = tokens.contains(&"easy");
    let has_hard = tokens.contains(&"hard");
    let has_text = tokens.contains(&"text");
    let has_vision = tokens.contains(&"vision");
    if tokens.contains(&"not") || tokens.contains(&"no")
        || has_easy == has_hard || has_text == has_vision
    {
        return None;
    }

    Some(Verdict {
        easy: has_easy,
        needs_vision: has_vision,
    })
}

/// 对同一输入复用缓存，供网络分类和单元测试共用。
#[cfg(test)]
fn classify_with_cache<F>(input: &str, classify: F) -> Option<Verdict>
where
    F: FnOnce() -> Option<Verdict>,
{
    if let Some(verdict) = cached_verdict_fifo(input) {
        return Some(verdict);
    }

    let verdict = classify()?;
    cache_verdict_fifo(input, verdict);
    Some(verdict)
}

/// 用便宜模型判定消息难度及是否需要视觉模型。
///
/// 任何网络、超时、状态码或解析失败都只返回 None，不影响主聊天流程。
pub async fn classify_with_ai(
    client: &reqwest::Client,
    base_url: &str,
    key: &str,
    model: &str,
    input: &str,
) -> Option<Verdict> {
    // 不允许用截断消息的结论降级整条消息；长输入连缓存也不采用。
    if input.chars().count() > 500 {
        return None;
    }
    if let Some(verdict) = cached_verdict_fifo(input) {
        return Some(verdict);
    }

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "判断用户这句话：①是闲聊(easy)还是要动手的任务(hard)；②是否需要看图片或屏幕(vision)还是不需要(text)。直接输出两个词，不要任何解释或分析。例如：easy text / hard text / hard vision / easy vision"
            },
            { "role": "user", "content": input }
        ],
        "temperature": 0.0,
        "max_tokens": 512,
    });
    let url = format!("{}/chat/completions", crate::utils::normalize_v1_url(base_url));

    let response = client
        .post(url)
        .bearer_auth(key)
        .json(&body)
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }

    let value: Value = response.json().await.ok()?;
    let raw = value
        .get("choices")?
        .get(0)?
        .get("message")?
        .get("content")?
        .as_str()?;
    let verdict = parse_verdict(raw)?;
    cache_verdict_fifo(input, verdict);
    Some(verdict)
}

/// 纯闲聊的长度上限（字符数）。超过这个长度就认为是有内容的话，交给能力强的模型。
const CHAT_MAX_CHARS: usize = 50;

/// 任务意图关键词：命中任意一个就直接交给能力强的模型。
///
/// 刻意保守：宁可误判成「任务」（多花钱但靠谱），也不要误判成「闲聊」（省钱但干不了活）。
const TASK_HINTS: &[&str] = &[
    // 软件与系统操作
    "打开", "关闭", "启动", "运行", "执行", "安装", "卸载", "重启", "关机", "注销",
    "清理", "扫描", "优化",
    // 文件
    "删除", "写入", "创建", "新建", "复制", "移动", "重命名", "压缩", "解压",
    "保存", "下载", "导出", "备份",
    // 查询与检索
    "搜索", "查找", "搜一下", "查一下", "帮我查", "帮我找", "有没有",
    // 内容处理
    "翻译", "总结", "整理", "改写", "润色", "提取",
    // 系统能力
    "截图", "截屏", "截个屏", "剪贴板", "提醒", "定时", "音量", "亮度", "注册表", "服务",
    // 显式求助
    "帮我", "帮忙", "替我",
];

/// 这句话是不是「纯闲聊」（不含任务意图、也不长）。
///
/// 抽成独立函数是为了让测试和路由决策用同一套判断，避免两处逻辑漂移。
pub fn is_chat_only(input: &str) -> bool {
    let t = input.trim();
    if t.is_empty() {
        return false; // 空输入拿不准，给强的
    }
    if t.chars().count() > CHAT_MAX_CHARS {
        return false;
    }
    !TASK_HINTS.iter().any(|k| t.contains(k))
}

/// 挑选本次对话该用哪个模型。
///
/// # 参数
/// - `input`: 用户这次的输入
/// - `has_image`: 这次是否带图片（便宜模型多半不支持视觉，不能冒险）
/// - `agent_mode`: 是否开着 Agent 模式（要调工具，必须用能力强的）
/// - `cheap`: 配置里的便宜模型名（None / 空 = 路由关闭）
/// - `capable`: 配置里的主力模型名
///
/// # 返回
/// 实际要用的模型名。路由关闭或判断不确定时，一律返回 `capable`。
pub fn pick_model(
    input: &str,
    has_image: bool,
    agent_mode: bool,
    cheap: Option<&str>,
    capable: &str,
) -> String {
    // 1) 没配便宜模型 → 路由关闭，一切照旧
    let Some(cheap) = cheap.map(str::trim).filter(|s| !s.is_empty()) else {
        return capable.to_string();
    };

    // 2) 带图片：便宜模型可能看不了图，不冒这个险
    if has_image {
        return capable.to_string();
    }

    // 3) Agent 模式：要选工具、要多轮，必须用能力强的
    if agent_mode {
        return capable.to_string();
    }

    // 4) 纯闲聊才降级到便宜模型
    if is_chat_only(input) {
        return cheap.to_string();
    }

    capable.to_string()
}

/// 模型能力类型：明确文本、疑似视觉与未知。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelKind {
    Text,
    Vision,
    Unknown,
}

/// 从模型名推测能力类型（启发式）。
///
/// 视觉关键词与设置页保持一致；没有能力线索的名字归为未知。
pub fn model_kind(name: &str) -> ModelKind {
    const VISION_HINTS: &[&str] = &[
        "vision", "vl", "4v", "4o", "4.1", "llava", "gemini", "gpt-4",
        "claude", "image", "omni", "看图", "视觉",
    ];
    let lower = name.trim().to_lowercase();
    if VISION_HINTS.iter().any(|hint| lower.contains(hint)) {
        ModelKind::Vision
    } else if ["text", "纯文本"].iter().any(|hint| lower.contains(hint)) {
        ModelKind::Text
    } else {
        ModelKind::Unknown
    }
}

/// 是否允许开启 AI 难度判断。
///
/// 主人规则（2026-10-04）：**只有配置了两个及以上「同类型」模型**
/// （例如都是纯文本模型）时才允许开启，且开不开始终由用户自己选。
/// 两个不同名字的模型类型须一致；双方均未知时允许，一方未知时拒绝。
pub fn ai_router_allowed(cheap: Option<&str>, capable: &str) -> bool {
    let Some(cheap) = cheap.map(str::trim).filter(|s| !s.is_empty()) else {
        return false;
    };
    let capable = capable.trim();
    if capable.is_empty() || cheap == capable {
        return false;
    }
    model_kind(cheap) == model_kind(capable)
}

/// 在保留现有三条强制规则的基础上应用 AI 路由结果。
pub fn pick_model_with_verdict(
    input: &str,
    has_image: bool,
    agent_mode: bool,
    cheap: Option<&str>,
    capable: &str,
    vision: Option<&str>,
    ai_router: bool,
    verdict: Option<Verdict>,
) -> String {
    let Some(cheap) = cheap.map(str::trim).filter(|s| !s.is_empty()) else {
        return capable.to_string();
    };
    if has_image || agent_mode {
        return capable.to_string();
    }
    // 主人规则：只有「两个同类型模型」才允许 AI 判断生效；类型不同时退回本地判断。
    if !ai_router || !ai_router_allowed(Some(cheap), capable) {
        return pick_model(input, false, false, Some(cheap), capable);
    }

    match verdict {
        Some(verdict) if verdict.needs_vision => vision
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(capable)
            .to_string(),
        Some(verdict) if verdict.easy => cheap.to_string(),
        Some(_) => capable.to_string(),
        None => pick_model(input, false, false, Some(cheap), capable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const CAP: &str = "glm-5.2";
    const CHEAP: &str = "deepseek-v4-flash";

    #[test]
    fn 模型类型按名字区分() {
        assert_eq!(model_kind("deepseek-v4-flash"), ModelKind::Unknown);
        assert_eq!(model_kind("glm-5.2"), ModelKind::Unknown);
        assert_eq!(model_kind("custom-text"), ModelKind::Text);
        assert_eq!(model_kind("glm-4v"), ModelKind::Vision);
        assert_eq!(model_kind("qwen-vl-max"), ModelKind::Vision);
    }

    #[test]
    fn 同类型两个模型时才允许开启ai路由() {
        assert!(ai_router_allowed(Some("deepseek-v4-flash"), "glm-5.2"));
        assert!(ai_router_allowed(Some("glm-4v"), "qwen-vl-max"));
        // 类型不同 / 缺一个 → 一律不允许
        assert!(!ai_router_allowed(Some("glm-4v"), "glm-5.2"));
        assert!(!ai_router_allowed(Some("deepseek-v4-flash"), "glm-4v"));
        assert!(!ai_router_allowed(None, "glm-5.2"));
        assert!(!ai_router_allowed(Some("   "), "glm-5.2"));
        assert!(!ai_router_allowed(Some("glm-5.3"), ""));
    }

    #[test]
    fn 视觉关键词与未知类型保守路由() {
        for name in [
            "custom-vision", "qwen-vl-max", "glm-4v", "gpt-4o", "gpt-4.1",
            "llava", "gemini", "gpt-4", "claude", "custom-image", "custom-omni",
            "看图模型", "视觉模型",
        ] {
            assert_eq!(model_kind(name), ModelKind::Vision, "{name}");
        }
        assert!(ai_router_allowed(Some("gpt-4o"), "qwen-vl-max"));
        assert!(!ai_router_allowed(Some("gpt-4o"), "deepseek-chat"));
        assert!(!ai_router_allowed(Some("deepseek-chat"), "gpt-4o"));
        assert!(ai_router_allowed(Some("deepseek-flash"), "deepseek-v4-pro"));
        assert!(ai_router_allowed(Some("small-text"), "large-text"));
        assert!(!ai_router_allowed(Some("small-text"), "gpt-4o"));
        assert!(!ai_router_allowed(Some("small-text"), "deepseek-chat"));
    }

    #[test]
    fn 同名模型去掉空格后仍不能开启ai路由() {
        for (cheap, capable) in [
            ("deepseek-chat", "deepseek-chat"),
            ("  deepseek-chat ", " deepseek-chat  "),
            ("gpt-4o", " gpt-4o "),
        ] {
            assert!(!ai_router_allowed(Some(cheap), capable));
        }
    }

    #[test]
    fn 类型不同时ai路由不生效_回退本地判断() {
        // 便宜模型是视觉模型、主力是纯文本 → 即使开关打开也不做 AI 路由
        assert_eq!(
            pick_model_with_verdict(
                "在吗",
                false,
                false,
                Some("glm-4v"),
                CAP,
                None,
                true,
                Some(Verdict { easy: true, needs_vision: false }),
            ),
            pick_model("在吗", false, false, Some("glm-4v"), CAP)
        );
    }

    #[test]
    fn parse_verdict_解析难度和视觉标记() {
        assert_eq!(parse_verdict("easy text"), Some(Verdict { easy: true, needs_vision: false }));
        assert_eq!(parse_verdict("EASY TEXT"), Some(Verdict { easy: true, needs_vision: false }));
        assert_eq!(parse_verdict("hard"), None);
        assert_eq!(parse_verdict("hard vision"), Some(Verdict { easy: false, needs_vision: true }));
        assert_eq!(parse_verdict("这需要动手 hard text"), Some(Verdict { easy: false, needs_vision: false }));
        assert_eq!(parse_verdict("需要看图 vision easy"), Some(Verdict { easy: true, needs_vision: true }));
        assert_eq!(parse_verdict(""), None);
        assert_eq!(parse_verdict("???"), None);
    }

    #[test]
    fn 分类缺维度否定冲突与子串一律拒绝() {
        for raw in [
            "easy", "hard", "vision", "text", "not easy text", "no hard vision",
            "easygoing", "easygoing text", "hardship vision", "easy textual",
            "easy hard text", "easy text vision", "", "???",
        ] {
            assert_eq!(parse_verdict(raw), None, "{raw}");
        }
        assert_eq!(parse_verdict("easy,text"), Some(Verdict { easy: true, needs_vision: false }));
        assert_eq!(parse_verdict("HARD / VISION"), Some(Verdict { easy: false, needs_vision: true }));
    }

    #[tokio::test]
    async fn 超过500字符不读取缓存也不发送网络请求() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("local server");
        listener.set_nonblocking(true).expect("nonblocking server");
        let url = format!("http://{}", listener.local_addr().expect("local address"));
        let client = reqwest::Client::builder().no_proxy().build().expect("client");
        for input in ["长".repeat(501), "x".repeat(501)] {
            cache_verdict_fifo(&input, Verdict { easy: true, needs_vision: false });
            assert_eq!(classify_with_ai(&client, &url, "test-key", CHEAP, &input).await, None);
            assert!(matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock));
            assert_eq!(pick_model_with_verdict(&input, false, false, Some(CHEAP), CAP, None, true, None), CAP);
        }
    }

    #[tokio::test]
    async fn 不超过500字符按原文发送并接受完整分类() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        for input in ["网络边界测试短消息".to_string(), "界".repeat(500)] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("local server");
            let url = format!("http://{}", listener.local_addr().expect("local address"));
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.expect("request");
                let mut request = Vec::new();
                let mut buffer = [0u8; 4096];
                let (body_start, body_length) = loop {
                    let count = socket.read(&mut buffer).await.expect("read headers");
                    assert!(count > 0);
                    request.extend_from_slice(&buffer[..count]);
                    if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..position]);
                        let length = headers.lines().find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse::<usize>().ok()).flatten()
                        }).expect("body length");
                        break (position + 4, length);
                    }
                };
                while request.len() < body_start + body_length {
                    let count = socket.read(&mut buffer).await.expect("read body");
                    assert!(count > 0);
                    request.extend_from_slice(&buffer[..count]);
                }
                let body: Value = serde_json::from_slice(&request[body_start..body_start + body_length]).expect("JSON request");
                let response = r#"{"choices":[{"message":{"content":"easy text"}}]}"#;
                let headers = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response.len());
                socket.write_all(headers.as_bytes()).await.expect("response headers");
                socket.write_all(response.as_bytes()).await.expect("response body");
                body
            });
            let client = reqwest::Client::builder().no_proxy().build().expect("client");
            assert_eq!(classify_with_ai(&client, &url, "test-key", CHEAP, &input).await, Some(Verdict { easy: true, needs_vision: false }));
            let body = tokio::time::timeout(Duration::from_secs(2), server).await.expect("server timeout").expect("server task");
            assert_eq!(body["messages"][1]["content"].as_str(), Some(input.as_str()));
        }
    }

    #[test]
    fn 缓存命中时不再调用分类器() {
        let input = "缓存测试_同一输入_不重复调用";
        let calls = AtomicUsize::new(0);
        let first = classify_with_cache(input, || {
            calls.fetch_add(1, Ordering::SeqCst);
            Some(Verdict { easy: true, needs_vision: false })
        });
        let second = classify_with_cache(input, || {
            calls.fetch_add(1, Ordering::SeqCst);
            Some(Verdict { easy: false, needs_vision: true })
        });
        assert_eq!(first, second);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn ai_router关闭时保持本地路由行为() {
        let long = "嗯嗯".repeat(30);
        for input in ["在吗", "帮我打开QQ", long.as_str()] {
            for has_image in [true, false] {
                for agent_mode in [true, false] {
                    for verdict in [
                        None,
                        Some(Verdict { easy: true, needs_vision: true }),
                        Some(Verdict { easy: false, needs_vision: false }),
                    ] {
                        assert_eq!(
                            pick_model_with_verdict(
                                input, has_image, agent_mode, Some(CHEAP), CAP,
                                Some("vision-model"), false, verdict,
                            ),
                            pick_model(input, has_image, agent_mode, Some(CHEAP), CAP)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn 需要视觉时绝不使用便宜模型() {
        assert_eq!(
            pick_model_with_verdict(
                "看一下",
                false,
                false,
                Some(CHEAP),
                CAP,
                Some("vision-model"),
                true,
                Some(Verdict { easy: true, needs_vision: true }),
            ),
            "vision-model"
        );
    }

    #[test]
    fn ai判不出来时回退本地判断() {
        assert_eq!(
            pick_model_with_verdict("在吗", false, false, Some(CHEAP), CAP, None, true, None),
            CHEAP
        );
        assert_eq!(
            pick_model_with_verdict("帮我打开QQ", false, false, Some(CHEAP), CAP, None, true, None),
            CAP
        );
    }

    #[test]
    fn 没配便宜模型时永远用主力模型() {
        // 这是最重要的保底：默认行为绝不能变
        let long = "给我讲个长篇故事".repeat(5);
        let cases: [&str; 4] = ["在吗", "今天好累", "帮我打开QQ", long.as_str()];
        for input in cases {
            for img in [true, false] {
                for agent in [true, false] {
                    assert_eq!(
                        pick_model(input, img, agent, None, CAP),
                        CAP,
                        "路由关闭时应当一律用主力模型"
                    );
                    assert_eq!(pick_model(input, img, agent, Some("   "), CAP), CAP);
                    for ai_router in [true, false] {
                        for cheap in [None, Some("   ")] {
                            assert_eq!(
                                pick_model_with_verdict(
                                    input, img, agent, cheap, CAP, Some("vision-model"),
                                    ai_router, Some(Verdict { easy: true, needs_vision: true }),
                                ),
                                CAP
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn 闲聊走便宜模型() {
        for input in ["在吗", "嗯嗯", "今天好累呀", "陪我聊会儿天", "晚安"] {
            assert_eq!(
                pick_model(input, false, false, Some(CHEAP), CAP),
                CHEAP,
                "「{input}」是闲聊，应该走便宜模型"
            );
        }
    }

    #[test]
    fn 任务类走主力模型() {
        for input in [
            "帮我打开QQ",
            "安装一下剪映",
            "把桌面那张图压缩一下",
            "搜一下今天天气",
            "截个屏",
            "关机",
            "删除这个文件",
        ] {
            assert_eq!(
                pick_model(input, false, false, Some(CHEAP), CAP),
                CAP,
                "「{input}」是任务，必须走主力模型"
            );
        }
    }

    #[test]
    fn 长消息即使没有关键词也走主力模型() {
        // 50 字以上的话，多半有正经内容，别省钱
        let long = "嗯嗯".repeat(30); // 60 字，且不含任务关键词
        assert_eq!(long.chars().count(), 60);
        assert!(!is_chat_only(&long), "60 字应超过闲聊上限");
        assert_eq!(pick_model(&long, false, false, Some(CHEAP), CAP), CAP);
    }

    #[test]
    fn 短闲聊仍在便宜模型范围内() {
        let short = "嗯嗯".repeat(20); // 40 字
        assert!(short.chars().count() <= CHAT_MAX_CHARS);
        assert!(is_chat_only(&short));
        assert_eq!(pick_model(&short, false, false, Some(CHEAP), CAP), CHEAP);
    }

    #[test]
    fn 带图片一定走主力模型() {
        // 哪怕只是闲聊，带图也不能交给可能不支持视觉的便宜模型
        assert_eq!(
            pick_model("在吗", true, false, Some(CHEAP), CAP),
            CAP,
            "带图必须用主力模型"
        );
    }

    #[test]
    fn agent模式一定走主力模型() {
        assert_eq!(
            pick_model("在吗", false, true, Some(CHEAP), CAP),
            CAP,
            "Agent 模式必须用主力模型"
        );
    }

    #[test]
    fn 空输入保守给主力模型() {
        assert_eq!(pick_model("", false, false, Some(CHEAP), CAP), CAP);
        assert_eq!(pick_model("   ", false, false, Some(CHEAP), CAP), CAP);
        assert!(!is_chat_only(""));
    }

    #[test]
    fn 中文按字符算长度而不是字节() {
        // 60 个汉字 = 60 字符（不是 180 字节）。若按字节算会误判成长消息。
        let s = "啊".repeat(40);
        assert_eq!(s.chars().count(), 40);
        assert!(is_chat_only(&s), "40 个汉字应当算短消息");
    }
}
