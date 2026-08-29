// 《铃·记忆体》冲突检测与解决（AI-8）
//
// 同一记忆（id 相同）在不同设备上被修改 → 冲突。
// 解决策略（任务书任务 5）：
//   - newest：保留时间戳较新的版本（默认）
//   - local ：始终保留本地版本
//   - remote：始终保留远程版本
// 用户偏好持久化于 ~/.铃记忆体/sync_config.json（conflict_policy 字段）。
use crate::error::AppError;
use crate::types::{ConflictPolicy, Memory, SyncHistoryEntry, SyncStatus};
use chrono::DateTime;
use std::sync::{LazyLock, Mutex};

/// 同步配置（设备身份 + 冲突策略），落盘 ~/.铃记忆体/sync_config.json
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncConfig {
    /// 本机设备 ID（首次启动生成，uuid v4）
    pub device_id: String,
    /// 本机设备名称（如 "主人-PC"）
    pub device_name: String,
    /// 冲突解决策略
    #[serde(default)]
    pub conflict_policy: ConflictPolicy,
}

/// 同步运行时状态（进度/历史，不落盘或随历史落盘）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncRuntime {
    pub status: String, // idle / discovering / syncing / done / error
    pub progress: f32,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub history: Vec<SyncHistoryEntry>,
}

impl Default for SyncRuntime {
    fn default() -> Self {
        SyncRuntime {
            status: "idle".into(),
            progress: 0.0,
            message: None,
            history: Vec::new(),
        }
    }
}

static CONFIG: Mutex<Option<SyncConfig>> = Mutex::new(None);
static RUNTIME: LazyLock<Mutex<SyncRuntime>> = LazyLock::new(|| Mutex::new(SyncRuntime::default()));

/// 同步配置路径：~/.铃记忆体/sync_config.json
pub fn config_path() -> std::path::PathBuf {
    crate::config::data_dir().join("sync_config.json")
}

/// 初始化同步配置（lib.rs setup 调用）：加载或首次生成设备 ID
pub fn init() {
    let loaded = std::fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str::<SyncConfig>(&s).ok());
    let cfg = loaded.unwrap_or_else(|| SyncConfig {
        device_id: uuid::Uuid::new_v4().to_string(),
        device_name: default_device_name(),
        conflict_policy: ConflictPolicy::Newest,
    });
    // 补默认名称（老配置可能缺）
    let cfg = if cfg.device_name.is_empty() {
        SyncConfig {
            device_name: default_device_name(),
            ..cfg
        }
    } else {
        cfg
    };
    *CONFIG.lock().unwrap() = Some(cfg.clone());
    let _ = persist(&cfg);
    log::info!("[sync] 同步配置就绪：device_id={}", cfg.device_id);
}

/// 默认设备名：主机名
fn default_device_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "铃-设备".to_string())
}

/// 读取当前同步配置
pub fn get_config() -> SyncConfig {
    CONFIG.lock().unwrap().clone().unwrap_or_else(|| {
        // 未 init 时兜底生成
        let cfg = SyncConfig {
            device_id: uuid::Uuid::new_v4().to_string(),
            device_name: default_device_name(),
            conflict_policy: ConflictPolicy::Newest,
        };
        *CONFIG.lock().unwrap() = Some(cfg.clone());
        cfg
    })
}

/// 持久化配置
pub fn persist(cfg: &SyncConfig) -> Result<(), AppError> {
    crate::config::ensure_data_dir()?;
    let s = serde_json::to_string_pretty(cfg)?;
    std::fs::write(config_path(), s)?;
    Ok(())
}

/// 设置冲突策略（前端设置）
pub fn set_conflict_policy(policy: ConflictPolicy) -> Result<(), AppError> {
    let mut cfg = get_config();
    cfg.conflict_policy = policy;
    *CONFIG.lock().unwrap() = Some(cfg.clone());
    persist(&cfg)
}

/// 合并记忆：冲突检测 + 按策略解决
/// 返回 (合并后的新记忆列表, 冲突解决次数, 实际写入条数)
pub fn merge_memories(
    local: &[Memory],
    remote: &[Memory],
    policy: ConflictPolicy,
) -> (Vec<Memory>, usize, usize) {
    let mut merged: Vec<Memory> = local.to_vec();
    let mut conflicts = 0usize;
    let mut written = 0usize;

    for r in remote {
        match merged.iter_mut().find(|m| m.id == r.id) {
            Some(l) => {
                // 冲突：同 id 存在
                if l.timestamp != r.timestamp {
                    conflicts += 1;
                    match policy {
                        ConflictPolicy::Newest => {
                            if ts_of(r) > ts_of(l) {
                                *l = r.clone();
                                written += 1;
                            }
                        }
                        ConflictPolicy::Local => { /* 保留本地，不写 */ }
                        ConflictPolicy::Remote => {
                            *l = r.clone();
                            written += 1;
                        }
                    }
                }
                // 时间戳相同视为同一内容，忽略
            }
            None => {
                merged.push(r.clone());
                written += 1;
            }
        }
    }
    (merged, conflicts, written)
}

