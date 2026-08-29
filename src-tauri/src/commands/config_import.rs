// 《铃·记忆体》导入配置（AI-7 任务 10）
// 从 JSON 文件导入完整配置（会覆盖当前配置）。
use crate::config::{migration, store};
use crate::error::AppError;
use crate::types::{GetConfigResponse, ImportConfigRequest};

#[tauri::command]
pub fn import_config(request: ImportConfigRequest) -> Result<GetConfigResponse, AppError> {
    let s = std::fs::read_to_string(&request.path)
        .map_err(|e| AppError::ConfigImportError(format!("读取 {} 失败：{e}", request.path)))?;
    let raw: serde_json::Value = serde_json::from_str(&s)
        .map_err(|e| AppError::ConfigImportError(format!("JSON 解析失败：{e}")))?;
    let cfg = migration::migrate(raw)?;
    store::set_config(cfg)?;
    Ok(GetConfigResponse {
        config: store::get_config(),
    })
}
