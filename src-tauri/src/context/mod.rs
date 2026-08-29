// 《铃·记忆体》上下文管理
pub mod loader;

use std::sync::Mutex;

/// 全局记忆写锁：所有 index.json 写操作前必须获取此锁，
/// 防止与 AI-4（记忆存储）并发写入导致索引损坏。
pub static MEMORY_WRITER_LOCK: Mutex<()> = Mutex::new(());
