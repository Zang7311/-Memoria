// 《铃·记忆体》配置模块（AI-7 任务 1/5）
// 统一配置中心：所有模块通过本模块读写配置，不直接操作文件系统。
pub mod defaults;
pub mod encryption;
pub mod migration;
pub mod store;

use std::path::PathBuf;

/// 本版本的数据目录名（v1.0 离线智能版定制版）
///
/// ⚠️ 与主线 v0.5.x 的 `.铃记忆体` 刻意分开：这是内置大模型的定制分支，配置结构
/// （model_mode 取值）与主线不兼容，共用目录会互相写坏对方的 config.json。
/// 模型文件是唯一例外——local_llm::model_dirs() 会额外只读探测主线目录，
/// 避免用户为同一份 GGUF 重复下载 1.5GB。
pub const DATA_DIR_NAME: &str = ".铃记忆体-v10";

/// 配置数据目录：~/.铃记忆体-v10/
/// （注意与 AI-6 的 %APPDATA%/ling-memoria/ 分属两处，见 store.rs 顶部债务注释）
pub fn data_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(DATA_DIR_NAME)
}

/// 配置文件路径
pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

/// 默认数据路径（记忆存储）：~/Documents/铃记忆体-v10/
/// 同样与主线分开，避免两个版本往同一份 index.json 交叉写入。
pub fn default_data_path() -> String {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    std::path::Path::new(&home)
        .join("Documents")
        .join("铃记忆体-v10")
        .to_string_lossy()
        .to_string()
}

/// 应用私有目录名（%APPDATA% 下；插件注册表 / 桌面模块 / 记忆回退路径共用）
/// 主线是 `ling-memoria`，本定制版用 `ling-memoria-v10`。
pub const APP_DIR_NAME: &str = "ling-memoria-v10";

/// %APPDATA%/ling-memoria-v10/（插件注册表、监测规则、工具箱条目、记忆回退目录）
pub fn app_dir() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join(APP_DIR_NAME)
}

/// 确保数据目录存在
pub fn ensure_data_dir() -> Result<PathBuf, std::io::Error> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
