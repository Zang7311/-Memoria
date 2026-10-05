use crate::agent::forged_tools::{self, DeleteForgedToolRequest, ForgedTool, SetForgedToolEnabledRequest};
use crate::error::AppError;

#[tauri::command]
pub fn list_forged_tools() -> Result<Vec<ForgedTool>, AppError> { forged_tools::list() }

#[tauri::command]
pub fn delete_forged_tool(request: DeleteForgedToolRequest) -> Result<(), AppError> { forged_tools::delete(&request.id) }

#[tauri::command]
pub fn set_forged_tool_enabled(request: SetForgedToolEnabledRequest) -> Result<(), AppError> {
    forged_tools::set_enabled(&request.id, request.enabled)
}
