// 《铃·记忆体》Agent 经验记忆（experience.rs）
//
// 职责：把「任务 → 成功用到的工具序列」记下来，下次遇到相似任务时先给规划器提个醒，
//       让它少走弯路（避免重演「打开 QQ 瞎搜 50 秒」那类试错）。
//
// 存储：%APPDATA%\ling-memoria\agent_experience.json
// 结构：[{ task, keywords, tools, count, last_used }]
//
// 匹配：关键词重叠打分（纯字符串，不依赖向量模型，零额外成本）
// 上限：最多 MAX_ENTRIES 条，超出按 last_used 淘汰最旧的
//
// 设计原则：永不 panic、永不阻塞主流程。读不到 / 写不进都只是「这次没经验可用」。
use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 最多保留的经验条数
const MAX_ENTRIES: usize = 200;

/// 关键词重叠率达到这个值才算「相似任务」
const SIMILARITY_THRESHOLD: f64 = 0.5;

/// 提示里最多列几条经验
const MAX_SUGGESTIONS: usize = 3;

/// 一条经验
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experience {
    /// 原始任务文本
    pub task: String,
    /// 从任务里抽出的关键词（用于相似度匹配）
    #[serde(default)]
    pub keywords: Vec<String>,
    /// 成功时依次用到的工具名
    #[serde(default)]
    pub tools: Vec<String>,
    /// 成功次数（同一套路重复命中会累加）
    #[serde(default = "one")]
    pub count: u32,
    /// 最后一次使用时间（Unix 秒，用于淘汰）
    #[serde(default)]
    pub last_used: u64,
}

fn one() -> u32 {
    1
}

/// 经验库文件路径
fn store_path() -> Option<PathBuf> {
    let base = std::env::var("APPDATA").ok()?;
    Some(
        PathBuf::from(base)
            .join("ling-memoria")
            .join("agent_experience.json"),
    )
}

/// 从指定文件读取（抽出来是为了让测试能针对临时文件跑完整往返）
fn load_from(p: &std::path::Path) -> Vec<Experience> {
    let Ok(raw) = std::fs::read_to_string(p) else {
        return Vec::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

/// 写到指定文件（自动建目录；失败静默：经验丢了不影响任务本身）
fn save_to(p: &std::path::Path, list: &[Experience]) {
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(list) {
        let _ = std::fs::write(p, text);
    }
}

/// 当前 Unix 秒
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 粗分词：抽 ASCII 单词（长度 >= 2）+ 中文 2-gram。
///
/// 不引入分词库 —— 任务文本通常很短，2-gram 足够做相似度匹配。
fn keywords(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    // 1. ASCII 单词 / 数字
    let mut ascii = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
            ascii.push(ch.to_ascii_lowercase());
        } else {
            if ascii.len() >= 2 {
                out.push(ascii.clone());
            }
            ascii.clear();
        }
    }
    if ascii.len() >= 2 {
        out.push(ascii);
    }

    // 2. 中文按 2-gram 切
    let cjk: Vec<char> = text
        .chars()
        .filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c))
        .collect();
    if cjk.len() == 1 {
        out.push(cjk[0].to_string());
    }
    for w in cjk.windows(2) {
        out.push(format!("{}{}", w[0], w[1]));
    }

    out.sort();
    out.dedup();
    out
}

/// 两个关键词集合的重叠率（交集 / 较小集合大小）
///
/// 用较小集合做分母：短任务匹配长经验时不会被稀释。
fn similarity(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let sa: HashSet<&String> = a.iter().collect();
    let sb: HashSet<&String> = b.iter().collect();
    let inter = sa.intersection(&sb).count();
    let denom = sa.len().min(sb.len()) as f64;
    inter as f64 / denom
}

/// 查相似任务的成功经验，拼成给规划器看的提示。
///
/// 返回 `None` 表示没有可用经验（首跑或都不像）。
pub fn suggest(task: &str) -> Option<String> {
    store_path().and_then(|p| suggest_at(&p, task))
}

