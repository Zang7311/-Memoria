// 《铃·记忆体》标记记忆重要命令（commands/memory_mark.rs）
use crate::error::AppError;
use crate::memory::storage;
use crate::types::Memory;

/// 标记记忆为重要（加 tags:["important"]），返回更新后的记忆
#[tauri::command]
pub fn mark_memory_important(
    memory_id: String,
    set_name: Option<String>,
) -> Result<Memory, AppError> {
    storage::mark_important(&memory_id, set_name.as_deref())
}
