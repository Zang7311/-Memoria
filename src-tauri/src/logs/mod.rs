// 《铃·记忆体》日志模块（AI-7 任务 7/10）
//
// 结构：
//   logs::init()           — 初始化 log crate，后端写入 app.log
//   logs::app_log_path()   — 返回日志文件完整路径
//   logs::writer::clear()  — 清空日志文件
//   logs::reader::read_logs() — 读取并分页过滤日志
pub mod reader;
pub mod writer;

use std::path::PathBuf;

/// 日志文件路径：~/.铃记忆体/app.log
pub fn app_log_path() -> PathBuf {
    crate::config::data_dir().join("app.log")
}

/// 日志目录：~/.铃记忆体/（app.log 所在目录，供诊断包打包使用）
pub fn logs_dir() -> PathBuf {
    crate::config::data_dir()
}

/// 初始化日志系统：将 log crate 路由到文件 + 控制台
pub fn init() {
    // 确保目录存在（config 模块可能尚未初始化）
    let dir = crate::config::data_dir();
    let _ = std::fs::create_dir_all(&dir);

    let log_path = app_log_path();

    // 使用 env_logger-like 简单实现：通过 log::set_logger 路由到文件
    // 用标准库实现，不引入额外依赖
    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let logger = FileLogger {
            file: std::sync::Mutex::new(file),
        };
        if log::set_boxed_logger(Box::new(logger)).is_ok() {
            log::set_max_level(log::LevelFilter::Info);
        }
    }
}

struct FileLogger {
    file: std::sync::Mutex<std::fs::File>,
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S");
        let line = format!(
            "[{}] [{}] {}\n",
            now,
            record.level(),
            record.args()
        );
        let line = crate::config::store::get_runtime_config().redact_api_secrets(&line);
        if let Ok(mut f) = self.file.lock() {
            use std::io::Write;
            let _ = f.write_all(line.as_bytes());
        }
        // 同时输出到 stderr（开发时可见）
        eprint!("{}", line);
    }

    fn flush(&self) {
        use std::io::Write;
        if let Ok(mut f) = self.file.lock() {
            let _ = f.flush();
        }
    }
}