/// 针对指定经验库文件查询（测试用同一套逻辑，保证测的就是线上跑的）
fn suggest_at(path: &std::path::Path, task: &str) -> Option<String> {
    let kw = keywords(task);
    if kw.is_empty() {
        return None;
    }

    let mut hits: Vec<(f64, Experience)> = load_from(path)
        .into_iter()
        .filter_map(|e| {
            let score = similarity(&kw, &e.keywords);
            if score >= SIMILARITY_THRESHOLD && !e.tools.is_empty() {
                Some((score, e))
            } else {
                None
            }
        })
        .collect();

    if hits.is_empty() {
        return None;
    }

    // 相似的排前面，分数相同则成功次数多的优先
    hits.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.1.count.cmp(&a.1.count))
    });

    let lines: Vec<String> = hits
        .iter()
        .take(MAX_SUGGESTIONS)
        .map(|(_, e)| {
            format!(
                "- 以前做过「{}」（成功 {} 次），当时依次用了：{}",
                e.task,
                e.count,
                e.tools.join(" → ")
            )
        })
        .collect();

    Some(format!(
        "以下是过去成功完成相似任务的做法，优先参考（但要用当前可用工具，工具名可能已变）：\n{}",
        lines.join("\n")
    ))
}

/// 任务成功后记一条经验。
///
/// - 关键词完全相同的旧记录会合并（count +1、工具序列以最新的为准）
/// - 超出上限则淘汰最久未使用的
pub fn record(task: &str, tools: &[String]) {
    if let Some(p) = store_path() {
        record_at(&p, task, tools);
    }
}

/// 针对指定经验库文件记录（测试走这条，逻辑与线上完全一致）
fn record_at(path: &std::path::Path, task: &str, tools: &[String]) {
    if task.trim().is_empty() || tools.is_empty() {
        return;
    }

    let kw = keywords(task);
    let mut list = load_from(path);
    let now = now_secs();

    if let Some(existing) = list.iter_mut().find(|e| e.keywords == kw) {
        existing.count = existing.count.saturating_add(1);
        existing.last_used = now;
        existing.tools = tools.to_vec();
        existing.task = task.to_string();
    } else {
        list.push(Experience {
            task: task.to_string(),
            keywords: kw,
            tools: tools.to_vec(),
            count: 1,
            last_used: now,
        });
    }

    // 淘汰：按 last_used 升序排，保留最新的 MAX_ENTRIES 条
    if list.len() > MAX_ENTRIES {
        list.sort_by_key(|e| e.last_used);
        let drop_n = list.len() - MAX_ENTRIES;
        list.drain(..drop_n);
    }

    save_to(path, &list);
}

