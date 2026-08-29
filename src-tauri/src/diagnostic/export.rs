// 《铃·记忆体》诊断包导出（AI-7 任务 8）
// 收集 config.json（脱敏）+ logs/ 日志 + 系统信息，打包为 zip。
// 输出到 <data_path>/diagnostics/diagnostic_{timestamp}.zip
use crate::config::store;
use crate::diagnostic::{collect_system_info, format_system_info};
use crate::error::AppError;
use crate::types::{ExportDiagnosticRequest, ExportDiagnosticResponse};
use chrono::Local;
use serde_json::Value;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod::Deflated;

/// 执行诊断包导出
pub fn export(req: ExportDiagnosticRequest) -> Result<ExportDiagnosticResponse, AppError> {
    let cfg = store::get_config();
    let out_dir = PathBuf::from(&cfg.data_path).join("diagnostics");
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| AppError::DiagnosticExportError(format!("创建目录失败：{e}")))?;

    let ts = Local::now().format("%Y%m%d_%H%M%S");
    let zip_path = out_dir.join(format!("diagnostic_{ts}.zip"));

    let file = File::create(&zip_path)
        .map_err(|e| AppError::DiagnosticExportError(format!("创建 zip 失败：{e}")))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(Deflated);

    if req.include_config {
        zip.start_file("config.json", opts)
            .map_err(|e| AppError::DiagnosticExportError(format!("写入 config 失败：{e}")))?;
        zip.write_all(redact_config().as_bytes())
            .map_err(|e| AppError::DiagnosticExportError(format!("写入 config 内容失败：{e}")))?;
    }

    if req.include_logs {
        let logs_dir = crate::logs::logs_dir();
        if let Ok(entries) = std::fs::read_dir(&logs_dir) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if !name.ends_with(".log") {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(e.path()) {
                    let entry = format!("logs/{name}");
                    zip.start_file(&entry, opts)
                        .map_err(|e| AppError::DiagnosticExportError(format!("写入日志失败：{e}")))?;
                    zip.write_all(content.as_bytes())
                        .map_err(|e| AppError::DiagnosticExportError(format!("写入日志内容失败：{e}")))?;
                }
            }
        }
    }

    if req.include_system_info {
        let info = collect_system_info()?;
        zip.start_file("system_info.txt", opts)
            .map_err(|e| AppError::DiagnosticExportError(format!("写入系统信息失败：{e}")))?;
        zip.write_all(format_system_info(&info).as_bytes())
            .map_err(|e| AppError::DiagnosticExportError(format!("写入系统信息内容失败：{e}")))?;
    }

    zip.finish()
        .map_err(|e| AppError::DiagnosticExportError(format!("zip 封口失败：{e}")))?;

    Ok(ExportDiagnosticResponse {
        success: true,
        file_path: Some(zip_path.to_string_lossy().to_string()),
        error: None,
    })
}

/// 读取 config.json 并脱敏（隐藏 api_key_encrypted，仅保留首尾 4 位）
fn redact_config() -> String {
    let path = crate::config::config_path();
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return "{}".to_string(),
    };
    let mut value: Value = serde_json::from_str(&raw).unwrap_or(Value::Object(Default::default()));

    if let Some(obj) = value.as_object_mut() {
        if let Some(Value::String(key)) = obj.get("api_key_encrypted") {
            let masked = if key.len() <= 8 {
                "********".to_string()
            } else {
                format!("{}...{}", &key[..4], &key[key.len() - 4..])
            };
            obj.insert("api_key_encrypted".to_string(), Value::String(masked));
        }
        // 盐也打码（虽非密钥材料，一并脱敏更稳妥）
        if let Some(Value::String(salt)) = obj.get("master_password_salt") {
            let masked = format!("{}...({} 字符)", &salt[..6], salt.len());
            obj.insert("master_password_salt".to_string(), Value::String(masked));
        }
    }
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}
