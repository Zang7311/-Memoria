// 《铃·记忆体》重置配置（AI-7 任务 10）
use crate::config::store;
use crate::error::AppError;
use crate::types::GetConfigResponse;

/// 重置所有配置为默认值（保留主密码与已加密 API Key，避免误清凭据）
#[tauri::command]
pub fn reset_config() -> Result<GetConfigResponse, AppError> {
    let cfg = store::reset()?;
    Ok(GetConfigResponse { config: cfg })
}