/// 当前经验条数（诊断用）
pub fn count() -> usize {
    store_path().map(|p| load_from(&p).len()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 关键词_抽出ascii单词() {
        let k = keywords("open QQ.exe please");
        assert!(k.contains(&"open".to_string()));
        assert!(k.contains(&"qq.exe".to_string()));
        assert!(k.contains(&"please".to_string()));
    }

    #[test]
    fn 关键词_单字母被忽略() {
        let k = keywords("a b cd");
        assert!(!k.contains(&"a".to_string()));
        assert!(!k.contains(&"b".to_string()));
        assert!(k.contains(&"cd".to_string()));
    }

    #[test]
    fn 关键词_中文切二gram() {
        let k = keywords("打开QQ");
        assert!(k.contains(&"打开".to_string()), "实际: {k:?}");
        // 单个汉字也保留（"Q" 后跟 "Q" 是 ascii，不算中文）
        assert!(k.contains(&"开".to_string()) || k.contains(&"打开".to_string()));
    }

    #[test]
    fn 关键词_去重且有序() {
        let k = keywords("打开 打开 打开");
        let mut sorted = k.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(k, sorted);
    }

    #[test]
    fn 相似度_完全相同为1() {
        let a = keywords("打开QQ");
        assert!((similarity(&a, &a) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn 相似度_毫无交集为0() {
        let a = keywords("打开QQ");
        let b = keywords("清理磁盘空间");
        assert_eq!(similarity(&a, &b), 0.0);
    }

    #[test]
    fn 相似度_空集合为0() {
        let a: Vec<String> = Vec::new();
        let b = keywords("打开QQ");
        assert_eq!(similarity(&a, &b), 0.0);
    }

    #[test]
    fn 相似度_高重叠被识别() {
        let a = keywords("帮我打开QQ");
        let b = keywords("打开QQ");
        assert!(
            similarity(&a, &b) >= SIMILARITY_THRESHOLD,
            "相似度 = {}",
            similarity(&a, &b)
        );
    }

    #[test]
    fn 相似度_低重叠不被误判() {
        let a = keywords("打开QQ");
        let b = keywords("把屏幕亮度调到百分之五十");
        assert!(similarity(&a, &b) < SIMILARITY_THRESHOLD);
    }

    #[test]
    fn 记录_空任务或空工具直接忽略() {
        // 不 panic 即可（无副作用路径）
        record("", &["a".into()]);
        record("任务", &[]);
    }

    #[test]
    fn 序列化_字段缺省能兼容旧数据() {
        // 只有 task 的极简记录也要能解析出来
        let raw = r#"[{"task":"打开QQ"}]"#;
        let list: Vec<Experience> = serde_json::from_str(raw).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].count, 1);
        assert!(list[0].tools.is_empty());
        assert!(list[0].keywords.is_empty());
    }

    // —————— 下面是「落盘往返」测试：走真实文件，用临时路径，不碰用户真实经验库 ——————

    /// 造一个本次测试专用的临时文件路径（先清掉残留）
    fn temp_path(tag: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("ling_exp_test_{tag}_{}.json", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn 往返_记录后能被相似任务命中() {
        let path = temp_path("hit");

        record_at(&path, "帮我打开QQ", &["toolbox_agent_open_app".into()]);

        // 真的写到磁盘了
        assert!(path.is_file(), "经验文件应已落盘");
        assert_eq!(load_from(&path).len(), 1);

        // 相似任务应命中，并带回当时用的工具
        let hit = suggest_at(&path, "打开QQ").expect("相似任务应命中经验");
        assert!(
            hit.contains("toolbox_agent_open_app"),
            "提示里应包含工具名，实际：{hit}"
        );
        assert!(hit.contains("帮我打开QQ"), "提示里应包含原任务，实际：{hit}");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn 往返_不相似的任务不会命中() {
        let path = temp_path("miss");
        record_at(&path, "帮我打开QQ", &["toolbox_agent_open_app".into()]);

        assert!(
            suggest_at(&path, "把屏幕亮度调到五十").is_none(),
            "不相似的任务不该命中"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn 往返_同一任务重复记录只累加次数() {
        let path = temp_path("merge");
        record_at(&path, "打开QQ", &["toolbox_agent_open_app".into()]);
        record_at(&path, "打开QQ", &["toolbox_agent_open_app".into()]);
        record_at(&path, "打开QQ", &["toolbox_agent_open_app".into()]);

        let list = load_from(&path);
        assert_eq!(list.len(), 1, "同一任务应合并成一条，而不是三条");
        assert_eq!(list[0].count, 3);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn 往返_文件不存在时安全返回空() {
        let path = temp_path("absent");
        assert!(load_from(&path).is_empty());
        assert!(suggest_at(&path, "随便什么任务").is_none());
    }

    #[test]
    fn 往返_文件损坏时当空库不崩溃() {
        let path = temp_path("broken");
        std::fs::write(&path, "{ 这不是合法 JSON").unwrap();

        assert!(load_from(&path).is_empty(), "坏文件应被当成空库");

        // 坏文件之后还能正常写入
        record_at(&path, "打开QQ", &["toolbox_agent_open_app".into()]);
        assert_eq!(load_from(&path).len(), 1);

        let _ = std::fs::remove_file(&path);
    }
}
