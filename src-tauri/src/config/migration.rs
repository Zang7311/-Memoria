// 《铃·记忆体》配置迁移（AI-7 任务 4 + moon12 旧数据迁移）
// 配置文件含 config_version；结构变更时自动升级到新版本，保留已有数据。
//
// 当前版本：v1。迁移策略：
//   - 解析时先读 config_version（缺省按 v1）
//   - 用默认值补齐所有缺失字段（向前兼容，未来加字段也能自动补）
//   - 若未来需要 v1→v2 结构变更，在此处增加升级步骤（如往旧版本字段补新字段）
//
// moon12：旧版数据目录迁移（%APPDATA% 残留 → 新数据目录）也在此模块实现，
// 见 migrate_legacy_data()，由 config::store::init 启动时调用一次。
use crate::config::defaults::default_config;
use crate::error::AppError;
use serde_json::Value;
use tauri::Emitter;

/// 当前配置版本
pub const CURRENT_VERSION: u32 = 1;

/// 迁移：将原始 JSON 配置解析为 AppConfig
/// - 缺失字段用默认值补齐
/// - config_version 低于当前版本时触发升级逻辑
pub fn migrate(raw: Value) -> Result<crate::types::AppConfig, AppError> {
    let mut obj = match raw {
        Value::Object(m) => m,
        _ => {
            return Err(AppError::ConfigMigrationError(
                "配置文件根节点必须是 JSON 对象".to_string(),
            ))
        }
    };

    // 版本号缺省按 1
    obj.entry("config_version".to_string())
        .or_insert(Value::Number(1.into()));

    // 用默认值补齐所有缺失字段（保证结构完整）
    let defaults = serde_json::to_value(default_config())
        .map_err(|e| AppError::ConfigMigrationError(format!("默认配置序列化失败：{e}")))?;
    if let Value::Object(def_obj) = defaults {
        for (k, v) in def_obj {
            obj.entry(k.clone()).or_insert(v);
        }
    }

    // 未来版本升级：config_version > 1 时在这里逐级升级（当前仅预留）
    let _version = obj
        .get("config_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as u32;
    // —— 预留：if version < 2 { /* v1 → v2 升级逻辑 */ } ——

    // 解析为 AppConfig
    let mut cfg: crate::types::AppConfig =
        serde_json::from_value(Value::Object(obj)).map_err(|e| {
            AppError::ConfigMigrationError(format!("配置解析失败：{e}"))
        })?;

    // 版本号对齐当前（保证后续升级有正确基线）
    cfg.config_version = CURRENT_VERSION;
    Ok(cfg)
}

// ==================== moon12：旧版数据目录迁移 ====================

/// 旧数据目录迁移（幂等，由 config::store::init 启动时调用）
///
/// 背景：早期版本数据可能残留在 %APPDATA% 下的旧目录（如 com.zang7311.memoria、
/// ling-memoria 等），而新版本使用 ~/.铃记忆体（配置）+ data_path（记忆）。
/// 首次启动时若发现旧数据目录有内容、且新目录为空 → 复制旧记忆/配置到新目录，
/// 写标记文件防重复迁移，并通过事件通知前端提示用户。
///
/// 安全原则：只复制不移动；新位置已有数据时跳过，绝不覆盖用户新数据。
/// 分两步：① 迁移配置（须在配置加载前）；② 配置就绪后再迁移记忆（依赖 data_path）。
pub fn migrate_legacy_data(app: &tauri::AppHandle) {
    migrate_legacy_config();
    migrate_legacy_memory(app);
}

/// ① 旧配置 → ~/.铃记忆体/config.json（仅当新配置不存在）
pub fn migrate_legacy_config() {
    use std::path::PathBuf;

    let appdata = std::env::var("APPDATA").unwrap_or_default();
    let old_config_candidates = [
        PathBuf::from(&appdata).join("com.zang7311.memoria"), // 早期 identifier 路径（任务书点名）
        PathBuf::from(&appdata).join("com.zang-服务器.mem"),  // 当前 identifier 的 Tauri 默认路径
    ];

    // 新配置已存在 → 不迁移（避免覆盖用户新配置）
    let new_cfg = crate::config::config_path();
    if new_cfg.exists() {
        return;
    }
    for old in old_config_candidates.iter() {
        let old_cfg = old.join("config.json");
        if old_cfg.exists() {
            if let Some(parent) = new_cfg.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::fs::copy(&old_cfg, &new_cfg).is_ok() {
                log::info!(
                    "[迁移] 旧配置已复制：{} → {}",
                    old_cfg.display(),
                    new_cfg.display()
                );
            }
            break; // 只迁移第一个命中的旧配置
        }
    }
}

/// ② 旧记忆 → 新记忆根目录（依赖已加载的配置：data_path 或默认回退；仅当新记忆为空）
pub fn migrate_legacy_memory(app: &tauri::AppHandle) {
    use std::path::PathBuf;

    // 标记文件：~/.铃记忆体/.migrated_from_legacy（存在 = 已迁移过，跳过）
    let mark_path = crate::config::data_dir().join(".migrated_from_legacy");
    if mark_path.exists() {
        return;
    }

    let appdata = std::env::var("APPDATA").unwrap_or_default();
    // 旧记忆候选：AI-3/AI-4 时代定稿的默认路径（实测存在 48 条记忆）
    let old_memory_candidates = [PathBuf::from(&appdata).join("ling-memoria").join("memory")];

    let mut migrated_any = false;
    let mut sources: Vec<String> = Vec::new();

    // 新记忆根目录（此时配置已加载，data_path 生效）
    let new_memory_root = crate::memory::storage::root_dir();
    let new_index = new_memory_root.join("index.json");
    let new_empty = !new_index.exists();
    if new_empty {
        for old in old_memory_candidates.iter() {
            let old_index = old.join("index.json");
            if old_index.exists() {
                // 目标目录与来源相同（data_path 未设置时回退同一路径）→ 无需迁移
                if old.canonicalize().ok() == new_memory_root.canonicalize().ok() {
                    continue;
                }
                if copy_dir_recursive(old, &new_memory_root) {
                    log::info!(
                        "[迁移] 旧记忆已复制：{} → {}",
                        old.display(),
                        new_memory_root.display()
                    );
                    sources.push(old.display().to_string());
                    migrated_any = true;
                }
                break;
            }
        }
    }

    // 写迁移标记（无论是否迁移成功都标记，避免每次启动重复扫描）
    if let Some(parent) = mark_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        &mark_path,
        format!(
            "migrated_at={}\nsources={}",
            crate::utils::now_str(),
            if sources.is_empty() {
                "none".to_string()
            } else {
                sources.join(";")
            }
        ),
    );

    // 迁移过数据 → 通知前端提示用户
    if migrated_any {
        log::info!("[迁移] 旧版数据迁移完成：{}", sources.join("; "));
        let _ = app.emit("legacy-migrated", sources.join(";"));
    }
}

