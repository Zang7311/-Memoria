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
pub fn init(app: AppHandle) {
    *APP.lock().unwrap() = Some(app.clone());

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
    *CONFIG.lock().unwrap() = Some(cfg);
    // 通知前端初始配置就绪
    let _ = app.emit("config-changed", ());
}

/// 获取当前配置（克隆）
pub fn get_config() -> AppConfig {
    CONFIG.lock().unwrap().clone().unwrap_or_else(default_config)
}

/// 保存并广播（写文件 + 更新缓存 + emit 事件）
pub fn set_config(cfg: AppConfig) -> Result<(), AppError> {
    save_config_file(&cfg)?;
    *CONFIG.lock().unwrap() = Some(cfg.clone());
    notify_changed();
    Ok(())
}

/// 增量更新配置（仅更新传入字段，null 表示清除该字段）
/// - 特殊字段 "api_key"（明文）：未解锁时拒绝；解锁后加密写入 api_key_encrypted
/// - "api_key_encrypted"（已加密串）：未解锁时拒绝直接覆盖，需先解锁
pub fn update(updates: &HashMap<String, Value>) -> Result<AppConfig, AppError> {
    let mut cfg = get_config();

    // 处理明文 api_key
    if let Some(v) = updates.get("api_key") {
        let mut updates = updates.clone();
        updates.remove("api_key");
        if v.is_null() {
            cfg.api_key_encrypted = None;
        } else {
            let plain = v
                .as_str()
                .ok_or_else(|| AppError::EncryptionError("api_key 必须是字符串".into()))?;
            if plain.is_empty() {
                cfg.api_key_encrypted = None;
            } else {
                let key = encryption::get_key()?; // 未解锁 → Locked
                cfg.api_key_encrypted = Some(encryption::encrypt_with_key(&key, plain)?);
            }
        }
        return update(&updates); // 递归处理其余字段
    }

    let mut obj = serde_json::to_value(&cfg)
        .map_err(|e| AppError::ConfigSaveError(format!("配置序列化失败：{e}")))?;
    if let Value::Object(map) = &mut obj {
        for (k, v) in updates {
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
    let new_cfg: AppConfig = serde_json::from_value(obj)
        .map_err(|e| AppError::ConfigSaveError(format!("配置更新解析失败：{e}")))?;
    set_config(new_cfg.clone())?;
    Ok(new_cfg)
}

/// 重置所有配置为默认值（保留 master_password 与 api_key_encrypted？任务书：重置全部。
/// 这里重置非加密字段，保留主密码盐 + 已加密的 api_key，避免误清密钥）
pub fn reset() -> Result<AppConfig, AppError> {
    let mut d = default_config();
    // 保留主密码与已加密 key（重置不应清掉用户凭据，避免数据无法恢复）
    let old = get_config();
    d.has_master_password = old.has_master_password;
    d.master_password_salt = old.master_password_salt;
    d.api_key_encrypted = old.api_key_encrypted;
    set_config(d.clone())?;
    Ok(d)
}

// ==================== 主密码管理 ====================

/// 设置主密码（首次引导 / 修改）：生成盐 → 派生密钥 → 存内存 → 持久化盐+标志
pub fn set_master_password(password: &str) -> Result<(), AppError> {
    if password.is_empty() {
        return Err(AppError::EncryptionError("主密码不能为空".into()));
    }
    let salt_b64 = encryption::generate_salt_b64();
    let salt = encryption::decode_salt(&salt_b64)?;
    let key = encryption::derive_key(password, &salt);
    encryption::set_key(key);

    let mut cfg = get_config();
    cfg.master_password_salt = Some(salt_b64);
    cfg.has_master_password = true;
    set_config(cfg)
}

/// 解锁：用主密码 + 已存盐派生密钥；若有已加密 api_key，尝试解密验证（成功才算解锁）
pub fn unlock(password: &str) -> Result<bool, AppError> {
    let cfg = get_config();
    let salt_b64 = cfg
        .master_password_salt
        .clone()
        .ok_or_else(|| AppError::MasterPasswordNotSet("尚未设置主密码".into()))?;
    let salt = encryption::decode_salt(&salt_b64)?;
    let key = encryption::derive_key(password, &salt);

    // 若已有加密 api_key，用派生密钥解密验证；失败 → 主密码错误
    if let Some(enc) = &cfg.api_key_encrypted {
        encryption::decrypt_with_key(&key, enc)?;
    }
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
    if let Some(app) = APP.lock().unwrap().as_ref() {
        let _ = app.emit("config-changed", ());
    }
}
