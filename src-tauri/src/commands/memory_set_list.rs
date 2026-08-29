// 《铃·记忆体》列出记忆集命令（commands/memory_set_list.rs）
use crate::error::AppError;
use crate::memory::storage;

/// 列出所有记忆集名称（含 default）
#[tauri::command]
pub fn list_memory_sets() -> Result<Vec<String>, AppError> {
    storage::list_memory_sets()
}
