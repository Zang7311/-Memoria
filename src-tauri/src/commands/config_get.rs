// 《铃·记忆体》获取配置（AI-7 任务 10）
use crate::config::store;
use crate::error::AppError;
use crate::types::GetConfigResponse;

/// 返回配置（已脱敏：不含主力与便宜模型的密钥明文或密文，
/// 改为 has_api_key / has_plain_key / has_cheap_api_key 布尔，前端只需知道"有没有"）
#[tauri::command]
pub fn get_config() -> Result<GetConfigResponse, AppError> {
    GetConfigResponse::from_config(&store::get_config())
}
