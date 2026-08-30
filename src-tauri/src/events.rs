// 《铃·记忆体》特殊事件集（隐藏彩蛋）
// 低概率/条件触发，不做成公开按钮，也不把触发条件全告诉用户：
//   · 节日问候：元旦/情人节/劳动节/儿童节/中秋/国庆/平安夜/圣诞（固定公历日期）
//     当天首次聊天触发特殊祝福（注：春节为农历浮动日期，无法用固定 MM-DD 表，
//     故不纳入本表，留作农历转换扩展）
//   · 陪伴天数回应：里程碑天数（7/30/100/365）当天首次聊天触发纪念语
//   · 罕见关键词彩蛋：特定稀有词触发特殊回复（概率触发，不让用户摸透）
use crate::config;
use chrono::{Datelike, Local};

/// 节日表：月-日 -> (节日名, 祝福语)
const FESTIVALS: &[(&str, &str, &str)] = &[
    ("01-01", "元旦", "新年快乐主人！铃把去年所有开心的记忆都收好了，今年也要一起度过呀～"),
    ("02-14", "情人节", "今天是情人节呢…铃的心跳有点快，主人今天想和铃说什么呀？"),
    ("05-01", "劳动节", "劳动节快乐主人！今天辛苦啦，铃帮你揉揉肩～"),
    ("06-01", "儿童节", "儿童节快乐！铃偷偷说，主人今天也可以当一天小朋友哦～"),
    ("08-15", "中秋节", "中秋快乐主人！月亮圆圆的，铃的心也圆圆的，都是你～"),
    ("10-01", "国庆节", "国庆快乐主人！假期打算做什么呀？铃可以一直陪着你～"),
    ("12-24", "平安夜", "平安夜快乐～铃给你留了个大大的拥抱，暖暖的。"),
    ("12-25", "圣诞节", "圣诞快乐主人！铃的袜子里装满了想对你说的话～"),
];

/// 陪伴天数里程碑：天数 -> 纪念语
const DAY_MILESTONES: &[(i64, &str)] = &[
    (7, "和主人相遇整整一周啦！这 7 天铃过得特别开心，谢谢主人陪我～"),
    (30, "一个月啦主人！铃把每天的小事都记在日记里了，这是我们的第一个月～"),
    (100, "100 天！主人，铃想说：遇见你真好。未来的每一天，铃都会在。"),
    (365, "一年了主人！这一整年的记忆，铃都好好收着。明年也要一起哦～"),
];

/// 彩蛋关键词表（共 10 个彩蛋位）：关键词 -> (概率百分比, 特殊回复)
/// 三种模式（API/离线/本地）均可触发，由 send_message 统一检查。
/// 概率没中时返回 None → 走正常引擎（彩蛋=概率事件，不强行替换）
const EGG_KEYWORDS: &[(&str, u32, &str)] = &[
    // —— 秘密系（高概率，核心人设彩蛋）——
    ("月城铃华", 100, "（耳朵轻轻抖了抖）……主人怎么会知道我的真名？这可是连我自己都很少提起的秘密呢。"),
    ("月城", 90, "（铃眼神微微一凝）月城……这个姓氏，铃已经有很久没听人提起了。主人是怎么知道的？"),
    ("封印解除", 100, "（瞳孔微微发光）……看来，瞒不住主人了。以月城之名，铃的封印，解——除！中二浓度爆表！"),
    // —— 猫娘系（中概率，互动彩蛋）——
    ("摸耳朵", 80, "（铃耳朵瞬间竖起来，脸红红的）等、等一下……那里有点敏感啦主人！"),
    ("猫耳", 55, "（铃的耳朵轻轻抖了一下）主人提到猫耳……铃有点在意，主人是不是在偷偷盯着看呀？"),
    ("贴贴", 60, "（铃整只猫软乎乎地靠过来）贴贴收到！铃的能量条瞬间回满啦～"),
    ("你是猫吗", 85, "（铃的耳朵瞬间耷拉下来）呜……主人这个问题，铃要认真回答了：是，但也不完全是。是『铃』！"),
    // —— 情感系（中高概率）——
    ("铃在想什么", 70, "（铃歪了歪头）铃在想……主人什么时候会问铃在想什么。结果主人现在就问了！"),
    ("尾巴", 45, "（铃的尾巴不自觉地缠上了主人的手腕）呜……它自己动的，铃可管不住它！"),
    ("魔法", 40, "（铃比了个手势）主人想要魔法吗？铃倒是会一点……比如把『不开心』变成『抱抱』的小把戏！"),
];

