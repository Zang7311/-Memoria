// 《铃·记忆体》主密码管理命令（AI-7 任务 2/3/11）
// 用于首次引导设置主密码、重启后解锁。
use crate::config::store;
use crate::error::AppError;
use crate::types::{MasterPasswordStatus, SetMasterPasswordRequest, UnlockRequest};

/// 设置主密码（首次引导 / 修改）：派生密钥存内存 + 持久化盐与标志
#[tauri::command]
pub fn set_master_password(request: SetMasterPasswordRequest) -> Result<(), AppError> {
    store::set_master_password(&request.password)
}

/// 解锁：输入主密码，派生密钥并验证已加密的 API Key
#[tauri::command]
pub fn unlock(request: UnlockRequest) -> Result<bool, AppError> {
    store::unlock(&request.password)
}

/// 查询主密码状态（是否设置过 + 是否已解锁）
#[tauri::command]
pub fn master_password_status() -> Result<MasterPasswordStatus, AppError> {
    Ok(store::master_password_status())
}
