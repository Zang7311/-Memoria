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

/// 规范化 API Base URL：去尾部斜杠；若已以 /v1 结尾则不重复拼接，否则补上 /v1。
/// 兼容用户填写带/不带 /v1 的地址（如 https://api.deepseek.com 或 .../v1 均可用）。
pub fn normalize_v1_url(base: &str) -> String {
    let b = base.trim().trim_end_matches('/');
    if b.ends_with("/v1") {
        b.to_string()
    } else {
        format!("{b}/v1")
    }
}
