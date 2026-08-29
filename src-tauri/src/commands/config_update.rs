// 《铃·记忆体》更新配置（AI-7 任务 10，支持增量更新）
use crate::config::store;
use crate::error::AppError;
use crate::types::{GetConfigResponse, UpdateConfigRequest};

/// 增量更新配置；返回更新后的完整配置
#[tauri::command]
pub fn update_config(request: UpdateConfigRequest) -> Result<GetConfigResponse, AppError> {
    let cfg = store::update(&request.updates)?;
    Ok(GetConfigResponse { config: cfg })
}
