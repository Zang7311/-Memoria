// 《铃·记忆体》写入记忆命令（commands/memory_write.rs）
use crate::error::AppError;
use crate::memory::storage;
use crate::types::WriteMemoryRequest;

/// 写入单条记忆（供对话引擎或前端调用）
#[tauri::command]
pub fn write_memory(req: WriteMemoryRequest) -> Result<(), AppError> {
    storage::write_memory(req.memory, req.set_name.as_deref())
}
