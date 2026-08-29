// 《铃·记忆体》切换记忆集命令（commands/memory_set_switch.rs）
use crate::error::AppError;
use crate::memory::storage;
use crate::types::SwitchMemorySetRequest;

/// 切换记忆集，返回当前集名称
#[tauri::command]
pub fn switch_memory_set(req: SwitchMemorySetRequest) -> Result<String, AppError> {
    storage::switch_memory_set(&req.set_name)
}
