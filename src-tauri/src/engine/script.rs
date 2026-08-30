// 《铃·记忆体》脚本模式引擎
// 从 resources/reply_library.json 加载预设回复，按关键词匹配分类，
// 无匹配时从「日常」随机返回，回复拆成 3~5 字片段以 50ms 间隔流式推送。
use crate::error::AppError;
use crate::stream::sender;
use crate::types::Memory;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use tauri::AppHandle;

/// 回复库结构：分类名 -> 回复列表
type ReplyLibrary = HashMap<String, Vec<String>>;

// 全局只读的回复库（OnceLock 保证只加载一次）
static REPLY_LIBRARY: OnceLock<ReplyLibrary> = OnceLock::new();

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

/// 关键词 -> 分类 映射（打分制：命中多个关键词累计权重，取最高分分类）
/// 相比旧版顺序判断，改进：
///   · 打分制：明确词（长词/专用词）权重高，短词（累/哭/懒）权重低防误伤
///   · 否定检测：「不累/没哭/不难过」自动抵消对应关键词，避免「我不难过」被当难过
///   · 记忆兜底：classify_with_memory 无匹配时扫描最近记忆，回答有连续性
/// depth 影响深度分类（哲理/古风/冷笑话）的启用与兜底概率：
///   depth 1-2：只走常规 7 类（与旧版行为一致）
///   depth 3+ ：深度分类关键词生效；无匹配时按概率抽深度分类彩蛋
fn classify(input: &str, depth: u8) -> &'static str {
    classify_scored(input, depth, None)
}

