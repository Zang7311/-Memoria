// 《铃·记忆体》创建记忆集命令（commands/memory_set_create.rs）
use crate::error::AppError;
use crate::memory::storage;
use crate::types::CreateMemorySetRequest;

/// 创建新记忆集，返回新集名称
#[tauri::command]
pub fn create_memory_set(req: CreateMemorySetRequest) -> Result<String, AppError> {
    storage::create_memory_set(&req.set_name)?;
    Ok(req.set_name.trim().to_string())
}
