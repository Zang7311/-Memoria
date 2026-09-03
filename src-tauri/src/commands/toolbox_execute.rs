// 《铃·记忆体》IPC：工具箱（AI-6 任务 10）
//  - list_toolbox_items：列出全部条目（预设 + 用户自定义）
//  - execute_toolbox：执行指定命令（tokio 异步 + 30s 超时）
//  - save_toolbox_item：新增/更新用户自定义条目
//  - delete_toolbox_item：删除用户自定义条目
use crate::desktop::toolbox;
use crate::error::AppError;
use crate::types::{
    DeleteToolboxItemRequest, ExecuteToolboxRequest, ExecuteToolboxResponse,
    ListToolboxItemsResponse, SaveToolboxItemRequest,
};
use tauri::{AppHandle, Manager};

/// 获取资源目录（开发模式 = src-tauri，打包模式 = 安装目录 resources）
fn resource_dir(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .resource_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
}

/// 列出工具箱条目（预设 + 用户自定义）
#[tauri::command]
pub fn list_toolbox_items(app: AppHandle) -> ListToolboxItemsResponse {
    ListToolboxItemsResponse {
        items: toolbox::list_items(&resource_dir(&app)),
    }
}

/// 执行工具箱命令（异步，30 秒超时）
#[tauri::command]
pub async fn execute_toolbox(
    app: AppHandle,
    request: ExecuteToolboxRequest,
) -> Result<ExecuteToolboxResponse, AppError> {
    // H-7：format-disk 硬编码二次确认 + 盘符严格校验
    if request.item_id == "format-disk" {
        if !request.confirm {
            return Err(AppError::ToolboxError(
                "格式化磁盘为不可逆危险操作，必须传入 confirm=true 才能执行".into(),
            ));
        }
        // 盘符必须为单个字母 A-Z（不区分大小写）
        let drive = request.input.as_deref().unwrap_or("").trim();
        let valid = drive.len() == 1 && drive.chars().next().map_or(false, |c| c.is_ascii_alphabetic());
        if !valid {
            return Err(AppError::ToolboxError(
                "盘符必须是单个英文字母（A-Z），格式化中止".into(),
            ));
        }
    }

    match toolbox::find_item(&resource_dir(&app), &request.item_id) {
        Some(item) => {
            // 组合工具（steps 非空）：按顺序执行各步骤，结果合并返回（复用快捷指令步骤执行器）
            if !item.steps.is_empty() {
                let mut outs: Vec<String> = Vec::with_capacity(item.steps.len());
                for step in &item.steps {
                    outs.push(crate::commands::quick_command::execute_step(&app, step).await);
                }
                Ok(ExecuteToolboxResponse {
                    success: true,
                    output: Some(outs.join("；")),
                    error: None,
                })
            } else {
                toolbox::execute(&item, request.input).await
            }
        }
        None => Err(AppError::ToolboxError(format!(
            "工具箱条目不存在：{}",
            request.item_id
        ))),
    }
}

/// 新增/更新用户自定义工具箱条目
#[tauri::command]
pub fn save_toolbox_item(request: SaveToolboxItemRequest) -> Result<(), AppError> {
    toolbox::save_user_item(request.item)
}

/// 删除用户自定义工具箱条目
#[tauri::command]
pub fn delete_toolbox_item(request: DeleteToolboxItemRequest) -> Result<(), AppError> {
    toolbox::delete_user_item(&request.item_id)
}