/// 打分版 classify：可带最近记忆做兜底（记忆里常聊的话题也算分类依据）
fn classify_scored(input: &str, depth: u8, memories: Option<&[crate::types::Memory]>) -> &'static str {
    // 规则表：分类 -> (关键词, 权重)。权重 3=明确词，2=常用词，1=易误伤短词
    const RULES: &[(&str, &[(&str, u8)])] = &[
        ("问候", &[("早上好", 3), ("早安", 3), ("中午好", 3), ("下午好", 3), ("晚上好", 3), ("你好", 2), ("嗨", 2), ("hello", 3), ("hi", 2), ("早呀", 3), ("晚好", 3)]),
        ("日语", &[("日语", 3), ("日文", 3), ("日本语", 3), ("日本語", 3), ("霓虹语", 3), ("japanese", 3), ("nihongo", 3)]),
        ("英文", &[("英文", 3), ("英语", 3), ("english", 3), ("英语说", 3), ("翻译成英文", 3)]),
        ("颜表情", &[("颜文字", 3), ("颜表情", 3), ("kaomoji", 3), ("表情", 2), ("卖萌表情", 3)]),
        ("撒娇", &[("撒娇", 3), ("抱抱", 3), ("贴贴", 3), ("亲亲", 3), ("摸摸头", 3), ("求抱", 3), ("蹭蹭", 3), ("亲一个", 3), ("抱一下", 3), ("陪陪", 2), ("理理", 2), ("小鱼干", 2)]),
        ("夸奖", &[("真棒", 3), ("好厉害", 3), ("好棒", 3), ("夸夸", 3), ("厉害", 2), ("真可爱", 3), ("好聪明", 3), ("做得好", 3), ("好帅", 2), ("好美", 2), ("优秀", 2)]),
        ("想念", &[("想你", 3), ("想你了", 3), ("想铃", 3), ("在吗", 2), ("在不在", 2), ("好久不见", 3), ("想死你", 3)]),
        ("吃饭", &[("吃饭", 3), ("吃了吗", 3), ("饿了", 3), ("吃什么", 3), ("干饭", 3), ("肚子饿", 3), ("早餐", 2), ("午餐", 2), ("晚餐", 2), ("点外卖", 2)]),
        ("睡觉", &[("睡觉", 3), ("晚安", 3), ("困了", 3), ("睡觉觉", 3), ("想睡", 3), ("睡了", 2), ("入睡", 2), ("睡觉啦", 3), ("困", 1)]),
        ("工作", &[("上班", 3), ("工作", 2), ("加班", 3), ("打工", 3), ("写代码", 3), ("作业", 2), ("学习", 2), ("开会", 3), ("项目", 2), ("老板", 2), ("同事", 2)]),
        ("游戏", &[("游戏", 3), ("打游戏", 3), ("开黑", 3), ("上分", 3), ("我的世界", 3), ("mc", 2), ("minecraft", 3), ("王者", 3), ("原神", 3), ("steam", 3), ("对局", 2), ("团战", 3), ("输了", 2), ("赢了", 2)]),
        ("动漫", &[("动漫", 3), ("追番", 3), ("新番", 3), ("番剧", 3), ("漫画", 3), ("动画", 3), ("二次元", 3), ("看番", 3), ("番吗", 3), ("番", 1)]),
        ("安慰", &[("难受", 3), ("难过", 3), ("伤心", 3), ("委屈", 3), ("不开心", 3), ("低落", 3), ("沮丧", 3), ("失恋", 3), ("焦虑", 3), ("疲惫", 3), ("压力", 2), ("累", 1), ("哭", 1), ("痛", 1), ("烦", 1), ("害怕", 2), ("困", 1), ("失败", 2)]),
        ("道歉", &[("对不起", 3), ("抱歉", 3), ("我错了", 3), ("原谅", 3), ("道歉", 3), ("认错", 3), ("赔罪", 3), ("不好意思", 2)]),
        ("情话", &[("爱你", 3), ("喜欢你", 3), ("我爱你", 3), ("最喜欢你", 3), ("想你", 3), ("情话", 3), ("告白", 3), ("在一起", 3)]),
        ("吐槽", &[("偷懒", 3), ("熬夜", 3), ("晚睡", 3), ("没吃饭", 3), ("摸鱼", 3), ("零食当饭吃", 3), ("懒", 1), ("早睡", 2), ("喝水", 2), ("作业", 2), ("忘", 1)]),
    ];
    // 深度分类（depth ≥ 3 才计分）
    const DEEP_RULES: &[(&str, &[(&str, u8)])] = &[
        ("哲理", &[("哲理", 3), ("哲学", 3), ("人生", 2), ("意义", 2), ("道理", 2), ("智慧", 2)]),
        ("古风", &[("古风", 3), ("诗句", 3), ("诗词", 3), ("文言", 3), ("古诗词", 3), ("古文", 3)]),
        ("冷笑话", &[("冷笑话", 3), ("笑话", 3), ("段子", 3), ("冷知识", 3), ("幽默", 3)]),
    ];

    // 对输入打分
    let mut best: Option<(&'static str, u32)> = None;
    consider_rules(input, RULES, &mut best);
    if depth >= 3 {
        consider_rules(input, DEEP_RULES, &mut best);
    }

    // 常规分类优先于深度分类（同分时常规获胜）：上面先扫常规后扫深度，且用严格大于，天然满足
    if let Some((cat, _)) = best {
        return cat;
    }

    // —— 记忆兜底：输入无关键词命中时，扫描最近记忆（主人常聊话题也算数）——
    if let Some(ms) = memories {
        // 只取最近的用户消息做话题统计（最多 10 条）
        let recent_user: String = ms
            .iter()
            .rev()
            .filter(|m| m.role == "user")
            .take(10)
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        if !recent_user.trim().is_empty() {
            consider_rules(&recent_user, RULES, &mut best);
            if depth >= 3 {
                consider_rules(&recent_user, DEEP_RULES, &mut best);
            }
            if let Some((cat, _)) = best {
                return cat;
            }
        }
    }

    // —— 深度彩蛋：无任何命中时，depth≥3 按概率抽深度分类 ——
    if depth >= 3 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        // 用输入长度做额外扰动，避免同一时刻总是同分类
        let mix = (nanos as usize) ^ (input.len() * 7919);
        let roll = (mix % 100) as u8;
        let chance = match depth {
            3 => 25u8,
            4.. => 45u8,
            _ => 0,
        };
        if roll < chance {
            match (mix / 7) % 3 {
                0 => "哲理",
                1 => "古风",
                _ => "冷笑话",
            }
        } else {
            "日常"
        }
    } else {
        "日常"
    }
}

/// 否定检测：输入含「不X / 没X / 别X」时视为否定，关键词不计分
/// （如「我不累」「没哭」「别难过」不会触发安慰分类）
fn has_negation(input: &str, kw: &str) -> bool {
    // 双字以上关键词才做否定检测（单字「累/哭」在「不累/没哭」中紧邻）
    let negs = ["不", "没", "别"];
    negs.iter().any(|n| {
        let neg_kw = format!("{n}{kw}");
        input.contains(&neg_kw)
    })
}

