// 《铃·记忆体》多会话存储（收尾工程师批次3）
// 会话文件存 <数据路径>/sessions/<id>.json，每个会话一个文件（meta + messages）。
// 与记忆同根目录，统一由设置页「数据路径」管理。
use crate::error::AppError;
use crate::types::{Message, Session, SessionMeta};
use std::path::PathBuf;

/// 会话目录（与记忆同根，位于用户数据路径下）
pub fn sessions_dir() -> PathBuf {
    let cfg = crate::config::store::get_config();
    let base = cfg.data_path.trim();
    let root = if base.is_empty() {
        std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string())
    } else {
        base.to_string()
    };
    PathBuf::from(root).join("sessions")
}

fn session_path(id: &str) -> PathBuf {
    sessions_dir().join(format!("{id}.json"))
}

/// 生成会话 id（复用工具函数，保证唯一）
fn gen_session_id() -> String {
    crate::utils::gen_id()
}

/// 从会话内容生成标题（取第一条用户消息，去重行、截 20 字）
fn auto_title(messages: &[Message]) -> Option<String> {
    messages
        .iter()
        .find(|m| m.role == "user")
        .map(|m| {
            let raw = m.content.trim().lines().next().unwrap_or("").trim();
            let raw = if raw.chars().count() > 20 {
                format!("{}…", raw.chars().take(20).collect::<String>())
            } else {
                raw.to_string()
            };
            if raw.is_empty() { "新会话".to_string() } else { raw }
        })
}

/// 列出所有会话元信息（按更新时间倒序）
pub fn list_sessions() -> Result<Vec<SessionMeta>, AppError> {
    let dir = sessions_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut metas = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json")
            && !path.to_string_lossy().ends_with(".json.tmp")
        {
            if let Ok(s) = load_session_file(&path) {
                metas.push(s.meta);
            }
        }
    }
    metas.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(metas)
}

fn load_session_file(path: &PathBuf) -> Result<Session, AppError> {
    let raw = std::fs::read_to_string(path)?;
    serde_json::from_str(&raw)
        .map_err(|e| AppError::MemoryError(format!("会话文件解析失败：{e}")))
}

/// 加载单个会话（含完整消息）
pub fn load_session(id: &str) -> Result<Session, AppError> {
    let path = session_path(id);
    if !path.exists() {
        return Err(AppError::MemoryNotFound(id.to_string()));
    }
    load_session_file(&path)
}

/// 新建会话（返回空会话，尚未持久化也可立即用；这里直接落盘）
pub fn create_session() -> Result<Session, AppError> {
    let now = crate::utils::now_str();
    let id = gen_session_id();
    let meta = SessionMeta {
        id: id.clone(),
        title: "新会话".to_string(),
        created_at: now.clone(),
        updated_at: now,
        message_count: 0,
    };
    let s = Session { meta, messages: Vec::new() };
    save_session_file(&s)?;
    log::info!("[session] 新建会话 {}", s.meta.id);
    Ok(s)
}

fn save_session_file(s: &Session) -> Result<(), AppError> {
    let dir = sessions_dir();
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(s)?;
    let path = dir.join(format!("{}.json", s.meta.id));
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// 保存会话消息（更新消息、消息数、标题、时间；不存在则自动新建）
pub fn save_session(id: &str, messages: &[Message]) -> Result<Session, AppError> {
    let mut s = if let Ok(existing) = load_session(id) {
        existing
    } else {
        let now = crate::utils::now_str();
        Session {
            meta: SessionMeta {
                id: id.to_string(),
                title: "新会话".to_string(),
                created_at: now.clone(),
                updated_at: now,
                message_count: 0,
            },
            messages: Vec::new(),
        }
    };
    s.messages = messages.to_vec();
    if let Some(t) = auto_title(messages) {
        if s.meta.title == "新会话" || s.meta.message_count == 0 {
            s.meta.title = t;
        }
    }
    s.meta.message_count = messages.len();
    s.meta.updated_at = crate::utils::now_str();
    save_session_file(&s)?;
    Ok(s)
}

/// 重命名会话
pub fn rename_session(id: &str, title: &str) -> Result<SessionMeta, AppError> {
    let mut s = load_session(id)?;
    let t = title.trim();
    if !t.is_empty() {
        s.meta.title = t.to_string();
    }
    s.meta.updated_at = crate::utils::now_str();
    save_session_file(&s)?;
    Ok(s.meta)
}

/// 删除会话
pub fn delete_session(id: &str) -> Result<(), AppError> {
    let path = session_path(id);
    if !path.exists() {
        return Err(AppError::MemoryNotFound(id.to_string()));
    }
    std::fs::remove_file(path)?;
    log::info!("[session] 删除会话 {id}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, content: &str) -> Message {
        Message {
            id: crate::utils::gen_id(),
            role: role.to_string(),
            content: content.to_string(),
            timestamp: crate::utils::now_str(),
        }
    }

    #[test]
    fn auto_title_uses_first_user_message() {
        let msgs = vec![
            msg("user", "  你好，铃\n第二行"),
            msg("assistant", "同学你好呀"),
        ];
        let t = auto_title(&msgs).unwrap();
        assert!(t.starts_with("你好，铃"));
    }

    #[test]
    fn auto_title_truncates_long() {
        let long = "很".repeat(50);
        let msgs = vec![msg("user", &long)];
        let t = auto_title(&msgs).unwrap();
        assert!(t.chars().count() <= 21); // 20 字 + …
    }
}
