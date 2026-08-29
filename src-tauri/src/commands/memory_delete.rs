// 《铃·记忆体》删除记忆命令（commands/memory_delete.rs）
use crate::error::AppError;
use crate::memory::storage;
use crate::types::{DeleteMemoryRequest, DeleteMemoryResponse};

/// 删除单条记忆
#[tauri::command]
pub fn delete_memory(req: DeleteMemoryRequest) -> Result<DeleteMemoryResponse, AppError> {
    storage::delete_memory(&req.memory_id, req.set_name.as_deref())?;
    Ok(DeleteMemoryResponse {
        success: true,
        message: format!("记忆 {} 已删除", req.memory_id),
    })
}
