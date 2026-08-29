// 《铃·记忆体》网络状态命令（AI-8 任务 8）
use crate::error::AppError;
use crate::network::monitor;
use crate::types::GetNetworkStatusResponse;

/// 获取当前网络状态（online / offline / unknown）
#[tauri::command]
pub fn get_network_status() -> Result<GetNetworkStatusResponse, AppError> {
    Ok(GetNetworkStatusResponse {
        status: monitor::get_network_status(),
    })
}
