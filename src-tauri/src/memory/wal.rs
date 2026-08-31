// 《铃·记忆体》WAL 简化版（append-only log）
//
// 设计：每次写入时，将新记忆追加到 <index_path>.wal 文件（每行一条 JSON）。
// 启动时 replay_wal() 将 .wal 内容合并回 index.json，然后清空 .wal。
// 这样常规写路径只做顺序追加（O(1) I/O），避免每次重写整个 index.json（O(N)）。
//
// 局限：.wal 与 index.json 之间不是原子的（极端崩溃时可能重放已有记录）；
// append_memory 已做 id 去重，所以重放是幂等的，不会产生重复数据。
//
// 若后续需要完整 WAL（CRC、sequence number、checkpoint 触发器），可在此基础上扩展。

use crate::error::AppError;
use crate::types::Memory;
use std::io::Write;
use std::path::{Path, PathBuf};

/// 返回对应的 WAL 文件路径
pub fn wal_path(index_path: &Path) -> PathBuf {
    let mut p = index_path.to_path_buf();
    let ext = p
        .extension()
        .map(|e| format!("{}.wal", e.to_string_lossy()))
        .unwrap_or_else(|| "wal".to_string());
    p.set_extension(ext);
    p
}

/// 追加一条记忆到 .wal 文件（每条独立一行 JSON，顺序追加，O(1)）
pub fn append_to_wal(index_path: &Path, memory: &Memory) -> Result<(), AppError> {
    let path = wal_path(index_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(memory)? + "\n";
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    file.write_all(line.as_bytes())?;
    Ok(())
}

/// 启动时回放：将 .wal 中的记录合并（去重）到 index.json，然后清空 .wal。
/// 应在 storage::read_all 被首次调用前（或 init 阶段）调用。
pub fn replay_wal(index_path: &Path) -> Result<(), AppError> {
    let wal = wal_path(index_path);
    if !wal.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(&wal)?;
    let entries: Vec<Memory> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    if entries.is_empty() {
        let _ = std::fs::remove_file(&wal);
        return Ok(());
    }

    // 读现有 index（允许不存在）
    let mut existing: Vec<Memory> = if index_path.exists() {
        std::fs::read_to_string(index_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // 去重合并（id 为主键）
    for entry in entries {
        if !existing.iter().any(|m| m.id == entry.id) {
            existing.push(entry);
        }
    }

    // 原子写回 index.json
    let json = serde_json::to_string_pretty(&existing)?;
    let tmp = index_path.with_extension("json.tmp");
    std::fs::write(&tmp, &json)?;
    std::fs::rename(&tmp, index_path)?;

    // 清空 WAL（截断）
    std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&wal)
        .ok();

    log::info!(
        "[wal] 回放完成：合并 {} 条到 {}",
        existing.len(),
        index_path.display()
    );
    Ok(())
}
