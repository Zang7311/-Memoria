// 《铃·记忆体》流式事件推送封装
// 统一封装 chat_chunk / chat_end / chat_error 三个事件
// 注意：前端 listen<string>('chat_chunk') 只收裸字符串，故 payload 直接是片段字符串
use tauri::{AppHandle, Emitter};
use crate::error::AppError;

/// 推送一段回复内容（每次 2~5 字）
pub fn send_chunk(app: &AppHandle, chunk: &str) -> Result<(), AppError> {
    app.emit("chat_chunk", chunk)?;
    Ok(())
}

/// 推送流式结束信号
pub fn send_end(app: &AppHandle) -> Result<(), AppError> {
    app.emit("chat_end", ())?;
    Ok(())
}

/// 推送流式错误（前端显示对应提示）
pub fn send_error(app: &AppHandle, error: &str) -> Result<(), AppError> {
    app.emit("chat_error", error)?;
    Ok(())
}
