// 《铃·记忆体》日志读模块（AI-7 任务 7）
use crate::types::LogLevel;
use std::path::PathBuf;

/// 读取日志，支持级别过滤 + 关键词搜索 + offset/limit 分页。
/// 返回 (当前页行列表, 总匹配行数)。
pub fn read_logs(
    path: &PathBuf,
    level: Option<LogLevel>,
    keyword: Option<&str>,
    offset: usize,
    limit: usize,
) -> (Vec<String>, usize) {
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return (Vec::new(), 0),
    };

    let filtered: Vec<&str> = content
        .lines()
        .filter(|line| {
            // 级别过滤：日志行格式 [时间] [LEVEL] 消息
            if let Some(min_level) = level {
                let passes = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"]
                    .iter()
                    .enumerate()
                    .any(|(i, lvl)| {
                        // 行中含该级别字符串，且该级别 >= 最低级别
                        line.contains(&format!("[{}]", lvl))
                            && (i as u8) >= min_level.order()
                    });
                if !passes {
                    return false;
                }
            }
            // 关键词过滤
            if let Some(kw) = keyword {
                if !kw.is_empty() && !line.contains(kw) {
                    return false;
                }
            }
            true
        })
        .collect();

    let total = filtered.len();
    let page: Vec<String> = filtered
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|s| s.to_string())
        .collect();

    (page, total)
}
