// 《铃·记忆体》工具函数：时间格式化、ID 生成、日志初始化
use chrono::Local;

/// 当前时间字符串（ISO 风格，本地时区）
pub fn now_str() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%.3f").to_string()
}

/// 生成一个消息 ID（格式：msg_<timestamp>_<随机>）
pub fn gen_id() -> String {
    let ts = Local::now().format("%Y%m%d%H%M%S%3f").to_string();
    format!("msg_{ts}_{}", uuid::Uuid::new_v4().simple())
}
