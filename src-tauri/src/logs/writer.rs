// 《铃·记忆体》日志写模块（AI-7 任务 10）
use std::io;

/// 清空主日志文件内容（保留文件，截断为 0 字节）
pub fn clear() -> Result<(), io::Error> {
    let path = super::app_log_path();
    // 用写模式打开（truncate = true）即可清空
    std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(&path)?;
    Ok(())
}
