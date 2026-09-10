// 《铃·记忆体》获取配置（AI-7 任务 10）
use crate::config::store;
use crate::error::AppError;
use crate::types::GetConfigResponse;

/// 返回配置（已脱敏：不含 api_key_plain 明文与 api_key_encrypted 密文，
/// 改为 has_api_key / has_plain_key 两个布尔，前端只需知道"有没有"）
#[tauri::command]
pub fn get_config() -> Result<GetConfigResponse, AppError> {
    GetConfigResponse::from_config(&store::get_config())
}
