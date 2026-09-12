// agent_cancel：将指定任务标记为「用户已取消」
//
// 前端调用：invoke('agent_cancel', { request_id: '...' })
// 后端检查点（loop_.rs）会在下一个检查点退出循环，返回 Ok + interrupted: true。
use crate::agent::cancel;
use crate::error::AppError;

#[tauri::command]
pub async fn agent_cancel(request_id: String) -> Result<(), AppError> {
    cancel::set_cancelled(&request_id);
    Ok(())
}
