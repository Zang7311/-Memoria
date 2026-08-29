// 《铃·记忆体》获取记忆列表命令（commands/memory_get.rs）
use crate::error::AppError;
use crate::memory::storage;
use crate::types::{GetMemoriesRequest, GetMemoriesResponse};

/// 获取记忆列表：支持分页（limit）、搜索（keyword）、指定记忆集（set_name）
#[tauri::command]
pub fn get_memories(req: GetMemoriesRequest) -> Result<GetMemoriesResponse, AppError> {
    let (memories, total) = storage::get_memories(
        req.set_name.as_deref(),
        req.limit,
        req.keyword.as_deref(),
    )?;
    Ok(GetMemoriesResponse { memories, total })
}
