// 《铃·记忆体》获取配置（AI-7 任务 10）
use crate::config::store;
use crate::error::AppError;
use crate::types::GetConfigResponse;

/// 返回完整配置
#[tauri::command]
pub fn get_config() -> Result<GetConfigResponse, AppError> {
    Ok(GetConfigResponse {
        config: store::get_config(),
    })
}
