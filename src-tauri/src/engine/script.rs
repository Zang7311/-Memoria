// 《铃·记忆体》脚本模式引擎
// 从 resources/reply_library.json 加载预设回复，按关键词匹配分类，
// 无匹配时从「日常」随机返回，回复拆成 3~5 字片段以 50ms 间隔流式推送。
use crate::error::AppError;
use crate::stream::sender;
use crate::types::Memory;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use tauri::AppHandle;

/// 回复库结构：分类名 -> 回复列表
type ReplyLibrary = HashMap<String, Vec<String>>;

// 全局只读的回复库（OnceLock 保证只加载一次）
static REPLY_LIBRARY: OnceLock<ReplyLibrary> = OnceLock::new();

// 上一次选中的回复索引（避免短时间内重复输出同一条）
static LAST_INDEX: AtomicUsize = AtomicUsize::new(usize::MAX);

/// 加载回复库（内嵌资源，编译时打进二进制，避免运行时路径问题）
fn load_library() -> &'static ReplyLibrary {
    REPLY_LIBRARY.get_or_init(|| {
        let raw = include_str!("../../resources/reply_library.json");
        serde_json::from_str::<ReplyLibrary>(raw)
            .unwrap_or_else(|e| {
                log::error!("回复库解析失败：{e}");
                let mut map = HashMap::new();
                map.insert(
                    "日常".to_string(),
                    vec!["主人～铃在这里呢。".to_string()],
                );
                map
            })
    })
}

/// 关键词 -> 分类 映射（优先匹配更具体的分类）
/// depth 影响深度分类（哲理/古风/冷笑话）的启用与兜底概率：
///   depth 1-2：只走常规 7 类（与旧版行为一致）
///   depth 3+ ：深度分类关键词生效；无匹配时按概率抽深度分类彩蛋
fn classify(input: &str, depth: u8) -> &'static str {
    // 语言类请求（优先级最高）
    const LANG_JP: &[&str] = &[
        "日语", "日文", "日本語", "霓虹语", "japanese", "nihongo", "日本语",
    ];
    const LANG_EN: &[&str] = &[
        "英文", "英语", "english", "ingli", "英语说",
    ];
    // 撒娇类
    const ACT_CUTE: &[&str] = &[
        "撒娇", "抱抱", "贴贴", "亲亲", "摸摸头", "求抱", "蹭蹭", "亲一个",
        "抱一下",
    ];
    // 颜表情类
    const ACT_FACE: &[&str] = &[
        "颜文字", "表情", "kaomoji", "颜表情", "卖萌表情",
    ];
    // 安慰类关键词
    const COMFORT: &[&str] = &[
        "累", "难受", "难过", "伤心", "哭", "痛", "烦", "压力", "疲惫", "委屈",
        "不开心", "低落", "沮丧", "失恋", "失败", "焦虑", "害怕", "困",
    ];
    // 吐槽触发词
    const IRONY: &[&str] = &[
        "偷懒", "懒", "熬夜", "晚睡", "没吃饭", "忘", "摸鱼", "零食当饭吃",
        "早睡", "喝水", "作业",
    ];
    // 深度分类关键词（depth ≥ 3 才启用）
    const DEEP_PHILO: &[&str] = &["哲理", "人生", "意义", "哲学", "道理", "智慧"];
    const DEEP_ANCIENT: &[&str] = &["古风", "诗句", "诗词", "文言", "古诗词", "古文"];
    const DEEP_JOKE: &[&str] = &["冷笑话", "笑话", "段子", "冷知识", "幽默"];

    if LANG_JP.iter().any(|k| input.contains(k)) {
        "日语"
    } else if LANG_EN.iter().any(|k| input.contains(k)) {
        "英文"
    } else if ACT_FACE.iter().any(|k| input.contains(k)) {
        "颜表情"
    } else if ACT_CUTE.iter().any(|k| input.contains(k)) {
        "撒娇"
    } else if COMFORT.iter().any(|k| input.contains(k)) {
        "安慰"
    } else if IRONY.iter().any(|k| input.contains(k)) {
        "吐槽"
    } else if depth >= 3 {
        // 深度分类：仅思考深度 ≥ 3 时可命中
        if DEEP_PHILO.iter().any(|k| input.contains(k)) {
            "哲理"
        } else if DEEP_ANCIENT.iter().any(|k| input.contains(k)) {
            "古风"
        } else if DEEP_JOKE.iter().any(|k| input.contains(k)) {
            "冷笑话"
        } else {
            // 无关键词匹配时，按深度概率抽深度分类彩蛋（depth 越高概率越大）
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let roll = (nanos % 100) as u8;
            let chance = match depth {
                3 => 25u8,
                4.. => 45u8,
                _ => 0,
            };
            if roll < chance {
                match (nanos / 7) % 3 {
                    0 => "哲理",
                    1 => "古风",
                    _ => "冷笑话",
                }
            } else {
                "日常"
            }
        }
    } else {
        "日常"
    }
}

