// 《铃·记忆体》同步状态 / 密码命令（AI-8 任务 6/9）
use crate::config::store as config_store;
use crate::error::AppError;
use crate::sync::conflict;
use crate::types::{
    DiscoverDevicesResponse, SetSyncPasswordRequest, SyncStatus,
};

/// 获取当前同步状态（idle/discovering/syncing/done/error + 进度 + 历史）
#[tauri::command]
pub fn get_sync_status() -> Result<SyncStatus, AppError> {
    Ok(conflict::get_sync_status())
}

/// 设置/修改同步主密码（与 AI-7 共享同一主密码体系）
#[tauri::command]
pub fn set_sync_password(request: SetSyncPasswordRequest) -> Result<(), AppError> {
    config_store::set_master_password(&request.password)
}

/// 设置冲突解决策略（newest / local / remote）
#[tauri::command]
pub fn set_conflict_policy(policy: String) -> Result<(), AppError> {
    let p = match policy.as_str() {
        "local" => crate::types::ConflictPolicy::Local,
        "remote" => crate::types::ConflictPolicy::Remote,
        _ => crate::types::ConflictPolicy::Newest,
    };
    conflict::set_conflict_policy(p)
}

/// 获取当前设备列表（不扫描，直接读缓存）
#[tauri::command]
pub fn get_sync_devices() -> Result<DiscoverDevicesResponse, AppError> {
    Ok(DiscoverDevicesResponse {
        devices: crate::sync::discovery::list_devices(),
    })
}
