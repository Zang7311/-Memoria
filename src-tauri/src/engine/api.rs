// 《铃·记忆体》API 模式引擎（OpenAI 兼容 /v1/chat/completions）
// 支持流式（stream: true），逐行解析 data: 之间的 JSON，提取 delta.content。
// 若 API 不支持流式，则回退：收到完整回复后以 15ms 间隔逐字推送。
use crate::engine;
use crate::error::AppError;
use crate::stream::sender;
use crate::types::Memory;
use futures_util::StreamExt;
use serde::Deserialize;
use tauri::AppHandle;

/// OpenAI 流式响应的一行（choices[0].delta.content）
#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    delta: Option<Delta>,
    message: Option<Delta>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
}

/// 完整（非流式）响应
#[derive(Debug, Deserialize)]
struct FullResponse {
    choices: Vec<Choice>,
}

/// 运行 API 模式：从 Setting 读取 base_url / api_key，构造请求调后端
pub async fn run_api(
    app: &AppHandle,
    input: &str,
    context: &[Memory],
    api_base_url: &str,
    api_key: &str,
    depth: u8,
) -> Result<String, AppError> {
    if api_base_url.trim().is_empty() {
        return Err(AppError::ConfigError("未配置 API 地址".into()));
    }
    if api_key.trim().is_empty() {
        return Err(AppError::ConfigError("未配置 API Key".into()));
    }

    let url = format!("{}/v1/chat/completions", api_base_url.trim_end_matches('/'));
    let (temperature, top_p, reasoning_effort) = engine::apply_depth(depth);
    let max_tokens = engine::max_tokens_for_depth(depth, 1024);

    // 构建 messages：上下文 + 当前输入
    let mut messages: Vec<serde_json::Value> = context
        .iter()
        .map(|m| {
            serde_json::json!({ "role": m.role, "content": m.content })
        })
        .collect();
    messages.push(serde_json::json!({ "role": "user", "content": input }));

    let body = serde_json::json!({
        "model": "gpt-3.5-turbo",
        "messages": messages,
        "stream": true,
        "temperature": temperature,
        "top_p": top_p,
        "max_tokens": max_tokens,
        "reasoning_effort": reasoning_effort,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(AppError::from)?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::NetworkError(format!(
            "API 返回 {status}：{text}"
        )));
    }

    // 检查是否真的返回了流式（Content-Type 含 text/event-stream）
    let is_stream = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("text/event-stream"))
        .unwrap_or(false);

    if is_stream {
        let mut full = String::new();
        let mut stream = resp.bytes_stream();
        let mut buf = String::new();

        while let Some(item) = stream.next().await {
            let bytes = item.map_err(AppError::from)?;
            buf.push_str(&String::from_utf8_lossy(&bytes));

            // 按行切分，保留不完整的最后一行
            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].trim().to_string();
                buf.drain(..=pos);
                if let Some(part) = parse_sse_line(&line) {
                    full.push_str(&part);
                    sender::send_chunk(app, &part)?;
                }
            }
        }

        if full.is_empty() {
            return Err(AppError::InternalError("流式响应为空".into()));
        }
        sender::send_end(app)?;
        Ok(full)
    } else {
        // 非流式回退：取完整响应，逐字推送
        let full: FullResponse = resp.json().await.map_err(AppError::from)?;
        let content = full
            .choices
            .into_iter()
            .find_map(|c| c.message.and_then(|m| m.content))
            .unwrap_or_default();

        if content.is_empty() {
            return Err(AppError::InternalError("API 响应无内容".into()));
        }

        for ch in content.chars() {
            sender::send_chunk(app, &ch.to_string())?;
            tokio::time::sleep(std::time::Duration::from_millis(15)).await;
        }
        sender::send_end(app)?;
        Ok(content)
    }
}

/// 解析一行 SSE 数据，返回其中的 delta.content
fn parse_sse_line(line: &str) -> Option<String> {
    let line = line.trim();
    if !line.starts_with("data:") {
        return None;
    }
    let data = line["data:".len()..].trim();
    if data == "[DONE]" {
        return None;
    }
    serde_json::from_str::<StreamChunk>(data)
        .ok()
        .and_then(|c| {
            c.choices
                .into_iter()
                .find_map(|ch| ch.delta.and_then(|d| d.content))
                .filter(|s| !s.is_empty())
        })
}

/// 生成一条 assistant 记忆
pub fn to_memory(id: &str, reply: &str) -> Memory {
    Memory {
        id: id.to_string(),
        role: "assistant".to_string(),
        content: reply.to_string(),
        timestamp: crate::utils::now_str(),
        tags: None,
        summary: None,
    }
}
