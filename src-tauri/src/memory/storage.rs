// 《铃·记忆体》记忆存储核心（AI-4 扩展）
// 设计原则：与 AI-3 对话引擎无缝配合——
//   · 沿用 AI-3 已定稿的路径 %APPDATA%/ling-memoria/memory/index.json（default 集）
//   · 沿用 AI-3 已定稿的全局写锁 crate::context::MEMORY_WRITER_LOCK
//   · 数据模型沿用 index.json 存 Memory[]（不引入独立文件/MemoryMeta，避免推翻 AI-3）
// 在此之上扩展：多记忆集（子文件夹）、增删改查、搜索、压缩、索引重建。
use crate::context::MEMORY_WRITER_LOCK;
use crate::error::AppError;
use crate::types::Memory;
use std::path::PathBuf;

/// 默认记忆集名称（即 AI-3 现有的单集）
pub const DEFAULT_SET: &str = "default";

/// 获取记忆根目录（所有记忆集的父目录）
/// 优先使用配置中心的自定义数据路径（设置页「数据路径」可改，收尾工程师修正：真正生效）；
/// 未设置时回退 %APPDATA%/ling-memoria/memory。
pub fn root_dir() -> PathBuf {
    let cfg = crate::config::store::get_config();
    let custom = cfg.data_path.trim();
    if !custom.is_empty() {
        return PathBuf::from(custom);
    }
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join("ling-memoria").join("memory")
}

/// 获取 index.json 的默认路径（与 AI-3 约定一致，保留给 AI-3 使用）
pub fn default_index_path() -> PathBuf {
    root_dir().join("index.json")
}

/// 获取指定记忆集的 index.json 路径
/// - None 或 "default" → 根目录下的 index.json（AI-3 的单集）
/// - 其他 set_name   → 根目录下子文件夹 set_name/index.json
pub fn set_index_path(set_name: Option<&str>) -> PathBuf {
    match set_name {
        None | Some(DEFAULT_SET) => default_index_path(),
        Some(name) => root_dir().join(name).join("index.json"),
    }
}

/// 读取全部记忆（指定记忆集），文件不存在视为空，不报错
/// 索引损坏时自动重建（备份旧文件后重置为空），不丢现场
pub fn read_all(index_path: &PathBuf) -> Result<Vec<Memory>, AppError> {
    match crate::memory::index::load_index(index_path) {
        Ok(m) => Ok(m),
        Err(AppError::IndexCorrupted(_)) => {
            crate::memory::index::rebuild_index(index_path)?;
            Ok(Vec::new())
        }
        Err(e) => Err(e),
    }
}

/// 原子写入 Memory[] 到 index.json（先写临时文件再重命名，防中途崩溃损坏索引）
/// 调用方必须已持有 MEMORY_WRITER_LOCK
pub(crate) fn atomic_write_index(index_path: &PathBuf, memories: &[Memory]) -> Result<(), AppError> {
    if let Some(parent) = index_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(memories)?;
    let tmp = index_path.with_extension("json.tmp");
    std::fs::write(&tmp, &json)?;
    std::fs::rename(&tmp, index_path)?;
    Ok(())
}

/// 公开整表写入（记忆中心批量操作用，内部带写锁）
pub fn write_all(index_path: &PathBuf, memories: &[Memory]) -> Result<(), AppError> {
    let _guard = MEMORY_WRITER_LOCK
        .lock()
        .map_err(|_| AppError::MemoryError("记忆写锁获取失败".into()))?;
    atomic_write_index(index_path, memories)
}

/// 追加一条记忆（带全局锁，去重），返回写入后的总条数
/// 供 AI-3 及对话引擎调用（保持原签名不变）
pub fn append_memory(index_path: &PathBuf, memory: &Memory) -> Result<usize, AppError> {
    let _guard = MEMORY_WRITER_LOCK
        .lock()
        .map_err(|_| AppError::MemoryError("记忆写锁获取失败".into()))?;

    let mut memories = read_all(index_path)?;
    if memories.iter().any(|m| m.id == memory.id) {
        return Ok(memories.len());
    }
    memories.push(memory.clone());
    atomic_write_index(index_path, &memories)?;
    log::info!("记忆写入：id={} role={} 当前共 {} 条", memory.id, memory.role, memories.len());
    Ok(memories.len())
}

/// 写入一条记忆到指定记忆集（带锁、去重、自动压缩检查）
pub fn write_memory(memory: Memory, set_name: Option<&str>) -> Result<(), AppError> {
    let path = set_index_path(set_name);
    append_memory(&path, &memory)?;
    // 超过阈值触发自动压缩（默认 20 条）
    crate::memory::compress::maybe_compress(&path, set_name)?;
    Ok(())
}

/// 删除单条记忆（指定记忆集），返回删除是否成功
pub fn delete_memory(memory_id: &str, set_name: Option<&str>) -> Result<(), AppError> {
    let path = set_index_path(set_name);
    let _guard = MEMORY_WRITER_LOCK
        .lock()
        .map_err(|_| AppError::MemoryError("记忆写锁获取失败".into()))?;

    let mut memories = read_all(&path)?;
    let before = memories.len();
    memories.retain(|m| m.id != memory_id);
    if memories.len() == before {
        return Err(AppError::MemoryNotFound(memory_id.to_string()));
    }
    atomic_write_index(&path, &memories)?;
    log::info!("记忆删除：id={} 当前共 {} 条", memory_id, memories.len());
    Ok(())
}

