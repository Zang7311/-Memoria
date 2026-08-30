// 《铃·记忆体》记忆自动压缩（memory/compress.rs）
// 当一个记忆集超过阈值（默认 20 条）时，将最早的一批合并为一条“摘要记忆”。
// 摘要生成：优先调用对话引擎摘要 API；不可用时用简单拼接。
// 压缩后删除被合并的原记忆，写入新的摘要记忆（需获取写锁）。
use crate::context::MEMORY_WRITER_LOCK;
use crate::error::AppError;
use crate::types::Memory;
use std::path::PathBuf;

/// 触发压缩的记忆条数阈值
pub const COMPRESS_THRESHOLD: usize = 20;
/// 每次合并最早多少条
pub const COMPRESS_BATCH: usize = 10;

/// 生成摘要内容（当前为简单拼接；后续可接入对话引擎 API 生成更智能摘要）
fn summarize(batch: &[Memory]) -> String {
    if batch.is_empty() {
        return String::new();
    }
    let mut parts: Vec<String> = Vec::new();
    for m in batch {
        let role = if m.role == "assistant" { "铃" } else { "同学" };
        let mut text = m.content.trim().to_string();
        if text.chars().count() > 40 {
            let cut: String = text.chars().take(40).collect();
            text = format!("{cut}…");
        }
        parts.push(format!("{role}：{text}"));
    }
    parts.join("；")
}

/// 触发检查并压缩（由 storage::write_memory 调用）
/// 若记忆数超过阈值，将最早的 COMPRESS_BATCH 条「普通记忆」（summary 为 None）合并为摘要。
/// 已有 summary 的摘要记忆不参与再次压缩，避免递归压缩。
pub fn maybe_compress(index_path: &PathBuf, set_name: Option<&str>) -> Result<(), AppError> {
    let _guard = MEMORY_WRITER_LOCK
        .lock()
        .map_err(|_| AppError::MemoryError("记忆写锁获取失败".into()))?;

    // 统一走 storage::read_all（带索引损坏自动重建）
    let mut memories = crate::memory::storage::read_all(index_path)?;
    if memories.len() <= COMPRESS_THRESHOLD {
        return Ok(());
    }

    // 按时间排序（取最早的）
    memories.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    // 只取普通记忆（summary 为 None），再截取最早 COMPRESS_BATCH 条
    let batch: Vec<Memory> = memories
        .iter()
        .filter(|m| m.summary.is_none())
        .take(COMPRESS_BATCH)
        .cloned()
        .collect();

    if batch.is_empty() {
        // 全是摘要记忆，无需压缩
        return Ok(());
    }

    // 记录被合并记忆的 id，用于删除
    let batch_ids: Vec<String> = batch.iter().map(|m| m.id.clone()).collect();

    // 生成摘要记忆
    let summary_text = summarize(&batch);
    let summary_mem = Memory {
        id: crate::utils::gen_id(),
        role: "assistant".to_string(),
        content: format!("【历史摘要】{summary_text}"),
        timestamp: crate::utils::now_str(),
        tags: Some(vec!["summary".to_string()]),
        summary: Some(summary_text),
        category: None,
        use_count: 0,
    };

    // 移除被合并的普通记忆，插入摘要记忆
    memories.retain(|m| !batch_ids.contains(&m.id));
    memories.push(summary_mem);

    // 复用 storage 的原子写回
    crate::memory::storage::atomic_write_index(index_path, &memories)?;

    // 日志记录（配合任务书「任务 11：日志记录」）
    log::info!(
        "记忆自动压缩完成：记忆集={}，合并 {} 条 → 1 条摘要，当前共 {} 条",
        set_name.unwrap_or("default"),
        batch_ids.len(),
        memories.len()
    );

    Ok(())
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
                    category: None,
            use_count: 0,
        }
    }

    #[test]
    fn summarize_joins_content() {
        let batch = vec![mem("1", "今天吃了饭"), mem("2", "明天要出门")];
        let s = summarize(&batch);
        assert!(s.contains("今天吃了饭"));
        assert!(s.contains("明天要出门"));
    }

    #[test]
    fn summarize_truncates_long() {
        let long = "长".repeat(100);
        let batch = vec![mem("1", &long)];
        let s = summarize(&batch);
        // 100 个"长"为 300 字节，截断后应明显更短且不含完整原文
        assert!(s.len() < 300);
        assert!(!s.contains(&long));
    }

    fn mem_with_ts(id: &str, content: &str, ts: &str) -> Memory {
        Memory {
            id: id.to_string(),
            role: "user".to_string(),
            content: content.to_string(),
            timestamp: ts.to_string(),
            tags: None,
            summary: None,
                    category: None,
            use_count: 0,
        }
    }

    #[test]
    fn compress_when_over_threshold() {
        // 构造 21 条记忆，超过阈值 20，写入临时文件后触发压缩
        let dir = std::env::temp_dir().join("memoria_test_compress");
        std::fs::create_dir_all(&dir).unwrap();
        let index_path = dir.join("index.json");

        let mut memories: Vec<Memory> = (0..21)
            .map(|i| {
                mem_with_ts(
                    &format!("id_{i:02}"),
                    &format!("第{i}条记忆内容"),
                    &format!("2026-01-01T00:{i:02}:00.000"),
                )
            })
            .collect();
        memories.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        // 直接写入 21 条
        let json = serde_json::to_string_pretty(&memories).unwrap();
        std::fs::write(&index_path, json).unwrap();

        // 触发压缩
        super::maybe_compress(&index_path, None).unwrap();

        // 读回验证：21 条 → 11 条（合并 10 条为 1 摘要，剩 11 普通 + 1 摘要 = 12）
        let after = crate::memory::storage::read_all(&index_path).unwrap();
        assert_eq!(after.len(), 12, "压缩后应为 11 普通 + 1 摘要 = 12 条");
        // 恰好有一条摘要记忆
        assert_eq!(after.iter().filter(|m| m.summary.is_some()).count(), 1);

        // 清理
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compress_skips_when_all_summary() {
        // 全是摘要记忆（summary 为 Some），不应再压缩
        let mut memories: Vec<Memory> = (0..21)
            .map(|i| {
                let mut m = mem_with_ts(
                    &format!("id_{i:02}"),
                    "摘要",
                    &format!("2026-01-01T00:{i:02}:00.000"),
                );
                m.summary = Some(format!("摘要{i}"));
                m
            })
            .collect();
        memories.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        let dir = std::env::temp_dir().join("memoria_test_compress_all");
        std::fs::create_dir_all(&dir).unwrap();
        let index_path = dir.join("index.json");
        let json = serde_json::to_string_pretty(&memories).unwrap();
        std::fs::write(&index_path, json).unwrap();

        super::maybe_compress(&index_path, None).unwrap();

        let after = crate::memory::storage::read_all(&index_path).unwrap();
        // 全是摘要，不压缩，条数不变
        assert_eq!(after.len(), 21);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
