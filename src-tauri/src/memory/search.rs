// 《铃·记忆体》记忆搜索（memory/search.rs）
// 关键词搜索：匹配 content 或 summary 包含关键词。
// 支持多关键词（空格分隔，AND 逻辑）。
use crate::types::Memory;

/// 判断单条记忆是否匹配关键词（content 或 summary 包含，忽略大小写）
fn matches(m: &Memory, kw: &str) -> bool {
    let kw = kw.to_lowercase();
    if kw.is_empty() {
        return true;
    }
    let content = m.content.to_lowercase();
    let summary = m.summary.clone().unwrap_or_default().to_lowercase();
    content.contains(&kw) || summary.contains(&kw)
}

/// 搜索记忆：多关键词（空格/逗号分隔，AND 逻辑）
/// 空关键词返回全部
pub fn search(memories: &[Memory], keyword: &str) -> Vec<Memory> {
    let kws: Vec<&str> = keyword
        .split(|c: char| c == ' ' || c == ',' || c == '，')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    if kws.is_empty() {
        return memories.to_vec();
    }

    memories
        .iter()
        .filter(|m| kws.iter().all(|kw| matches(m, kw)))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem(id: &str, content: &str, summary: Option<&str>) -> Memory {
        Memory {
            id: id.to_string(),
            role: "user".to_string(),
            content: content.to_string(),
            timestamp: String::new(),
            tags: None,
            summary: summary.map(|s| s.to_string()),
        }
    }

    #[test]
    fn empty_keyword_returns_all() {
        let ms = vec![mem("1", "你好", None), mem("2", "再见", None)];
        assert_eq!(search(&ms, "").len(), 2);
        assert_eq!(search(&ms, "   ").len(), 2);
    }

    #[test]
    fn single_keyword_matches_content_and_summary() {
        let ms = vec![
            mem("1", "今天天气很好", None),
            mem("2", "小猫很可爱", Some("关于猫的回忆")),
        ];
        let r = search(&ms, "天气");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "1");

        let r2 = search(&ms, "猫");
        // 只有记忆 2（content 与 summary 都含"猫"）匹配，记忆 1"今天天气很好"不含
        assert_eq!(r2.len(), 1);
        assert_eq!(r2[0].id, "2");
    }

    #[test]
    fn multi_keyword_and_logic() {
        let ms = vec![
            mem("1", "喜欢看动漫", None),
            mem("2", "喜欢看书", None),
            mem("3", "看书和动漫", None),
        ];
        let r = search(&ms, "喜欢 动漫");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "1");
    }

    #[test]
    fn case_insensitive() {
        let ms = vec![mem("1", "Hello World", None)];
        let r = search(&ms, "hello");
        assert_eq!(r.len(), 1);
    }
}