/// 递归复制目录（只复制不覆盖已存在文件，保证幂等与安全）
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> bool {
    use std::fs;
    let mut ok = true;
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let from = entry.path();
            let to = dst.join(entry.file_name());
            if from.is_dir() {
                let _ = fs::create_dir_all(&to);
                if !copy_dir_recursive(&from, &to) {
                    ok = false;
                }
            } else if from.is_file() {
                // 目标已存在 → 跳过（不覆盖用户新数据）
                if !to.exists() {
                    if fs::copy(&from, &to).is_err() {
                        ok = false;
                    }
                }
            }
        }
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn migrate_fills_defaults() {
        // 空配置 → 全默认
        let cfg = migrate(json!({})).unwrap();
        assert_eq!(cfg.theme, "dark");
        assert_eq!(cfg.model_mode, "script");
        assert_eq!(cfg.first_launch, true);
        assert_eq!(cfg.config_version, CURRENT_VERSION);
    }

    #[test]
    fn migrate_preserves_existing() {
        let cfg = migrate(json!({ "theme": "light", "context_length": 20 })).unwrap();
        assert_eq!(cfg.theme, "light");
        assert_eq!(cfg.context_length, 20);
        // 未提供的用默认
        assert_eq!(cfg.model_mode, "script");
    }

    #[test]
    fn migrate_rejects_non_object() {
        assert!(migrate(json!([1, 2, 3])).is_err());
    }
}
