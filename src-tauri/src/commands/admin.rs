// 《铃·记忆体》管理员权限命令（收尾工程师）
use crate::error::AppError;

/// 检测当前是否以管理员权限运行
/// Windows：`net session` 需管理员，非管理员返回"拒绝访问"（非 0 退出码）
#[tauri::command]
pub fn is_admin() -> bool {
    #[cfg(windows)]
    {
        let mut cmd = std::process::Command::new("net");
        cmd.args(["session"]);
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW 隐藏控制台
        cmd.output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// 以管理员权限重启应用（弹 UAC 提示，确认后以 runas 启动新实例）
/// 使用 current_exe() 动态获取程序路径，打包后地址不固定也能正确找到
#[tauri::command]
pub fn restart_as_admin() -> Result<(), AppError> {
    let exe = std::env::current_exe()
        .map_err(|e| AppError::InternalError(format!("获取程序路径失败：{e}")))?;
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-WindowStyle", "Hidden", "-Command"])
        .arg(format!("Start-Process -FilePath '{}' -Verb RunAs", exe.display()));
    // 隐藏控制台窗口
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd.spawn()
        .map_err(|e| AppError::InternalError(format!("启动管理员权限失败：{e}")))?;
    Ok(())
}
