// 《铃·记忆体》配置存储与持久化（AI-7 任务 1）
//
// ⚠️ 架构债务（主人拍板）：本模块只负责 AI-7 的配置中心 ~/.铃记忆体/config.json。
// 其中 monitor_rules / toolbox_items 字段归 AI-6 所有，AI-7 不负责持久化
// （AI-6 运行时数据在 %APPDATA%/ling-memoria/）。此处仅保留字段作为未来统一入口。
//
// 变更通知：任何配置保存后 emit "config-changed" 事件，其他模块监听后重新读取。
use crate::config::{self, defaults::default_config, encryption, migration};
use crate::error::AppError;
use crate::types::AppConfig;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use serde_json::Value;

/// 全局配置缓存（进程内共享）
static CONFIG: Mutex<Option<AppConfig>> = Mutex::new(None);
/// Tauri AppHandle（用于 emit 配置变更事件）
static APP: Mutex<Option<AppHandle>> = Mutex::new(None);

/// 初始化配置中心（lib.rs setup 时调用一次）
/// - 无配置文件 → 写默认配置
/// - 有配置文件 → 迁移后加载
/// - moon12：启动时先做旧版数据目录迁移（旧配置→新配置，配置就绪后旧记忆→新记忆）
pub fn init(app: AppHandle) {
    *APP.lock().unwrap_or_else(|e| e.into_inner()) = Some(app.clone());

    // moon12 ①：旧版配置迁移（仅当新配置不存在时复制，不覆盖用户数据）
    migration::migrate_legacy_config();

    let cfg = if !config::config_path().exists() {
        let d = default_config();
        let _ = save_config_file(&d);
        d
    } else {
        match load_from_file() {
            Ok(c) => c,
            Err(e) => {
                log::error!("[config] 配置加载失败，回退默认配置：{e}");
                default_config()
            }
        }
    };
    *CONFIG.lock().unwrap_or_else(|e| e.into_inner()) = Some(cfg);

    // moon12 ②：配置就绪后迁移旧记忆（依赖 data_path；幂等，只复制不覆盖）
    migration::migrate_legacy_memory(&app);

    // 通知前端初始配置就绪
    let _ = app.emit("config-changed", ());
}

/// 获取当前配置（克隆）
pub fn get_config() -> AppConfig {
    CONFIG.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_else(default_config)
}

/// 保存并广播（写文件 + 更新缓存 + emit 事件）
pub fn set_config(cfg: AppConfig) -> Result<(), AppError> {
    save_config_file(&cfg)?;
    *CONFIG.lock().unwrap_or_else(|e| e.into_inner()) = Some(cfg.clone());
    notify_changed();
    Ok(())
}

/// 增量更新配置（仅更新传入字段，null 表示清除该字段）
/// - 特殊字段 "api_key"（明文）：已解锁时加密；未解锁时沿用明文存储
/// - "api_key_encrypted"（已加密串）：未解锁时拒绝直接覆盖，需先解锁
pub fn update(updates: &HashMap<String, Value>) -> Result<AppConfig, AppError> {
    let mut cfg = get_config();

    // 处理明文 api_key（主密码可选：已解锁→加密存储；未设置主密码→明文存储）
    if let Some(v) = updates.get("api_key") {
        if v.is_null() {
            cfg.api_key_encrypted = None;
            cfg.api_key_plain = None;
        } else {
            let plain = v
                .as_str()
                .ok_or_else(|| AppError::EncryptionError("api_key 必须是字符串".into()))?;
            if plain.is_empty() {
                cfg.api_key_encrypted = None;
                cfg.api_key_plain = None;
            } else if encryption::is_unlocked() {
                // 已解锁：AES-256-GCM 加密存储（推荐）
                let key = encryption::get_key()?;
                cfg.api_key_encrypted = Some(encryption::encrypt_with_key(&key, plain)?);
                cfg.api_key_plain = None;
            } else {
                // 未设置/未解锁主密码：明文存储（前端会提示「未加密」，便于快速接入）
                cfg.api_key_encrypted = None;
                cfg.api_key_plain = Some(plain.to_string());
            }
        }
    }

    if let Some(value) = updates.get("cheap_api_key") {
        let key = if encryption::is_unlocked() {
            Some(encryption::get_key()?)
        } else {
            None
        };
        update_cheap_api_key(&mut cfg, value, key.as_ref())?;
    }

    let mut obj = serde_json::to_value(&cfg)
        .map_err(|e| AppError::ConfigSaveError(format!("配置序列化失败：{e}")))?;
    if let Value::Object(map) = &mut obj {
        for (k, v) in updates {
            if k == "api_key" || k == "cheap_api_key" {
                continue;
            }
            if k == "cheap_api_key_encrypted" {
                encryption::get_key()?;
                if v.is_null() {
                    map.remove(k);
                } else {
                    map.insert(k.clone(), v.clone());
                }
                continue;
            }
            // api_key_encrypted 直接写入需先解锁（防止用他人密钥覆盖）
            if k == "api_key_encrypted" {
                encryption::get_key()?;
                if v.is_null() {
                    map.remove(k);
                } else {
                    map.insert(k.clone(), v.clone());
                }
                continue;
            }
            if v.is_null() {
                map.remove(k);
            } else {
                map.insert(k.clone(), v.clone());
            }
        }
    }
    // 容错：u8 整数型字段若前端误传字符串（如 "4"），转换为数字，避免反序列化失败
    if let Value::Object(map) = &mut obj {
        for k in [
            "context_length",
            "depth",
            "language_mix_rate",
            "ui_radius",
        ] {
            if let Some(v) = map.get_mut(k) {
                if let Value::String(s) = v {
                    if let Ok(n) = s.parse::<u8>() {
                        *v = Value::Number(n.into());
                    }
                }
            }
        }
    }
    let new_cfg: AppConfig = serde_json::from_value(obj)
        .map_err(|e| AppError::ConfigSaveError(format!("配置更新解析失败：{e}")))?;
    set_config(new_cfg.clone())?;
    Ok(new_cfg)
}

