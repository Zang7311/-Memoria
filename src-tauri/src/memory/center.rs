// 《铃·记忆体》记忆中心命令（记忆中心大项目）
// 容量统计 / 分类分布 / 重复检测 / 批量删除
use crate::error::AppError;
use crate::memory::category::{category_distribution, classify, DEFAULT_CATEGORY};
use crate::memory::storage;
use crate::types::Memory;
use std::collections::HashMap;

/// 记忆中心统计：条数/占用空间/分类分布/重复数/重要数
#[tauri::command]
pub async fn memory_stats(set_name: Option<String>) -> Result<serde_json::Value, AppError> {
    let index_path = storage::set_index_path(set_name.as_deref());
    let mut memories = storage::read_all(&index_path)?;

    // 分类分布（读取时顺带补分类标签——旧数据无分类的自动补齐）
    let mut need_save = false;
    for m in memories.iter_mut() {
        if m.category.is_none() {
            m.category = Some(classify(&m.content));
            need_save = true;
        }
    }
    if need_save {
        let _ = storage::write_all(&index_path, &memories);
    }
    let dist = category_distribution(&memories);

    // 容量：index.json 大小（MB）
    let size_bytes = std::fs::metadata(&index_path).map(|m| m.len()).unwrap_or(0);
    let size_mb = size_bytes as f64 / 1024.0 / 1024.0;

    // 重复检测：内容相同（去除首尾空白）的条数
    let mut content_map: HashMap<String, usize> = HashMap::new();
    for m in &memories {
        let key = m.content.trim().to_string();
        if !key.is_empty() {
            *content_map.entry(key).or_insert(0) += 1;
        }
    }
    let duplicate_count: usize = content_map.values().filter(|&&n| n > 1).map(|&n| n - 1).sum();

    // 重要记忆数
    let important_count = memories
        .iter()
        .filter(|m| m.tags.as_ref().map(|t| t.contains(&"important".to_string())).unwrap_or(false))
        .count();

    Ok(serde_json::json!({
        "total": memories.len(),
        "size_mb": (size_mb * 100.0).round() / 100.0,
        "important_count": important_count,
        "duplicate_count": duplicate_count,
        "categories": dist.iter().map(|(c, n)| serde_json::json!({ "name": c, "count": n })).collect::<Vec<_>>(),
    }))
}

/// 批量删除记忆（记忆中心：勾选多条删除）
#[tauri::command]
pub async fn delete_memories_batch(ids: Vec<String>, set_name: Option<String>) -> Result<usize, AppError> {
    let index_path = storage::set_index_path(set_name.as_deref());
    let mut memories = storage::read_all(&index_path)?;
    let before = memories.len();
    memories.retain(|m| !ids.contains(&m.id));
    let deleted = before - memories.len();
    if deleted > 0 {
        // 被删除的记忆，向量缓存一并失效（避免残留孤儿向量）
        for id in &ids {
            crate::memory::vector::invalidate(id);
        }
        let _ = storage::write_all(&index_path, &memories);
    }
    Ok(deleted)
}

/// 标记/取消重要（批量版，供记忆中心勾选操作）
#[tauri::command]
pub async fn mark_important_batch(ids: Vec<String>, important: bool, set_name: Option<String>) -> Result<usize, AppError> {
    let index_path = storage::set_index_path(set_name.as_deref());
    let mut memories = storage::read_all(&index_path)?;
    let mut changed = 0usize;
    for m in memories.iter_mut() {
        if ids.contains(&m.id) {
            let tags = m.tags.get_or_insert_with(Vec::new);
            let has = tags.iter().any(|t| t == "important");
            if important && !has {
                tags.push("important".to_string());
                changed += 1;
            } else if !important && has {
                tags.retain(|t| t != "important");
                changed += 1;
            }
        }
    }
    if changed > 0 {
        let _ = storage::write_all(&index_path, &memories);
    }
    Ok(changed)
}

/// 修改一条记忆的内容（记忆中心编辑）
#[tauri::command]
pub async fn edit_memory_content(id: String, content: String, set_name: Option<String>) -> Result<(), AppError> {
    let index_path = storage::set_index_path(set_name.as_deref());
    let mut memories = storage::read_all(&index_path)?;
    if let Some(m) = memories.iter_mut().find(|m| m.id == id) {
        m.content = content;
        m.summary = None; // 摘要失效，重新生成
        // 内容变了重新分类
        m.category = Some(classify(&m.content));
        let _ = storage::write_all(&index_path, &memories);
        crate::memory::vector::invalidate(&id);
        Ok(())
    } else {
        Err(AppError::MemoryNotFound(id))
    }
}

/// 补一个默认实现兜底（供测试/其他地方引用 Memory 构造）
#[allow(dead_code)]
pub fn placeholder_memory(id: &str) -> Memory {
    Memory {
        id: id.to_string(),
        role: "user".to_string(),
        content: String::new(),
        timestamp: String::new(),
        tags: None,
        summary: None,
        category: None,
        use_count: 0,
    }
}

/// 兼容：默认分类常量导出
#[allow(dead_code)]
pub fn default_category() -> &'static str {
    DEFAULT_CATEGORY
}
