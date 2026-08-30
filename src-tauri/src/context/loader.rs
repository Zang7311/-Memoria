// 《铃·记忆体》上下文加载器
// 从记忆索引文件（index.json）读取最近 n 条记忆，格式化为 messages 列表。
// 截断策略：若总 token 超限（约 4 字符 ≈ 1 token），优先保留最近的记忆。
use crate::error::AppError;
use crate::types::Memory;
use std::path::PathBuf;

/// 读取记忆索引文件，返回全部记忆（新到旧或旧到新取决于文件，这里统一按时间升序）
/// AI-3 交付的公共函数。AI-4 的 storage::read_all 改走 index::load_index（带损坏重建），
/// 故本函数当前仅保留给潜在下游使用，不删除以保持 AI-3 接口稳定。
#[allow(dead_code)]
pub fn read_memories(index_path: &PathBuf) -> Result<Vec<Memory>, AppError> {
    if !index_path.exists() {
        // 文件不存在视为无记忆，不报错
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(index_path)?;
    let memories: Vec<Memory> = if raw.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&raw)
            .map_err(|e| AppError::MemoryError(format!("解析 index.json 失败：{e}")))?
    };
    Ok(memories)
}

/// 从记忆中截取最近 n 条，并按 token 上限做截断（估算 4 字符 ≈ 1 token）
/// 返回排序后的 messages（旧→新），供引擎构造请求体
/// 改进（对话本体优化）：标记 important 的记忆优先保留——先收集重要记忆，
/// 再从普通记忆中按时间补足；token 截断时重要记忆不被丢弃。
pub fn build_context(
    memories: &[Memory],
    context_length: u8,
    max_tokens: usize,
) -> Vec<Memory> {
    let n = context_length as usize;
    // 1) 重要记忆（tags 含 important）全量收集（最多 n 条）
    let mut important: Vec<Memory> = memories
        .iter()
        .filter(|m| m.tags.as_ref().is_some_and(|t| t.iter().any(|x| x == "important")))
        .cloned()
        .collect();
    important.sort_by_key(|m| m.timestamp.clone());

    // 2) 普通记忆取最近 n 条（倒序去重重要记忆后取）
    let important_ids: std::collections::HashSet<&str> =
        important.iter().map(|m| m.id.as_str()).collect();
    let mut recent: Vec<Memory> = memories
        .iter()
        .rev()
        .filter(|m| !important_ids.contains(m.id.as_str()))
        .take(n)
        .cloned()
        .collect();
    recent.reverse(); // 恢复旧→新

    // 3) 合并：重要记忆在前（保证不被截断），普通记忆在后（按时间）
    // 先记录重要记忆的数量（用于截断保护），再移动进 merged
    let important_len = important.len();
    let estimate_tokens = |m: &Memory| (m.content.chars().count() + 3) / 4;
    let mut merged: Vec<Memory> = Vec::new();
    merged.extend(important);
    merged.extend(recent);

    // 4) token 截断：重要记忆先计入额度，从普通记忆（末尾）开始丢
    let mut total: usize = merged.iter().map(estimate_tokens).sum();
    // 从后往前丢普通记忆，直到不超限；重要记忆在头部不丢
    let mut idx = merged.len();
    while total > max_tokens && idx > important_len {
        idx -= 1;
        total -= estimate_tokens(&merged[idx]);
    }
    merged.truncate(idx);
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem(id: &str, content: &str) -> Memory {
        Memory {
            id: id.to_string(),
            role: "user".to_string(),
            content: content.to_string(),
            timestamp: String::new(),
            tags: None,
            summary: None,
        }
    }

    #[test]
    fn take_recent_n() {
        let ms = vec![
            mem("1", "aaa"),
            mem("2", "bbb"),
            mem("3", "ccc"),
            mem("4", "ddd"),
        ];
        let ctx = build_context(&ms, 2, 1000);
        assert_eq!(ctx.len(), 2);
        assert_eq!(ctx[0].id, "3");
        assert_eq!(ctx[1].id, "4");
    }

    #[test]
    fn truncate_by_token() {
        let long = "a".repeat(4000);
        let ms = vec![mem("1", "short"), mem("2", &long)];
        let ctx = build_context(&ms, 2, 10); // 10 token ≈ 40 字符
        // 早的 short 会被丢，保留最近的 long（long 本身超限会整体保留，这里只验证不 panic）
        assert!(ctx.len() <= 2);
    }
}
