// 《铃·记忆体》启动同步命令（AI-8 任务 6）
use crate::error::AppError;
use crate::sync::transfer;
use crate::types::{StartSyncRequest, StartSyncResponse};
use tauri::AppHandle;

/// 发起 TCP 同步（支持 manual_ip / manual_port 备选）
#[tauri::command]
pub async fn start_sync(
    app: AppHandle,
    request: StartSyncRequest,
) -> Result<StartSyncResponse, AppError> {
    transfer::start_sync(request, app).await
}
