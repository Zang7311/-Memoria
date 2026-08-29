// 《铃·记忆体》配置迁移（AI-7 任务 4）
// 配置文件含 config_version；结构变更时自动升级到新版本，保留已有数据。
//
// 当前版本：v1。迁移策略：
//   - 解析时先读 config_version（缺省按 v1）
//   - 用默认值补齐所有缺失字段（向前兼容，未来加字段也能自动补）
//   - 若未来需要 v1→v2 结构变更，在此处增加升级步骤（如往旧版本字段补新字段）
use crate::config::defaults::default_config;
use crate::error::AppError;
use serde_json::Value;

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