/// 从分类中取一条回复（若分类为空则回退日常），保证不与上一次相同
fn pick_reply(category: &str) -> String {
    let lib = load_library();
    let cat = lib
        .get(category)
        .or_else(|| lib.get("日常"))
        .or_else(|| lib.values().next());

    match cat {
        Some(list) if !list.is_empty() => {
            let len = list.len();
            // 用纳秒时间戳做伪随机种子
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let mut idx = (nanos % len as u128) as usize;

            // 若与上一次相同则顺移一位（保证连续两条不重复）
            let last = LAST_INDEX.load(Ordering::Relaxed);
            if len > 1 && last != usize::MAX && idx == last {
                idx = (idx + 1) % len;
            }
            LAST_INDEX.store(idx, Ordering::Relaxed);

            list[idx].clone()
        }
        _ => "铃在这里呢～".to_string(),
    }
}

/// 运行脚本模式：返回完整回复文本（供上层写入记忆），内部完成流式推送
/// depth：思考深度 1-4，≥3 时解锁深度分类（哲理/古风/冷笑话）并提高彩蛋概率
pub async fn run_script(
    app: &AppHandle,
    input: &str,
    setting: &crate::types::Setting,
    depth: u8,
) -> Result<String, AppError> {
    let category = classify(input, depth);
    let raw = pick_reply(category);
    // 名称占位替换：自定义名称功能（默认「铃」/「主人」，未配置时保持原文）
    let reply = apply_names(&raw, setting);
    log::info!(
        "[script] 输入「{input}」→ 分类「{category}」（depth={depth}）"
    );

    // 拆分为 3~5 字片段，50ms 间隔推送
    let chars: Vec<char> = reply.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let size = 3 + ((i as u64) % 3) as usize; // 3/4/5 字循环
        let end = (i + size).min(chars.len());
        let chunk: String = chars[i..end].iter().collect();
        sender::send_chunk(app, &chunk)?;
        i = end;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    sender::send_end(app)?;
    Ok(reply)
}

/// 名称占位替换：把回复模板中的「铃」替换为 AI 自定义自称，「主人」替换为用户自定义称呼。
/// 未配置（None）时保持默认「铃」「主人」不变。先替换自称再替换称呼，
/// 避免自定义称呼中出现「铃」「主人」字样时被二次替换（配置端应避免这种字面冲突）。
pub fn apply_names(text: &str, setting: &crate::types::Setting) -> String {
    let self_name = setting.self_name.as_deref().unwrap_or("铃");
    let user_name = setting.user_name.as_deref().unwrap_or("主人");
    if self_name.is_empty() && user_name.is_empty() {
        return text.to_string();
    }
    let mut out = text.replace("铃", self_name);
    if !user_name.is_empty() && user_name != "主人" {
        out = out.replace("主人", user_name);
    }
    out
}

