// 《铃·记忆体》陪伴记录模块（P3：与铃的日记）
// 记录首次见面日期与里程碑事件，纯本地存储（~/.铃记忆体/milestones.json）
// 非游戏化：只做温柔记录，不搞等级/徽章
use crate::config;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 里程碑条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneItem {
    pub key: String,
    pub label: String,
    pub date: String, // YYYY-MM-DD
}

/// 每日日记条目（P3 改造：日记=持续记录，不只是成就）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyEntry {
    pub date: String,          // YYYY-MM-DD
    pub chat_count: u32,       // 当天聊天句数
    pub tool_count: u32,       // 当天工具箱使用次数
    pub topics: Vec<String>,   // 当天话题（去重）
    pub last_text: Option<String>, // 当天最后一句（作为"今日一言"候选）
}

/// 陪伴记录数据
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MilestoneData {
    /// 首次见面日期（YYYY-MM-DD）
    pub first_date: Option<String>,
    /// 已完成的里程碑（key -> item）
    #[serde(default)]
    pub items: HashMap<String, MilestoneItem>,
    /// 每日日记（date -> entry）
    #[serde(default)]
    pub daily: HashMap<String, DailyEntry>,
}

/// 里程碑文件路径
fn milestones_path() -> std::path::PathBuf {
    config::data_dir().join("milestones.json")
}

/// 读取陪伴记录（不存在则返回空）
fn load() -> MilestoneData {
    let path = milestones_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => MilestoneData::default(),
    }
}

/// 写入陪伴记录
fn save(data: &MilestoneData) -> Result<(), AppError> {
    let _ = config::ensure_data_dir().map_err(|e| AppError::ConfigError(e.to_string()))?;
    let path = milestones_path();
    let s = serde_json::to_string_pretty(data).map_err(|e| AppError::InternalError(e.to_string()))?;
    std::fs::write(&path, s).map_err(|e| AppError::InternalError(e.to_string()))
}

/// 今天日期（YYYY-MM-DD）
fn today() -> String {
    let now = chrono::Local::now();
    now.format("%Y-%m-%d").to_string()
}

/// 记录一个里程碑（首次见面日期在第一次调用时自动记下）
/// 幂等：同一 key 只记一次；已存在则忽略
#[tauri::command]
pub async fn record_milestone(key: String, label: String) -> Result<(), AppError> {
    let mut data = load();
    // 首次见面日期
    if data.first_date.is_none() {
        data.first_date = Some(today());
    }
    // 里程碑幂等
    if !data.items.contains_key(&key) {
        data.items.insert(
            key.clone(),
            MilestoneItem {
                key,
                label,
                date: today(),
            },
        );
    }
    save(&data)
}

