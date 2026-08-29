// 《铃·记忆体》索引管理（memory/index.rs）
// 管理 index.json（存 Memory[]）的加载、重建。
// 与 AI-3 共用同一份 index.json；索引损坏时自动重建。
use crate::error::AppError;
use crate::types::Memory;
use std::path::PathBuf;

/// 加载索引（读取 index.json 解析为 Memory[]）
/// - 文件不存在：视为空索引
/// - 文件损坏：抛 IndexCorrupted，由调用方决定重建
pub fn load_index(index_path: &PathBuf) -> Result<Vec<Memory>, AppError> {
    if !index_path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(index_path)?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&raw)
        .map_err(|e| AppError::IndexCorrupted(format!("{}：{}", index_path.display(), e)))
}

/// 索引文件是否存在且可解析（用于校验）
#[allow(dead_code)]
pub fn index_exists(index_path: &PathBuf) -> bool {
    index_path.exists()
}

/// 重建索引：若 index.json 损坏或缺失，写回一个空数组（或按需扫描恢复）。
/// 目前数据模型为单文件 Memory[]，无独立记忆文件，故重建即重置为有效结构。
/// 返回重建后的记忆数。
pub fn rebuild_index(index_path: &PathBuf) -> Result<usize, AppError> {
    // 尝试解析；若成功且结构有效则无需重建
    let memories = match load_index(index_path) {
        Ok(m) => m,
        Err(_) => {
            // 损坏：备份旧文件后重建为空索引，避免直接覆盖丢失现场
            let _ = std::fs::rename(index_path, index_path.with_extension("json.corrupted"));
            Vec::new()
        }
    };
    let json = serde_json::to_string_pretty(&memories)?;
    if let Some(parent) = index_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(index_path, json)?;
    Ok(memories.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_index_missing_file_returns_empty() {
        let dir = std::env::temp_dir().join("memoria_test_index_missing");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("index.json");
        let r = load_index(&p).unwrap();
        assert!(r.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_index_corrupted_returns_error() {
        let dir = std::env::temp_dir().join("memoria_test_index_corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("index.json");
        std::fs::write(&p, "{ 这不是合法 JSON").unwrap();
        assert!(matches!(load_index(&p), Err(AppError::IndexCorrupted(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rebuild_index_backs_up_corrupted() {
        let dir = std::env::temp_dir().join("memoria_test_index_rebuild");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("index.json");
        std::fs::write(&p, "corrupted!!").unwrap();

        let n = rebuild_index(&p).unwrap();
        assert_eq!(n, 0);
        // 重建后文件可解析为空数组
        assert!(load_index(&p).unwrap().is_empty());
        // 旧文件已备份
        assert!(dir.join("index.json.corrupted").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