fn update_cheap_api_key(
    cfg: &mut AppConfig,
    value: &Value,
    unlocked_key: Option<&[u8; encryption::KEY_LEN]>,
) -> Result<(), AppError> {
    if value.is_null() {
        cfg.cheap_api_key_encrypted = None;
        cfg.cheap_api_key_plain = None;
    } else {
        let plain = value
            .as_str()
            .ok_or_else(|| AppError::EncryptionError("cheap_api_key 必须是字符串".into()))?;
        if plain.is_empty() {
            cfg.cheap_api_key_encrypted = None;
            cfg.cheap_api_key_plain = None;
        } else if let Some(key) = unlocked_key {
            cfg.cheap_api_key_encrypted = Some(encryption::encrypt_with_key(key, plain)?);
            cfg.cheap_api_key_plain = None;
        } else {
            cfg.cheap_api_key_encrypted = None;
            cfg.cheap_api_key_plain = Some(plain.to_string());
        }
    }
    Ok(())
}

/// 重置所有配置为默认值（保留 master_password 与 api_key_encrypted？任务书：重置全部。
/// 这里重置非加密字段，保留主密码盐 + 已加密的 api_key，避免误清密钥）
pub fn reset() -> Result<AppConfig, AppError> {
    let mut d = default_config();
    // 保留主密码与已加密 key（重置不应清掉用户凭据，避免数据无法恢复）
    let old = get_config();
    d.has_master_password = old.has_master_password;
    d.master_password_salt = old.master_password_salt;
    d.master_password_check = old.master_password_check;
    d.api_key_encrypted = old.api_key_encrypted;
    set_config(d.clone())?;
    Ok(d)
}

// ==================== 主密码管理 ====================

/// 设置主密码（首次引导 / 修改）：生成盐 → 派生密钥 → 加密校验段 → 存内存 → 持久化
pub fn set_master_password(password: &str) -> Result<(), AppError> {
    if password.is_empty() {
        return Err(AppError::EncryptionError("主密码不能为空".into()));
    }
    let salt_b64 = encryption::generate_salt_b64();
    let salt = encryption::decode_salt(&salt_b64)?;
    let key = encryption::derive_key(password, &salt);
    // 用派生密钥加密固定校验明文，后续 unlock 时用于验证密码是否正确
    let check = encryption::encrypt_with_key(&key, "ling-check-v1")?;
    encryption::set_key(key);

    let mut cfg = get_config();
    cfg.master_password_salt = Some(salt_b64);
    cfg.has_master_password = true;
    cfg.master_password_check = Some(check);
    set_config(cfg)
}