/// 对一段文本按规则表打分，累计各分类命中权重，取最高分写入 best
/// 规则表是静态常量（'static），分类名可安全返回
fn consider_rules(
    text: &str,
    rules: &[(&'static str, &'static [(&'static str, u8)])],
    best: &mut Option<(&'static str, u32)>,
) {
    for (cat, kws) in rules {
        let mut score: u32 = 0;
        for (kw, w) in *kws {
            if text.contains(kw) && !has_negation(text, kw) {
                score += *w as u32;
            }
        }
        if score > 0 && (best.is_none() || score > best.unwrap().1) {
            *best = Some((cat, score));
        }
    }
}

// 最近使用的回复历史（避免短时间内重复，环形缓冲）
static RECENT_REPLIES: Mutex<Vec<String>> = Mutex::new(Vec::new());
const RECENT_LIMIT: usize = 8;
// 共享池参与概率（30%：让不同分类偶尔共用自然回应，模拟真实对话）
const SHARED_POOL_RATE: u32 = 30;

/// 从分类中取一条回复（若分类为空则回退日常）
/// 升级（Phase1）：
///   · 最近 RECENT_LIMIT 条不重复（历史缓冲，而非仅上一次）
///   · 30% 概率从 _shared 共享池取（万能回应，多分类可共用）
///   · 分类内随机 + 历史规避
fn pick_reply(category: &str) -> String {
    let lib = load_library();

    // 1) 随机种子
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    // 2) 共享池概率（30%）+ 分类兜底：分类为空时也走共享池
    let shared = lib.get("_shared");
    let mut use_shared = ((nanos % 100) as u32) < SHARED_POOL_RATE && category != "日常";
    if shared.map(|s| s.is_empty()).unwrap_or(true) {
        use_shared = false;
    }

    let mut candidates: Vec<String> = Vec::new();
    if use_shared {
        if let Some(s) = shared {
            candidates.extend(s.iter().cloned());
        }
    } else if let Some(list) = lib.get(category).filter(|l| !l.is_empty()) {
        candidates.extend(list.iter().cloned());
    } else if let Some(list) = lib.get("日常").filter(|l| !l.is_empty()) {
        candidates.extend(list.iter().cloned());
    } else if let Some(list) = lib.values().next().filter(|l| !l.is_empty()) {
        candidates.extend(list.iter().cloned());
    }

    if candidates.is_empty() {
        return "铃在这里呢～".to_string();
    }

    // 3) 取候选：优先避开最近使用过的（历史规避）
    // 注意：std Mutex 不可重入，必须 drop 锁后再调用 record_reply（内部会重新 lock）
    let recent = RECENT_REPLIES.lock().unwrap();
    // 从随机起点开始线性探测，最多尝试整个列表
    let start = (nanos % candidates.len() as u128) as usize;
    for offset in 0..candidates.len() {
        let idx = (start + offset) % candidates.len();
        let text = &candidates[idx];
        if !recent.iter().any(|r| r == text) {
            let chosen = text.clone();
            drop(recent);
            return record_reply(chosen);
        }
    }
    // 全部都在历史里（列表极小）→ 退而取起点
    let fallback = candidates[start].clone();
    drop(recent);
    record_reply(fallback)
}

/// 记录一条已使用回复（维护最近历史缓冲）
fn record_reply(text: String) -> String {
    let mut recent = RECENT_REPLIES.lock().unwrap();
    recent.push(text.clone());
    if recent.len() > RECENT_LIMIT {
        let overflow = recent.len() - RECENT_LIMIT;
        recent.drain(..overflow);
    }
    text
}

/// 运行脚本模式：返回完整回复文本（供上层写入记忆），内部完成流式推送
/// depth：思考深度 1-4，≥3 时解锁深度分类（哲理/古风/冷笑话）并提高彩蛋概率
/// memories：最近记忆（可空），用于无关键词命中时的记忆兜底分类
pub async fn run_script(
    app: &AppHandle,
    input: &str,
    setting: &crate::types::Setting,
    depth: u8,
    memories: &[crate::types::Memory],
) -> Result<String, AppError> {
    let category = classify_scored(input, depth, Some(memories));
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
        category: None,
        use_count: 0,
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
            // 扩充文库新增分类（moon12-3）
            ("早上好呀", "问候"),
            ("你好呀", "问候"),
            ("主人真棒", "夸奖"),
            ("我好想你", "想念"),
            ("你吃饭了吗", "吃饭"),
            ("我要睡觉了", "睡觉"),
            ("今天上班好累", "工作"),
            ("一起打游戏吗", "游戏"),
            ("推荐个动漫", "动漫"),
            ("对不起我错了", "道歉"),
            ("我喜欢你", "情话"),
            ("晚安", "睡觉"),
            ("在吗", "想念"),
            // 否定检测：不触发安慰
            ("我不难过", "日常"),
            ("我没哭", "日常"),
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
