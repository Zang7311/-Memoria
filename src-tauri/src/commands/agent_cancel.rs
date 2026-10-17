// agent_cancel：将指定任务标记为「用户已取消」
//
// 前端调用：invoke('agent_cancel', { request_id: '...' })
// 后端检查点（loop_.rs）会在下一个检查点退出循环，返回 Ok + interrupted: true。
//
// 若该任务正卡在「计划确认」上，这里同时把等待中的计划叫醒并置为取消，
// 否则用户点了停止还要干等 120 秒超时 —— 那是不可接受的。
use crate::agent::cancel;
use crate::agent::plan_gate::{self, PlanReply};
use crate::error::AppError;

#[tauri::command]
pub async fn agent_cancel(request_id: String) -> Result<(), AppError> {
    cancel::set_cancelled(&request_id);
    plan_gate::settle(&request_id, PlanReply::Cancel);
    Ok(())
}
