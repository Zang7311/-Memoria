// 《铃·记忆体》Agent 视觉模块（vision.rs）
//
// 职责：把本地图片读成 base64，送给支持视觉的多模态模型（OpenAI 兼容 image_url 格式），
// 返回文字描述。让 Agent 真正具备「看图」能力 —— 与 agent_screenshot 配合形成闭环：
//   截屏 → 看图 → 理解屏幕上发生了什么 → 决定下一步动作
//
// 模型选择顺序：cfg.vision_model（专用）→ cfg.api_model（回退）
// 未配置 vision_model 时用主模型，若主模型不支持看图，会返回明确的 400 提示。
use std::path::Path;

use base64::Engine as _;
use serde_json::json;

use crate::config;
use crate::error::AppError;

/// 图片大小上限（编码前的原始字节）。超过直接拒绝，避免请求体过大被中转站掐断。
const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;

/// 从全局配置读取视觉调用所需的三要素
fn vision_config() -> Result<(String, String, String), AppError> {
    let cfg = config::store::get_config();

    let base = cfg
        .api_base_url
        .clone()
        .ok_or_else(|| AppError::ConfigError("未配置 API 地址（看图功能需要云端模型）".into()))?;

    let key = cfg
        .api_key_encrypted
        .clone()
        .or_else(|| cfg.api_key_plain.clone())
        .ok_or_else(|| AppError::ConfigError("未配置 API Key".into()))?;

    // 视觉模型：优先「视觉模型」专用配置，留空则回退主模型
    let model = cfg
        .vision_model
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| cfg.api_model.clone());

    Ok((base, key, model))
}

/// 按扩展名推断 MIME 类型（默认按 png 处理）
fn mime_of(path: &str) -> &'static str {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());

    match ext.as_deref() {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        _ => "image/png",
    }
}

/// 让视觉模型「看」一张图片
///
/// # 参数
/// - `image_path`: 本地图片绝对路径
/// - `question`: 想问的问题；为空时做通用描述（并尽量读出图中文字）
///
/// # 返回
/// 模型的文字回答（中文、无 Markdown 符号）
pub async fn look(image_path: &str, question: Option<&str>) -> Result<String, AppError> {
    let p = Path::new(image_path);

    if !p.is_file() {
        return Err(AppError::ToolboxError(format!(
            "图片不存在：{image_path}（可以先用 agent_screenshot 截屏）"
        )));
    }

    let meta = std::fs::metadata(p).map_err(AppError::from)?;
    if meta.len() > MAX_IMAGE_BYTES {
        return Err(AppError::ToolboxError(format!(
            "图片太大（{:.1} MB），上限 8 MB。建议先用 agent_image 缩小后再看",
            meta.len() as f64 / 1024.0 / 1024.0
        )));
    }
    if meta.len() == 0 {
        return Err(AppError::ToolboxError(format!("图片是空文件：{image_path}")));
    }

    let bytes = std::fs::read(p).map_err(AppError::from)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let data_url = format!("data:{};base64,{}", mime_of(image_path), b64);

    let (base, key, model) = vision_config()?;
    let url = format!(
        "{}/chat/completions",
        crate::utils::normalize_v1_url(&base)
    );

    let prompt = question
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(
            "请详细描述这张图片的内容。如果图里有文字，请把文字也读出来。用中文回答，不要使用 Markdown 符号（星号、井号、反引号）。",
        );

    let body = json!({
        "model": model,
        "stream": false,
        "max_tokens": 1024,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": prompt },
                { "type": "image_url", "image_url": { "url": data_url } }
            ]
        }]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .bearer_auth(&key)
        .json(&body)
        .send()
        .await
        .map_err(AppError::from)?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::NetworkError(format!(
            "视觉模型返回 {status}：{text}\n（提示：当前用的模型是「{model}」，它可能不支持看图；可以在设置里填一个多模态模型，例如 glm-4v）"
        )));
    }

    let v: serde_json::Value = resp.json().await.map_err(AppError::from)?;

    // 兼容两种常见返回：choices[0].message.content 为字符串，或为分段数组
    let content = &v["choices"][0]["message"]["content"];
    let text = if let Some(s) = content.as_str() {
        s.trim().to_string()
    } else if let Some(arr) = content.as_array() {
        arr.iter()
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string()
    } else {
        String::new()
    };

    if text.is_empty() {
        Ok(format!("（视觉模型「{model}」没有返回任何内容，可能这张图它看不懂）"))
    } else {
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 推断mime_png() {
        assert_eq!(mime_of("C:\\a\\b\\shot.png"), "image/png");
    }

    #[test]
    fn 推断mime_jpg_大小写不敏感() {
        assert_eq!(mime_of("C:\\a\\b\\photo.JPG"), "image/jpeg");
        assert_eq!(mime_of("x.jpeg"), "image/jpeg");
    }

    #[test]
    fn 推断mime_未知扩展名按png() {
        assert_eq!(mime_of("noext"), "image/png");
        assert_eq!(mime_of("a.xyz"), "image/png");
    }

    #[test]
    fn 推断mime_webp与gif() {
        assert_eq!(mime_of("a.webp"), "image/webp");
        assert_eq!(mime_of("a.gif"), "image/gif");
    }
}