/// 解析 ISO 时间戳，失败按 0 处理（保证可比较）
fn ts_of(m: &Memory) -> i64 {
    DateTime::parse_from_rfc3339(&m.timestamp)
        .map(|d| d.timestamp())
        .unwrap_or(0)
}

/// 记录一条同步历史（保留最近 20 条）
pub fn record_history(entry: SyncHistoryEntry) {
    let mut rt = RUNTIME.lock().unwrap();
    rt.history.insert(0, entry);
    rt.history.truncate(20);
}

/// 更新运行时状态
pub fn set_status(status: &str, progress: f32, message: Option<String>) {
    let mut rt = RUNTIME.lock().unwrap();
    rt.status = status.to_string();
    rt.progress = progress;
    rt.message = message;
}

/// 读取当前同步状态（供 get_sync_status 命令）
pub fn get_sync_status() -> SyncStatus {
    let rt = RUNTIME.lock().unwrap();
    SyncStatus {
        status: rt.status.clone(),
        progress: rt.progress,
        message: rt.message.clone(),
        history: rt.history.clone(),
    }
}

/// 时间戳过滤（增量同步）：返回 ts > last_sync_time 的记忆
pub fn filter_incremental(memories: &[Memory], last_sync_time: Option<&str>) -> Vec<Memory> {
    match last_sync_time {
        None => memories.to_vec(),
        Some(ts_str) => {
            let base = DateTime::parse_from_rfc3339(ts_str)
                .map(|d| d.timestamp())
                .unwrap_or(0);
            memories
                .iter()
                .filter(|m| ts_of(m) > base)
                .cloned()
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem(id: &str, ts: &str) -> Memory {
        Memory {
            id: id.into(),
            role: "user".into(),
            content: format!("内容-{id}"),
            timestamp: ts.into(),
            tags: None,
            summary: None,
        }
    }

    #[test]
    fn merge_newest_wins() {
        let local = vec![mem("m1", "2026-08-01T00:00:00Z"), mem("m2", "2026-08-01T00:00:00Z")];
        let remote = vec![
            mem("m1", "2026-08-02T00:00:00Z"), // 更新
            mem("m3", "2026-08-01T00:00:00Z"), // 新增
        ];
        let (merged, conflicts, written) = merge_memories(&local, &remote, ConflictPolicy::Newest);
        assert_eq!(merged.len(), 3);
        assert_eq!(conflicts, 1);
        assert_eq!(written, 2);
        let m1 = merged.iter().find(|m| m.id == "m1").unwrap();
        assert_eq!(m1.timestamp, "2026-08-02T00:00:00Z");
    }

    #[test]
    fn merge_local_keeps_local() {
        let local = vec![mem("m1", "2026-08-01T00:00:00Z")];
        let remote = vec![mem("m1", "2026-08-02T00:00:00Z")];
        let (merged, conflicts, written) = merge_memories(&local, &remote, ConflictPolicy::Local);
        assert_eq!(conflicts, 1);
        assert_eq!(written, 0);
        assert_eq!(merged[0].timestamp, "2026-08-01T00:00:00Z");
    }

    #[test]
    fn merge_remote_keeps_remote() {
        let local = vec![mem("m1", "2026-08-02T00:00:00Z")];
        let remote = vec![mem("m1", "2026-08-01T00:00:00Z")];
        let (merged, conflicts, written) = merge_memories(&local, &remote, ConflictPolicy::Remote);
        assert_eq!(conflicts, 1);
        assert_eq!(written, 1);
        assert_eq!(merged[0].timestamp, "2026-08-01T00:00:00Z");
    }

    #[test]
    fn filter_incremental_works() {
        let all = vec![
            mem("m1", "2026-08-01T00:00:00Z"),
            mem("m2", "2026-08-02T00:00:00Z"),
        ];
        let inc = filter_incremental(&all, Some("2026-08-01T12:00:00Z"));
        assert_eq!(inc.len(), 1);
        assert_eq!(inc[0].id, "m2");
        assert_eq!(filter_incremental(&all, None).len(), 2);
    }
}
