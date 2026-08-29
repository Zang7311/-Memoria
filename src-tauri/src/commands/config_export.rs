// 《铃·记忆体》导出配置（AI-7 任务 10）
// 将当前完整配置导出为 JSON 文件到 <data_path>/config_export_{ts}.json
use crate::config::store;
use crate::error::AppError;
use crate::types::ExportConfigResponse;
use chrono::Local;
use std::path::PathBuf;

#[tauri::command]
pub fn export_config() -> Result<ExportConfigResponse, AppError> {
    let cfg = store::get_config();
    let out_dir = PathBuf::from(&cfg.data_path);
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| AppError::ConfigSaveError(format!("创建导出目录失败：{e}")))?;

    let ts = Local::now().format("%Y%m%d_%H%M%S");
    let path = out_dir.join(format!("config_export_{ts}.json"));

    let s = serde_json::to_string_pretty(&cfg)
        .map_err(|e| AppError::ConfigSaveError(format!("序列化失败：{e}")))?;
    std::fs::write(&path, s)
        .map_err(|e| AppError::ConfigSaveError(format!("写入失败：{e}")))?;

    Ok(ExportConfigResponse {
        success: true,
        path: Some(path.to_string_lossy().to_string()),
        error: None,
    })
}