/// 解锁：用主密码 + 已存盐派生密钥，先解密校验段验证密码正确性，再持有密钥
pub fn unlock(password: &str) -> Result<bool, AppError> {
    let cfg = get_config();
    let salt_b64 = cfg
        .master_password_salt
        .clone()
        .ok_or_else(|| AppError::MasterPasswordNotSet("尚未设置主密码".into()))?;
    let salt = encryption::decode_salt(&salt_b64)?;
    let key = encryption::derive_key(password, &salt);

    // 优先用校验段验证密码（set_master_password 时写入）
    if let Some(check) = &cfg.master_password_check {
        encryption::decrypt_with_key(&key, check)
            .map_err(|_| AppError::MasterPasswordWrong("主密码不正确".into()))?;
    } else if let Some(enc) = &cfg.api_key_encrypted {
        // 旧数据：无校验段，退回用 api_key 验证（兼容）
        encryption::decrypt_with_key(&key, enc)
            .map_err(|_| AppError::MasterPasswordWrong("主密码不正确".into()))?;
    } else if cfg.has_master_password {
        // 有主密码标记但既无校验段又无加密 key，说明数据损坏，拒绝接受
        return Err(AppError::MasterPasswordWrong(
            "主密码校验数据丢失，无法验证密码（配置可能已损坏）".into(),
        ));
    }
    // has_master_password 为假且无任何校验材料：首次兼容旧数据，直接接受
    encryption::set_key(key);
    Ok(true)
}

/// 当前主密码状态（是否设置过 + 是否已解锁）
pub fn master_password_status() -> crate::types::MasterPasswordStatus {
    let cfg = get_config();
    crate::types::MasterPasswordStatus {
        has_master_password: cfg.has_master_password,
        unlocked: encryption::is_unlocked(),
    }
}

// ==================== 文件读写 ====================

/// 从文件加载并迁移配置
fn load_from_file() -> Result<AppConfig, AppError> {
    let path = config::config_path();
    let s = std::fs::read_to_string(&path)
        .map_err(|e| AppError::ConfigLoadError(format!("读取 {} 失败：{e}", path.display())))?;
    let raw: Value =
        serde_json::from_str(&s).map_err(|e| AppError::ConfigLoadError(format!("JSON 解析失败：{e}")))?;
    migration::migrate(raw)
}

/// 保存配置到文件（先确保目录存在）
fn save_config_file(cfg: &AppConfig) -> Result<(), AppError> {
    config::ensure_data_dir()
        .map_err(|e| AppError::ConfigSaveError(format!("创建配置目录失败：{e}")))?;
    let s = serde_json::to_string_pretty(cfg)
        .map_err(|e| AppError::ConfigSaveError(format!("序列化失败：{e}")))?;
    std::fs::write(config::config_path(), s)
        .map_err(|e| AppError::ConfigSaveError(format!("写入失败：{e}")))?;
    Ok(())
}

