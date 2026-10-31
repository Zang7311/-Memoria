// 桌宠联动：把「铃说了什么」推给 pet-native 进程，让它头顶冒气泡。
//
// 通道选型：本机 UDP（127.0.0.1:46180）。
// - 桌宠是独立进程，没有 WebView，用不了 Tauri 的事件/IPC；
// - UDP 单向、无连接，桌宠没起来时发送只会静默丢弃，不会像命名管道那样阻塞或报错；
// - 只绑回环地址，外部机器进不来。
//
// 协议（一帧一个数据报，UTF-8，字段用 \t 分隔）：
//   v1\t<mood>\t<ttl 秒>\t<正文>
use std::net::UdpSocket;

use crate::config;

/// 桌宠监听的端口（改这里要同步改 pet-native/src/main.rs 的 PET_PORT）
pub const PET_PORT: u16 = 46180;

/// 气泡最多显示多少字：桌宠窗口很窄（逻辑 150px），再多就只剩省略号了
pub const MAX_CHARS: usize = 24;
/// 气泡默认停留秒数
pub const DEFAULT_TTL_SECS: f32 = 6.0;

/// 从回复文本里猜一个情绪，用来给气泡调色。
/// 只做关键词粗判，判不出来就当平静——宁可猜错成「平静」，也不要乱开心乱难过。
pub fn mood_of(text: &str) -> &'static str {
    // 先看负面：难过/生气比开心更需要被看见
    const SAD: [&str; 8] = ["呜", "难过", "伤心", "对不起", "抱歉", "唉", "寂寞", "委屈"];
    const ANGRY: [&str; 6] = ["哼", "生气", "讨厌", "过分", "不许", "坏蛋"];
    const HAPPY: [&str; 10] = [
        "！", "!", "哈哈", "嘿嘿", "开心", "太好了", "喜欢", "爱你", "谢谢", "耶",
    ];
    const CURIOUS: [&str; 3] = ["？", "?", "好奇"];

    if SAD.iter().any(|k| text.contains(k)) {
        return "sad";
    }
    if ANGRY.iter().any(|k| text.contains(k)) {
        return "angry";
    }
    if HAPPY.iter().any(|k| text.contains(k)) {
        return "happy";
    }
    if CURIOUS.iter().any(|k| text.contains(k)) {
        return "curious";
    }
    "calm"
}

/// 压成一行并截断到 MAX_CHARS，超了补省略号。
pub fn shorten(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| match c {
            '\r' => ' ',
            '\n' => ' ',
            c => c,
        })
        .collect();
    let flat = flat.trim();
    let mut out: String = flat.chars().take(MAX_CHARS).collect();
    if flat.chars().count() > MAX_CHARS {
        out.push('…');
    }
    out
}

/// 组装一帧协议文本（单独拆出来是为了能写单测）
pub fn compose_frame(mood: &str, ttl_secs: f32, text: &str) -> String {
    format!("v1\t{mood}\t{ttl_secs:.1}\t{}", shorten(text))
}

/// 发一帧给桌宠。没开桌宠/桌宠没起来时静默失败（返回 Err 只为日志）。
pub fn say(text: &str) -> Result<&'static str, String> {
    let body = shorten(text);
    if body.is_empty() {
        return Err("空内容不发".into());
    }
    let mood = mood_of(text);
    let frame = compose_frame(mood, DEFAULT_TTL_SECS, text);
    let sock = UdpSocket::bind("127.0.0.1:0").map_err(|e| format!("分配本机端口失败：{e}"))?;
    sock.send_to(frame.as_bytes(), ("127.0.0.1", PET_PORT))
        .map_err(|e| format!("发给桌宠失败：{e}"))?;
    Ok(mood)
}

/// 前端在铃回话结束时调用（设置里关掉「桌宠气泡」就不发）
#[tauri::command]
pub fn native_pet_say(text: String) -> Result<(), String> {
    if !config::store::get_config().native_pet_speech {
        return Ok(());
    }
    match say(&text) {
        Ok(mood) => {
            log::debug!("[pet-talk] 已推给桌宠（情绪 {mood}）");
            Ok(())
        }
        // 桌宠没起来、端口被占都属正常情况，不让前端报错弹窗
        Err(e) => {
            log::debug!("[pet-talk] 未送达：{e}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 情绪按关键词判断() {
        assert_eq!(mood_of("主人今天真棒！"), "happy");
        assert_eq!(mood_of("呜…有点难过"), "sad");
        assert_eq!(mood_of("哼，不理你了"), "angry");
        assert_eq!(mood_of("在做什么呢？"), "curious");
        assert_eq!(mood_of("嗯，知道了"), "calm");
    }

    #[test]
    fn 换行压成一行且超长截断() {
        assert_eq!(shorten("第一行\n第二行"), "第一行 第二行");
        let long = "啊".repeat(MAX_CHARS + 5);
        let out = shorten(&long);
        assert_eq!(out.chars().count(), MAX_CHARS + 1); // 30 个正文 + 省略号
        assert!(out.ends_with('…'));
    }

    #[test]
    fn 协议帧字段齐全() {
        let frame = compose_frame("happy", 6.0, "你好呀");
        let parts: Vec<&str> = frame.split('\t').collect();
        assert_eq!(parts[0], "v1");
        assert_eq!(parts[1], "happy");
        assert_eq!(parts[2], "6.0");
        assert_eq!(parts[3], "你好呀");
    }
}