/// 生成一条可写入记忆的 assistant 记忆
pub fn to_memory(id: &str, reply: &str) -> Memory {
    Memory {
        id: id.to_string(),
        role: "assistant".to_string(),
        content: reply.to_string(),
        timestamp: crate::utils::now_str(),
        tags: None,
        summary: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 核心回归：连续调用 pick_reply 不得相邻重复（LAST_INDEX 顺移逻辑）
    #[test]
    fn no_consecutive_duplicate() {
        let mut last: Option<String> = None;
        for _ in 0..300 {
            let r = pick_reply("日常");
            if let Some(prev) = &last {
                assert_ne!(prev, &r, "连续两次返回了相同回复：{r}");
            }
            last = Some(r);
        }
    }

    /// 十个分类都必须非空（含深度分类：哲理/古风/冷笑话）
    #[test]
    fn all_categories_have_replies() {
        let lib = load_library();
        for cat in ["日常", "安慰", "吐槽", "日语", "撒娇", "英文", "颜表情", "哲理", "古风", "冷笑话"] {
            assert!(
                lib.get(cat).map(|v| !v.is_empty()).unwrap_or(false),
                "分类「{cat}」为空"
            );
        }
        // 总数 ≥ 任务书要求的 50 条
        let total: usize = lib.values().map(|v| v.len()).sum();
        assert!(total >= 50, "回复总数 {total} 不足 50");
    }

    /// 关键词分类抽查（常规分类与深度无关）
    #[test]
    fn classify_keywords() {
        let cases = [
            ("我好累", "安慰"),
            ("主人又熬夜", "吐槽"),
            ("说日语", "日语"),
            ("抱抱我", "撒娇"),
            ("来点英文", "英文"),
            ("发个颜文字", "颜表情"),
            ("今天天气不错", "日常"),
        ];
        // 默认深度 2：常规分类不受影响
        for (input, want) in cases {
            assert_eq!(classify(input, 2), want, "输入「{input}」分类错误");
        }
    }

    /// 深度分类：depth 1-2 不命中，depth 3+ 命中哲理/古风/冷笑话
    #[test]
    fn classify_deep_categories_require_depth() {
        // depth=2：深度关键词不生效，回落日常
        assert_eq!(classify("讲个哲理", 2), "日常");
        assert_eq!(classify("来点古风", 2), "日常");
        assert_eq!(classify("讲个冷笑话", 2), "日常");
        // depth=3：深度关键词生效
        assert_eq!(classify("讲个哲理", 3), "哲理");
        assert_eq!(classify("来点古风", 3), "古风");
        assert_eq!(classify("讲个冷笑话", 3), "冷笑话");
        assert_eq!(classify("聊点人生", 4), "哲理");
        assert_eq!(classify("来段诗词", 4), "古风");
        assert_eq!(classify("来个段子", 4), "冷笑话");
        // 常规分类在深度模式下依然优先
        assert_eq!(classify("我好累啊", 4), "安慰");
        assert_eq!(classify("抱抱我", 4), "撒娇");
    }

    /// 自定义名称占位替换：默认不替换，配置后替换「铃」和「主人」
    #[test]
    fn apply_names_replace() {
        use crate::types::Setting;

        // 默认设置：保持原文
        let s = Setting::default();
        assert_eq!(apply_names("主人，铃在这里～", &s), "主人，铃在这里～");

        // 自定义 AI 自称 + 用户称呼
        let mut s2 = Setting::default();
        s2.self_name = Some("月城鈴華".to_string());
        s2.user_name = Some("阿伟".to_string());
        assert_eq!(
            apply_names("主人～铃陪着你！", &s2),
            "阿伟～月城鈴華陪着你！"
        );

        // 只改一个：自称保持默认
        let mut s3 = Setting::default();
        s3.user_name = Some("老板".to_string());
        assert_eq!(apply_names("主人，铃想你", &s3), "老板，铃想你");

        // 英文/颜表情分类里不含「主人」「铃」时原样通过
        assert_eq!(apply_names("I love you, master~", &s2), "I love you, master~");
    }
}
