// 《铃·记忆体》更新检查命令（AI-8 任务 7）
use crate::error::AppError;
use crate::types::CheckUpdateResponse;
use crate::update::checker;

/// 检查 GitHub Releases 是否有新版本（失败静默降级）
#[tauri::command]
pub async fn check_update(force: Option<bool>) -> Result<CheckUpdateResponse, AppError> {
    checker::check_update(force.unwrap_or(false)).await
}
