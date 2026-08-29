// 《铃·记忆体》AI-5 插件命令：终端命令扩展（任务 9）
// 用户自定义快捷指令（如 clean_temp → del /q %TEMP%\*），以"内置插件"形式注册，无需 JS 引擎。
use crate::error::AppError;
use crate::types::{AddTerminalCommandRequest, Plugin};

/// 添加自定义终端命令（注册为插件，执行时通过 tokio::process 运行，30s 超时）
#[tauri::command]
pub fn add_terminal_command(req: AddTerminalCommandRequest) -> Result<Plugin, AppError> {
    let plugin = crate::plugin::with_manager(|m| {
        m.add_terminal_command(&req.name, &req.command, &req.description)
    })?;
    Ok(plugin)
}
