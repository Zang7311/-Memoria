// 《铃·记忆体》更新配置（AI-7 任务 10，支持增量更新）
// v0.6：更新成功后广播 config-updated 事件，悬浮球/气泡窗口收到后重新加载配置（解决设置不同步）
use crate::config::store;
use crate::error::AppError;
use crate::types::{GetConfigResponse, UpdateConfigRequest};
use tauri::{AppHandle, Emitter};

/// 增量更新配置；返回更新后的完整配置（并广播 config-updated 通知其他窗口刷新）
#[tauri::command]
pub fn update_config(
    app: AppHandle,
    request: UpdateConfigRequest,
) -> Result<GetConfigResponse, AppError> {
    let cfg = store::update(&request.updates)?;
    let _ = app.emit("config-updated", ());
    Ok(GetConfigResponse { config: cfg })
}