/// 获取记忆列表（按 set_name、limit 分页、keyword 搜索）
pub fn get_memories(
    set_name: Option<&str>,
    limit: Option<usize>,
    keyword: Option<&str>,
) -> Result<(Vec<Memory>, usize), AppError> {
    let path = set_index_path(set_name);
    let all = read_all(&path)?;

    // 搜索过滤（读取配置中的 search_mode，默认 bigram）
    let mode = crate::config::store::get_config().search_mode;
    let mut list = match keyword {
        Some(kw) if !kw.trim().is_empty() => {
            crate::memory::search::search_with_mode(&all, kw, &mode)
        }
        _ => all,
    };

    let total = list.len();
    // 分页：取最新的 limit 条（时间戳倒序取最近，再恢复顺序）
    if let Some(n) = limit {
        if list.len() > n {
            list = list.into_iter().rev().take(n).collect();
            list.reverse();
        }
    }
    Ok((list, total))
}

/// 列出所有记忆集（读取根目录下含 index.json 的子文件夹）
pub fn list_memory_sets() -> Result<Vec<String>, AppError> {
    let root = root_dir();
    let mut sets = vec![DEFAULT_SET.to_string()];
    if root.exists() {
        for entry in std::fs::read_dir(&root)? {
            let entry = entry?;
            if entry.path().is_dir()
                && entry.path().join("index.json").exists()
            {
                if let Some(name) = entry.file_name().to_str() {
                    if name != DEFAULT_SET {
                        sets.push(name.to_string());
                    }
                }
            }
        }
    }
    Ok(sets)
}

/// 创建新记忆集（在根目录下新建子文件夹 + 初始化空 index.json）
pub fn create_memory_set(set_name: &str) -> Result<(), AppError> {
    let name = set_name.trim();
    if name.is_empty() {
        return Err(AppError::MemorySetNotFound("记忆集名称不能为空".into()));
    }
    if name == DEFAULT_SET {
        return Err(AppError::MemorySetAlreadyExists(name.to_string()));
    }
    // 防路径穿越
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err(AppError::ConfigError("记忆集名称非法".into()));
    }
    let path = set_index_path(Some(name));
    if path.exists() {
        return Err(AppError::MemorySetAlreadyExists(name.to_string()));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    atomic_write_index(&path, &[])?;
    Ok(())
}

/// 切换记忆集：验证目标集存在（default 恒存在），返回规范化后的集名
pub fn switch_memory_set(set_name: &str) -> Result<String, AppError> {
    let name = set_name.trim();
    if name.is_empty() {
        return Err(AppError::MemorySetNotFound("记忆集名称不能为空".into()));
    }
    if name == DEFAULT_SET {
        return Ok(DEFAULT_SET.to_string());
    }
    let path = set_index_path(Some(name));
    if !path.exists() {
        return Err(AppError::MemorySetNotFound(name.to_string()));
    }
    Ok(name.to_string())
}

/// 写入一条用户消息记忆（快捷方式，保留给 AI-3）
pub fn save_user_message(index_path: &PathBuf, id: &str, content: &str) -> Result<usize, AppError> {
    let mem = Memory {
        id: id.to_string(),
        role: "user".to_string(),
        content: content.to_string(),
        timestamp: crate::utils::now_str(),
        tags: None,
        summary: None,
        category: Some(crate::memory::category::classify(content)),
        use_count: 0,
    };
    append_memory(index_path, &mem)
}

/// 写入一条助手回复记忆（快捷方式，保留给 AI-3）
pub fn save_assistant_message(
    index_path: &PathBuf,
    id: &str,
    content: &str,
) -> Result<usize, AppError> {
    let mem = Memory {
        id: id.to_string(),
        role: "assistant".to_string(),
        content: content.to_string(),
        timestamp: crate::utils::now_str(),
        tags: None,
        summary: None,
        category: Some(crate::memory::category::classify(content)),
        use_count: 0,
    };
    append_memory(index_path, &mem)
}

/// 标记记忆为重要（加 tags:["important"]），返回更新后的记忆
pub fn mark_important(memory_id: &str, set_name: Option<&str>) -> Result<Memory, AppError> {
    let path = set_index_path(set_name);
    let _guard = MEMORY_WRITER_LOCK
        .lock()
        .map_err(|_| AppError::MemoryError("记忆写锁获取失败".into()))?;

    let mut memories = read_all(&path)?;
    // 用索引方式修改，避免可变借用与后续写回冲突
    let mut found = false;
    for m in memories.iter_mut() {
        if m.id == memory_id {
            let mut tags = m.tags.clone().unwrap_or_default();
            if !tags.iter().any(|t| t == "important") {
                tags.push("important".to_string());
            }
            m.tags = Some(tags);
            found = true;
            break;
        }
    }
    if !found {
        return Err(AppError::MemoryNotFound(memory_id.to_string()));
    }
    atomic_write_index(&path, &memories)?;
    Ok(memories
        .into_iter()
        .find(|m| m.id == memory_id)
        .ok_or_else(|| AppError::MemoryNotFound(memory_id.to_string()))?)
}
