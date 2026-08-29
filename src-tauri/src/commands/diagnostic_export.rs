// 《铃·记忆体》导出诊断包（AI-7 任务 8/10）
use crate::diagnostic::export;
use crate::error::AppError;
use crate::types::{ExportDiagnosticRequest, ExportDiagnosticResponse};

/// 导出诊断包：打包（脱敏配置 + 日志 + 系统信息）为 zip
#[tauri::command]
pub fn export_diagnostic(
    request: ExportDiagnosticRequest,
) -> Result<ExportDiagnosticResponse, AppError> {
    export::export(request)
}