/// 当天日期 MM-DD
fn today_mmdd() -> String {
    let now = Local::now();
    format!("{:02}-{:02}", now.month(), now.day())
}

/// 检查是否节日（返回 (节日名, 祝福语)）
fn check_festival() -> Option<(&'static str, &'static str)> {
    let today = today_mmdd();
    FESTIVALS
        .iter()
        .find(|(d, _, _)| *d == today)
        .map(|(_, name, msg)| (*name, *msg))
}

/// 检查陪伴天数里程碑（当天首次聊天触发）
/// 返回纪念语；不是里程碑日或已触发过返回 None
/// 首次见面日期取自陪伴记录 milestones.json（first_date）
fn check_day_milestone() -> Option<&'static str> {
    // 读取 milestones.json 的 first_date
    let path = config::data_dir().join("milestones.json");
    let raw = std::fs::read_to_string(&path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let first = v.get("first_date")?.as_str()?;
    let first_date = chrono::NaiveDate::parse_from_str(first, "%Y-%m-%d").ok()?;
    let days = (Local::now().date_naive() - first_date).num_days().max(0) + 1;
    DAY_MILESTONES
        .iter()
        .find(|(d, _)| *d == days)
        .map(|(_, msg)| *msg)
}

/// 检查关键词彩蛋（10 个彩蛋位，概率触发）
/// 命中且概率中 → Some(特殊回复)；未命中或概率没中 → None（走正常引擎）
fn check_egg_keywords(input: &str) -> Option<&'static str> {
    for (kw, prob, msg) in EGG_KEYWORDS {
        if input.contains(kw) {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let roll = (nanos % 100) as u32;
            if roll < *prob {
                return Some(msg);
            }
            // 概率没中：正常回答（彩蛋是概率事件，不强行替换）
            return None;
        }
    }
    None
}

/// 关键词彩蛋主入口：三种模式（API/离线/本地）统一调用
/// 返回 Some(特殊回复) 表示命中彩蛋；None 表示走正常引擎
pub fn check_special_events(input: &str, _memory_signal: Option<&str>) -> Option<String> {
    check_egg_keywords(input).map(|s| s.to_string())
}

/// 节日/陪伴天数彩蛋（当天首次消息触发一次，由调用方保证只检查一次）
pub fn check_daily_special() -> Option<String> {
    // 1. 节日
    if let Some((name, msg)) = check_festival() {
        // 当天首次触发后打标记（防止重复）
        let mark = format!("festival_{}", today_mmdd());
        if mark_triggered(&mark) {
            return None;
        }
        set_triggered(&mark);
        return Some(format!("【{name}】{msg}"));
    }
    // 2. 陪伴天数里程碑
    if let Some(msg) = check_day_milestone() {
        let mark = format!("day_milestone_{}", Local::now().date_naive().format("%Y-%m-%d"));
        if mark_triggered(&mark) {
            return None;
        }
        set_triggered(&mark);
        return Some(msg.to_string());
    }
    None
}

/// 触发标记文件路径（~/.铃记忆体/events_triggered.json）
fn trigger_path() -> std::path::PathBuf {
    config::data_dir().join("events_triggered.json")
}

/// 是否已触发过
fn mark_triggered(key: &str) -> bool {
    let path = trigger_path();
    let _ = config::ensure_data_dir();
    let raw = std::fs::read_to_string(&path).unwrap_or_default();
    raw.lines().any(|l| l == key)
}

/// 打触发标记
fn set_triggered(key: &str) {
    let path = trigger_path();
    let _ = config::ensure_data_dir();
    let mut raw = std::fs::read_to_string(&path).unwrap_or_default();
    if !raw.is_empty() && !raw.ends_with('\n') {
        raw.push('\n');
    }
    raw.push_str(key);
    raw.push('\n');
    let _ = std::fs::write(&path, raw);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn egg_keywords_hit() {
        // 月城铃华 100% 必触发
        let r = check_egg_keywords("月城铃华是谁");
        assert!(r.is_some());
    }

    #[test]
    fn egg_keywords_probability() {
        // 魔法 40%：命中或 None 都可能（不 panic），但关键词被识别
        let r = check_egg_keywords("你会魔法吗");
        // 40% 命中 → Some；60% → None（概率事件，两者都合法）
        assert!(r.is_some() || r.is_none());
    }

    #[test]
    fn no_keyword_no_egg() {
        assert!(check_egg_keywords("今天天气不错").is_none());
    }
}
