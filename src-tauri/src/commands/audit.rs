use crate::audit::{self, AuditEntry, AuditQuery};
use crate::error::AppError;

#[tauri::command]
pub async fn list_audit(query: AuditQuery) -> Result<Vec<AuditEntry>, AppError> {
    tauri::async_runtime::spawn_blocking(move || audit::store().list(&query))
        .await
        .map_err(|_| AppError::InternalError("执行记录读取任务失败".into()))?
}

#[tauri::command]
pub async fn export_audit(query: AuditQuery) -> Result<String, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        serde_json::to_string_pretty(&audit::store().list(&query)?)
            .map_err(|_| AppError::InternalError("执行记录导出失败".into()))
    })
    .await
    .map_err(|_| AppError::InternalError("执行记录导出任务失败".into()))?
}

#[tauri::command]
pub async fn clear_audit(confirmed: bool) -> Result<(), AppError> {
    if !confirmed {
        return Err(AppError::PermissionDenied(
            "请明确确认清空全部执行记录，此操作不可恢复".into(),
        ));
    }
    tauri::async_runtime::spawn_blocking(move || audit::store().clear())
        .await
        .map_err(|_| AppError::InternalError("执行记录清空任务失败".into()))?
}
