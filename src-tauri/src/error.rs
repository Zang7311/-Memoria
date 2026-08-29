// 《铃·记忆体》统一错误类型
// 所有引擎/命令的失败都归一到 AppError，避免裸 String 错误
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("网络请求失败：{0}")]
    NetworkError(String),
    #[error("本地模型不可用：{0}")]
    ModelError(String),
    #[error("记忆读写失败：{0}")]
    MemoryError(String),
    #[error("配置错误：{0}")]
    ConfigError(String),
    #[error("流式推送失败：{0}")]
    StreamError(String),
    #[error("内部错误：{0}")]
    InternalError(String),
    #[error("记忆不存在：{0}")]
    MemoryNotFound(String),
    #[error("记忆集不存在：{0}")]
    MemorySetNotFound(String),
    #[error("记忆集已存在：{0}")]
    MemorySetAlreadyExists(String),
    #[error("磁盘空间不足：{0}")]
    #[allow(dead_code)]
    StorageFull(String),
    #[error("索引文件损坏：{0}")]
    IndexCorrupted(String),
    #[error("获取记忆写入锁超时：{0}")]
    #[allow(dead_code)]
    LockTimeout(String),
    // ==================== AI-5 插件系统错误 ====================
    #[error("插件不存在：{0}")]
    PluginNotFound(String),
    #[error("插件已存在：{0}")]
    PluginAlreadyExists(String),
    #[error("插件安装失败：{0}")]
    PluginInstallError(String),
    #[error("权限不足：{0}")]
    PermissionDenied(String),
    #[error("技能不存在：{0}")]
    SkillNotFound(String),
    #[error("插件执行失败：{0}")]
    PluginExecutionError(String),
    #[error("插件执行超时（30 秒）：{0}")]
    PluginTimeout(String),
    // ==================== AI-6 桌面交互错误 ====================
    #[error("获取前台窗口信息失败：{0}")]
    WindowInfoError(String),
    #[error("屏幕监测不可用：{0}")]
    MonitorError(String),
    #[error("工具箱命令执行失败：{0}")]
    ToolboxError(String),
    #[error("工具箱命令执行超时（30 秒）：{0}")]
    ToolboxTimeout(String),
    #[error("全局快捷键注册失败：{0}")]
    HotkeyError(String),
    #[error("开机自启动设置失败：{0}")]
    AutostartError(String),
    // ==================== AI-7 配置与诊断错误 ====================
    #[error("配置读取失败：{0}")]
    ConfigLoadError(String),
    #[error("配置写入失败：{0}")]
    ConfigSaveError(String),
    #[error("配置导入失败：{0}")]
    ConfigImportError(String),
    #[error("配置迁移失败：{0}")]
    ConfigMigrationError(String),
    #[error("日志写入失败：{0}")]
    LogWriteError(String),
    #[error("日志读取失败：{0}")]
    LogReadError(String),
    #[error("加密失败：{0}")]
    EncryptionError(String),
    #[error("解密失败：{0}")]
    DecryptionError(String),
    #[error("未设置主密码：{0}")]
    MasterPasswordNotSet(String),
    #[error("主密码错误：{0}")]
    MasterPasswordWrong(String),
    #[error("尚未解锁：请先输入主密码解锁")]
    Locked,
    #[error("诊断包导出失败：{0}")]
    DiagnosticExportError(String),
    #[error("系统信息采集失败：{0}")]
    SystemInfoError(String),
    // ==================== AI-8 网络与同步错误 ====================
    #[error("同步失败：{0}")]
    SyncError(String),
    #[error("设备发现失败：{0}")]
    DiscoveryError(String),
    #[error("同步校验和不匹配，数据可能被篡改：{0}")]
    ChecksumMismatch(String),
    #[error("更新检查失败：{0}")]
    UpdateCheckError(String),
    #[error("网络监测失败：{0}")]
    NetworkMonitorError(String),
}

// 让 AppError 能直接作为 Tauri 命令的错误返回（实现 Into<String>）
impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::NetworkError(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::InternalError(format!("JSON 解析失败：{e}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::MemoryError(e.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        AppError::StreamError(e.to_string())
    }
}

// AI-5：JS 引擎（boa_engine）错误 → 插件执行错误
impl From<boa_engine::JsError> for AppError {
    fn from(e: boa_engine::JsError) -> Self {
        AppError::PluginExecutionError(format!("JS 引擎错误：{e}"))
    }
}

// Tauri 命令返回 Result<T, E> 时，E 需要实现 Serialize。
// AppError 通过 thiserror 的 Display 输出友好中文信息。
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