/// 广播配置变更事件
fn notify_changed() {
    if let Some(app) = APP.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        let _ = app.emit("config-changed", ());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_keys_update_persists_without_losing_other_fields() {
        const CHILD_MARKER: &str = "MEM_CONFIG_UPDATE_TEST_CHILD";
        if std::env::var_os(CHILD_MARKER).is_none() {
            // 子进程隔离全局配置与主密码状态，绝不改写用户配置或干扰并行测试。
            let directory = std::env::temp_dir().join(format!("mem-config-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&directory).expect("test directory");
            let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
                .args(["--exact", "config::store::tests::api_keys_update_persists_without_losing_other_fields", "--nocapture"])
                .env(CHILD_MARKER, "1")
                .env("USERPROFILE", &directory)
                .output().expect("isolated config test");
            std::fs::remove_dir_all(&directory).expect("remove test directory");
            assert!(output.status.success(), "isolated config test failed: {}", String::from_utf8_lossy(&output.stderr));
            return;
        }

        encryption::clear_key();
        set_config(default_config()).expect("initial config");
        update(&HashMap::from([("api_key".into(), Value::String("sk-test".into()))])).expect("save main key");
        assert_eq!(get_config().api_key_plain.as_deref(), Some("sk-test"));
        update(&HashMap::from([("cheap_api_key".into(), Value::String("sk-cheap-test".into()))])).expect("save cheap key");
        assert_eq!(get_config().cheap_api_key_plain.as_deref(), Some("sk-cheap-test"));

        let updates = HashMap::from([
            ("api_key".into(), Value::String("sk-new-main".into())),
            ("cheap_api_key".into(), Value::String("sk-new-cheap".into())),
            ("api_model".into(), Value::String("new-model".into())),
        ]);
        update(&updates).expect("save combined update");
        for cfg in [get_config(), load_from_file().expect("persisted config")] {
            assert_eq!(cfg.api_key_plain.as_deref(), Some("sk-new-main"));
            assert_eq!(cfg.cheap_api_key_plain.as_deref(), Some("sk-new-cheap"));
            assert_eq!(cfg.api_model, "new-model");
            assert!(cfg.api_key_encrypted.is_none());
            assert!(cfg.cheap_api_key_encrypted.is_none());
            let response = serde_json::to_string(&crate::types::GetConfigResponse::from_config(&cfg).expect("public config")).expect("serialize public config");
            assert!(!response.contains("sk-new-main"));
            assert!(!response.contains("sk-new-cheap"));
        }

        let key = [42; encryption::KEY_LEN];
        encryption::set_key(key);
        update(&updates).expect("save encrypted update");
        let cfg = load_from_file().expect("encrypted config");
        assert!(cfg.api_key_plain.is_none());
        assert!(cfg.cheap_api_key_plain.is_none());
        assert_eq!(encryption::decrypt_with_key(&key, cfg.api_key_encrypted.as_deref().expect("main ciphertext")).expect("decrypt main"), "sk-new-main");
        assert_eq!(encryption::decrypt_with_key(&key, cfg.cheap_api_key_encrypted.as_deref().expect("cheap ciphertext")).expect("decrypt cheap"), "sk-new-cheap");
        encryption::clear_key();
        for value in [Value::Null, Value::String(String::new())] {
            update(&HashMap::from([("api_key".into(), value.clone()), ("cheap_api_key".into(), value)])).expect("clear keys");
            let cfg = get_config();
            assert!(cfg.api_key_plain.is_none() && cfg.api_key_encrypted.is_none());
            assert!(cfg.cheap_api_key_plain.is_none() && cfg.cheap_api_key_encrypted.is_none());
        }
    }

    #[test]
    fn cheap_key_locked_saves_plaintext_and_clears_ciphertext() {
        let mut cfg = default_config();
        cfg.api_key_plain = Some("main-secret".into());
        cfg.cheap_api_key_encrypted = Some("old-ciphertext".into());
        update_cheap_api_key(&mut cfg, &Value::String("cheap-secret".into()), None).expect("save key");
        assert_eq!(cfg.cheap_api_key_plain.as_deref(), Some("cheap-secret"));
        assert!(cfg.cheap_api_key_encrypted.is_none());
        assert_eq!(cfg.api_key_plain.as_deref(), Some("main-secret"));
    }

    #[test]
    fn cheap_key_unlocked_encrypts_and_clears_plaintext() {
        let mut cfg = default_config();
        cfg.cheap_api_key_plain = Some("old-secret".into());
        let key = [42; encryption::KEY_LEN];
        update_cheap_api_key(&mut cfg, &Value::String("cheap-secret".into()), Some(&key)).expect("save key");
        assert!(cfg.cheap_api_key_plain.is_none());
        let encrypted = cfg.cheap_api_key_encrypted.as_deref().expect("ciphertext");
        assert!(!encrypted.contains("cheap-secret"));
        assert_eq!(encryption::decrypt_with_key(&key, encrypted).expect("decrypt key"), "cheap-secret");
    }

    #[test]
    fn cheap_key_null_and_empty_clear_both_storage_fields() {
        for value in [Value::Null, Value::String(String::new())] {
            let mut cfg = default_config();
            cfg.cheap_api_key_plain = Some("old-secret".into());
            cfg.cheap_api_key_encrypted = Some("old-ciphertext".into());
            update_cheap_api_key(&mut cfg, &value, None).expect("clear key");
            assert!(cfg.cheap_api_key_plain.is_none());
            assert!(cfg.cheap_api_key_encrypted.is_none());
        }
    }

    #[test]
    fn cheap_key_invalid_type_is_rejected_without_changes() {
        let mut cfg = default_config();
        cfg.cheap_api_key_plain = Some("old-secret".into());
        assert!(matches!(
            update_cheap_api_key(&mut cfg, &Value::Bool(true), None),
            Err(AppError::EncryptionError(_))
        ));
        assert_eq!(cfg.cheap_api_key_plain.as_deref(), Some("old-secret"));
    }
}