/// 获取陪伴记录（含陪伴天数 + 每日累积数据）
#[tauri::command]
pub async fn get_milestones() -> Result<serde_json::Value, AppError> {
    let data = load();
    // 计算陪伴天数：first_date 到今天
    let days = match &data.first_date {
        Some(fd) => {
            let first = chrono::NaiveDate::parse_from_str(fd, "%Y-%m-%d").unwrap_or_else(|_| chrono::Local::now().date_naive());
            let now = chrono::Local::now().date_naive();
            let d = (now - first).num_days().max(0) + 1; // 当天算第 1 天
            d
        }
        None => 0,
    };
    // 按日期排序输出（最新在前）
    let mut items: Vec<&MilestoneItem> = data.items.values().collect();
    items.sort_by(|a, b| b.date.cmp(&a.date));
    let items_json: Vec<serde_json::Value> = items
        .iter()
        .map(|m| {
            serde_json::json!({
                "key": m.key,
                "label": m.label,
                "date": m.date,
            })
        })
        .collect();

    // 每日累积数据（倒序：最新在前）
    let mut daily: Vec<&DailyEntry> = data.daily.values().collect();
    daily.sort_by(|a, b| b.date.cmp(&a.date));
    let daily_json: Vec<serde_json::Value> = daily
        .iter()
        .map(|e| {
            serde_json::json!({
                "date": e.date,
                "chat_count": e.chat_count,
                "tool_count": e.tool_count,
                "topics": e.topics,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "first_date": data.first_date,
        "days": days,
        "items": items_json,
        "daily": daily_json,
    }))
}

// ==================== 每日日记（P3 改造） ====================

/// 话题关键词表（从聊天内容提取当天话题）
const TOPIC_KEYWORDS: &[(&str, &[&str])] = &[
    ("游戏", &["游戏", "打游戏", "开黑", "上分", "我的世界", "mc", "王者", "原神", "steam", "副本", "排位"]),
    ("吃饭", &["吃饭", "吃了吗", "饿了", "吃什么", "干饭", "火锅", "外卖", "早餐", "午餐", "晚餐", "好吃"]),
    ("工作", &["上班", "工作", "加班", "打工", "写代码", "开会", "项目", "老板", "同事", "摸鱼", "作业", "学习", "考试"]),
    ("动漫", &["动漫", "番", "追番", "新番", "漫画", "动画", "二次元", "看番"]),
    ("电影", &["电影", "电视剧", "追剧", "影院", "刷剧"]),
    ("音乐", &["音乐", "歌", "唱歌", "听歌", "乐队", "耳机"]),
    ("健康", &["生病", "感冒", "发烧", "头疼", "肚子疼", "身体", "吃药", "医院", "难受"]),
    ("运动", &["跑步", "健身", "运动", "锻炼", "打球", "篮球", "足球", "游泳"]),
    ("家人", &["妈妈", "爸爸", "家人", "爸妈", "弟弟", "妹妹", "哥哥", "姐姐", "爷爷", "奶奶"]),
    ("朋友", &["朋友", "同学", "同事聚会", "兄弟", "闺蜜"]),
];

/// 从一句聊天里提取话题（返回命中的话题名列表）
fn extract_topics(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (topic, kws) in TOPIC_KEYWORDS {
        if kws.iter().any(|k| text.contains(k)) {
            found.push((*topic).to_string());
        }
    }
    found
}

/// 记录一次聊天：当日 chat_count +1，合并话题（幂等：同日累加）
#[tauri::command]
pub async fn record_daily_chat(text: String) -> Result<(), AppError> {
    let mut data = load();
    if data.first_date.is_none() {
        data.first_date = Some(today());
    }
    let date = today();
    let entry = data.daily.entry(date.clone()).or_insert_with(|| DailyEntry {
        date: date.clone(),
        chat_count: 0,
        tool_count: 0,
        topics: Vec::new(),
        last_text: None,
    });
    entry.chat_count += 1;
    // 合并话题（去重）
    for t in extract_topics(&text) {
        if !entry.topics.contains(&t) {
            entry.topics.push(t);
        }
    }
    // 记录当天最后一句（限长，供日记正文引用）
    let trimmed: String = text.chars().take(60).collect();
    entry.last_text = Some(trimmed);
    save(&data)
}

/// 记录一次工具箱使用：当日 tool_count +1
#[tauri::command]
pub async fn record_daily_tool(tool_name: String) -> Result<(), AppError> {
    let mut data = load();
    if data.first_date.is_none() {
        data.first_date = Some(today());
    }
    let date = today();
    let entry = data.daily.entry(date.clone()).or_insert_with(|| DailyEntry {
        date: date.clone(),
        chat_count: 0,
        tool_count: 0,
        topics: Vec::new(),
        last_text: None,
    });
    entry.tool_count += 1;
    // 工具名作为话题补充（如"清理内存"）
    let short: String = tool_name.chars().take(12).collect();
    if !entry.topics.contains(&short) && entry.topics.len() < 5 {
        entry.topics.push(short);
    }
    save(&data)
}
