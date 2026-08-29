// 《铃·记忆体》开机自启动（AI-6 任务 9）
//  - 写入注册表 HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run
//  - 键名：LingMemoria，值："<安装路径>\铃-记忆体.exe" --minimized
//  - 使用 HKCU 无需管理员权限，且按用户隔离
use crate::error::AppError;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "LingMemoria";

/// 设置开机自启动（enabled=true 写入，false 删除）
pub fn set_autostart(enabled: bool) -> Result<(), AppError> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(RUN_KEY)
        .map_err(|e| AppError::AutostartError(e.to_string()))?;

    if enabled {
        let exe = std::env::current_exe()
            .map_err(|e| AppError::AutostartError(format!("无法定位程序路径：{e}")))?;
        // 值格式："<路径>" --minimized（--minimized 由启动逻辑处理为最小化到托盘）
        let value = format!("\"{}\" --minimized", exe.display());
        key.set_value(VALUE_NAME, &value)
            .map_err(|e| AppError::AutostartError(e.to_string()))?;
        log::info!("[autostart] 已启用开机自启动：{value}");
    } else {
        match key.delete_value(VALUE_NAME) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(AppError::AutostartError(e.to_string())),
        }
        log::info!("[autostart] 已关闭开机自启动");
    }
    Ok(())
}

/// 查询开机自启动是否已启用
pub fn is_autostart_enabled() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags(RUN_KEY, KEY_READ | KEY_WRITE) {
        if let Ok(value) = key.get_value::<String, _>(VALUE_NAME) {
            return !value.is_empty();
        }
    }
    false
}
