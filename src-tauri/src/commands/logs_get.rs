// 《铃·记忆体》获取日志（AI-7 任务 7/10）
use crate::error::AppError;
use crate::logs;
use crate::types::{GetLogsRequest, GetLogsResponse};

/// 读取日志：支持级别过滤 + 关键词搜索 + 分页（offset/limit）
#[tauri::command]
pub fn get_logs(request: GetLogsRequest) -> Result<GetLogsResponse, AppError> {
    let (page, total) = logs::reader::read_logs(
        &logs::app_log_path(),
        request.level,
        request.keyword.as_deref(),
        request.offset,
        request.limit.max(1),
    );
    Ok(GetLogsResponse { logs: page, total })
}
