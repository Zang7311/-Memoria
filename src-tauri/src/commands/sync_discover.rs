// 《铃·记忆体》设备发现命令（AI-8 任务 6）
use crate::error::AppError;
use crate::sync::discovery;
use crate::types::DiscoverDevicesResponse;

/// 执行 UDP 广播发现，返回设备列表（含手动添加的设备）
/// timeout_secs：扫描持续秒数（默认 3）
#[tauri::command]
pub async fn discover_devices(
    timeout_secs: Option<u64>,
) -> Result<DiscoverDevicesResponse, AppError> {
    let secs = timeout_secs.unwrap_or(3).clamp(1, 10);
    discovery::discover_devices(secs).await
}

/// 手动添加设备（UDP 被防火墙阻断时的备选连接方式）
#[tauri::command]
pub async fn add_manual_device(ip: String, port: Option<u16>) -> Result<DiscoverDevicesResponse, AppError> {
    let p = port.unwrap_or(crate::sync::DATA_TRANSFER_PORT);
    discovery::add_manual_device(&ip, p).await?;
    Ok(DiscoverDevicesResponse {
        devices: discovery::list_devices(),
    })
}
