// 《铃·记忆体》多会话命令（收尾工程师批次3）
use crate::error::AppError;
use crate::types::{Message, Session, SessionMeta};

/// 列出所有会话元信息（按更新时间倒序）
#[tauri::command]
pub fn session_list() -> Result<Vec<SessionMeta>, AppError> {
    crate::sessions::storage::list_sessions()
}

/// 新建会话（返回空会话）
#[tauri::command]
pub fn session_create() -> Result<Session, AppError> {
    crate::sessions::storage::create_session()
}

/// 加载单个会话（含完整消息）
#[tauri::command]
pub fn session_load(id: String) -> Result<Session, AppError> {
    crate::sessions::storage::load_session(&id)
}

/// 保存会话消息（更新标题/计数/时间；不存在则自动新建）
#[tauri::command]
pub fn session_save(id: String, messages: Vec<Message>) -> Result<Session, AppError> {
    crate::sessions::storage::save_session(&id, &messages)
}

/// 重命名会话
#[tauri::command]
pub fn session_rename(id: String, title: String) -> Result<SessionMeta, AppError> {
    crate::sessions::storage::rename_session(&id, &title)
}

/// 删除会话
#[tauri::command]
pub fn session_delete(id: String) -> Result<(), AppError> {
    crate::sessions::storage::delete_session(&id)
}
