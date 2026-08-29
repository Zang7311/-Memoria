// 《铃·记忆体》本地 Ollama 模式引擎
// 只检测不托管：检测 localhost:11434 是否可用，不可用则推送明确错误提示。
// 从 /api/tags 取模型列表，默认用第一个；POST /api/chat 流式对话。
use crate::engine;
use crate::error::AppError;
use crate::stream::sender;
use crate::types::Memory;
use futures_util::StreamExt;
use serde::Deserialize;
use tauri::AppHandle;

const OLLAMA_BASE: &str = "http://localhost:11434";

/// Ollama /api/tags 响应
#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<ModelTag>,
}

#[derive(Debug, Deserialize)]
struct ModelTag {
    name: String,
}

/// Ollama /api/chat 流式响应的一行
#[derive(Debug, Deserialize)]
struct ChatChunk {
    #[serde(default)]
    message: Option<ChatMessage>,
    #[serde(default)]
    done: bool,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: String,
}

/// 检测 Ollama 服务是否可用，返回可用模型名
async fn detect_model() -> Result<String, AppError> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{OLLAMA_BASE}/api/tags"))
        .send()
        .await
        .map_err(|e| {
            AppError::ModelError(format!(
                "未检测到 Ollama 服务，请手动启动或安装（{e}）"
            ))
        })?;

    if !resp.status().is_success() {
        return Err(AppError::ModelError(format!(
            "Ollama 服务异常（HTTP {}）",
            resp.status()
        )));
    }

    let tags: TagsResponse = resp.json().await.map_err(|e| {
        AppError::ModelError(format!("解析模型列表失败：{e}"))
    })?;

    tags.models
        .into_iter()
        .next()
        .map(|m| m.name)
        .ok_or_else(|| AppError::ModelError("未检测到已安装的 Ollama 模型".into()))
}

/// 运行本地模式：流式对话并推送
pub async fn run_local(
    app: &AppHandle,
    input: &str,
    context: &[Memory],
    depth: u8,
) -> Result<String, AppError> {
    let model = detect_model().await?;
    log::info!("[local] 使用模型「{model}」");

    let (temperature, top_p, _) = engine::apply_depth(depth);

    let mut messages: Vec<serde_json::Value> = context
        .iter()
        .map(|m| serde_json::json!({ "role": m.role, "content": m.content }))
        .collect();
    messages.push(serde_json::json!({ "role": "user", "content": input }));

    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "options": {
            "temperature": temperature,
            "top_p": top_p,
        },
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{OLLAMA_BASE}/api/chat"))
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::ModelError(format!("请求 Ollama 失败：{e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::ModelError(format!(
            "Ollama 返回 HTTP {}",
            resp.status()
        )));
    }

    let mut full = String::new();
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();

    while let Some(item) = stream.next().await {
        let bytes = item.map_err(|e| AppError::ModelError(e.to_string()))?;
        buf.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim().to_string();
            buf.drain(..=pos);

            if line.is_empty() {
                continue;
            }
            if let Ok(chunk) = serde_json::from_str::<ChatChunk>(&line) {
                if let Some(msg) = chunk.message {
                    if !msg.content.is_empty() {
                        full.push_str(&msg.content);
                        sender::send_chunk(app, &msg.content)?;
                    }
                }
                if chunk.done {
                    break;
                }
            }
        }
    }

    if full.is_empty() {
        return Err(AppError::InternalError("本地模型无输出".into()));
    }
    sender::send_end(app)?;
    Ok(full)
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
