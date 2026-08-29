// 《铃·记忆体》工具箱管理（AI-6 任务 7）
//  - 预设命令从 resources/toolbox_presets.json 加载（随应用分发）
//  - 用户自定义命令存 %APPDATA%/ling-memoria/toolbox_items.json
//  - 执行使用 tokio::process 异步，30 秒超时强制终止
use crate::error::AppError;
use crate::types::{ExecuteToolboxResponse, ToolboxItem};
use std::time::Duration;

/// 合并返回工具箱条目：预设 + 用户自定义（用户条目 id 以 "user_" 前缀）
pub fn list_items(resource_dir: &std::path::Path) -> Vec<ToolboxItem> {
    let mut items = load_presets(resource_dir);
    items.extend(load_user_items());
    items
}

/// 加载预设命令（编译期内嵌，保证绿色版/安装版都能加载，不依赖运行时 resource_dir）
fn load_presets(_resource_dir: &std::path::Path) -> Vec<ToolboxItem> {
    match serde_json::from_str::<Vec<ToolboxItem>>(
        include_str!("../../resources/toolbox_presets.json"),
    ) {
        Ok(items) => items,
        Err(e) => {
            log::warn!("[toolbox] 解析内嵌预设失败：{e}");
            Vec::new()
        }
    }
}

/// 加载用户自定义条目
fn load_user_items() -> Vec<ToolboxItem> {
    let path = crate::desktop::toolbox_items_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str::<Vec<ToolboxItem>>(&s).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// 保存用户自定义条目
fn save_user_items(items: &[ToolboxItem]) {
    let path = crate::desktop::toolbox_items_path();
    match serde_json::to_string_pretty(items) {
        Ok(s) => {
            if let Err(e) = std::fs::write(&path, s) {
                log::warn!("[toolbox] 保存用户条目失败：{e}");
            }
        }
        Err(e) => log::warn!("[toolbox] 序列化失败：{e}"),
    }
}

/// 按 id 查找条目（预设 + 用户）
pub fn find_item(resource_dir: &std::path::Path, item_id: &str) -> Option<ToolboxItem> {
    list_items(resource_dir).into_iter().find(|i| i.id == item_id)
}

/// 保存（新增或更新）用户自定义条目
pub fn save_user_item(item: ToolboxItem) -> Result<(), AppError> {
    let mut items = load_user_items();
    if let Some(existing) = items.iter_mut().find(|i| i.id == item.id) {
        *existing = item;
    } else {
        items.push(item);
    }
    save_user_items(&items);
    Ok(())
}

/// 删除用户自定义条目
pub fn delete_user_item(item_id: &str) -> Result<(), AppError> {
    let mut items = load_user_items();
    items.retain(|i| i.id != item_id);
    save_user_items(&items);
    Ok(())
}

/// 异步执行工具命令（cmd /C），30 秒超时强制终止（kill_on_drop）
/// Windows：隐藏控制台窗口（CREATE_NO_WINDOW），避免执行时弹出黑色终端；
/// 命令输出仍通过 stdout 捕获并返回前端反馈。
pub async fn execute(item: &ToolboxItem) -> Result<ExecuteToolboxResponse, AppError> {
    let mut cmd = tokio::process::Command::new("cmd");
    cmd.arg("/C");
    // 超时 drop 命令时强制杀死子进程，避免残留
    cmd.kill_on_drop(true);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW 隐藏控制台
        // raw_arg 原样传命令，避免 .arg() 对含空格/引号的命令自动加引号导致 cmd 解析失败
        cmd.raw_arg(&item.command);
    }
    #[cfg(not(windows))]
    {
        cmd.arg(&item.command);
    }

    let result = tokio::time::timeout(Duration::from_secs(30), cmd.output()).await;
    match result {
        Err(_) => Err(AppError::ToolboxTimeout(item.name.clone())),
        Ok(Err(e)) => Err(AppError::ToolboxError(e.to_string())),
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let success = output.status.success();
            if success {
                let text = if !stdout.is_empty() { stdout } else { stderr };
                Ok(ExecuteToolboxResponse {
                    success: true,
                    output: if text.is_empty() { None } else { Some(text) },
                    error: None,
                })
            } else {
                // 失败时把原因放进 error（stderr 优先，否则 stdout，否则退出码），前端据此显示
                let err = if !stderr.is_empty() {
                    stderr
                } else if !stdout.is_empty() {
                    stdout
                } else {
                    format!("命令执行失败（退出码 {:?}）", output.status.code())
                };
                Ok(ExecuteToolboxResponse {
                    success: false,
                    output: None,
                    error: Some(err),
                })
            }
        }
    }
}
