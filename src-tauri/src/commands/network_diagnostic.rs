use crate::error::AppError;

#[tauri::command]
pub async fn network_diagnostic() -> Result<String, AppError> {
    crate::engine::net::diagnose(&crate::config::store::get_config()).await
}
