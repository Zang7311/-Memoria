// 《铃·记忆体》配置模块（AI-7 任务 1/5）
// 统一配置中心：所有模块通过本模块读写配置，不直接操作文件系统。
pub mod defaults;
pub mod encryption;
pub mod migration;
pub mod store;

use std::path::PathBuf;

/// 配置数据目录：~/.铃记忆体/
/// （任务书约定；注意与 AI-6 的 %APPDATA%/ling-memoria/ 分属两处，见 store.rs 顶部债务注释）
pub fn data_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".铃记忆体")
}

/// 配置文件路径
pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

/// 默认数据路径（记忆存储）：~/Documents/铃记忆体/
pub fn default_data_path() -> String {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    std::path::Path::new(&home)
        .join("Documents")
        .join("铃记忆体")
        .to_string_lossy()
        .to_string()
}

/// 确保数据目录存在
pub fn ensure_data_dir() -> Result<PathBuf, std::io::Error> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
