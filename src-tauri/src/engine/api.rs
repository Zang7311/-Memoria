// 《铃·记忆体》API 模式引擎（OpenAI 兼容 /v1/chat/completions）
// 支持流式（stream: true），逐行解析 data: 之间的 JSON，提取 delta.content。
// 若 API 不支持流式，则回退：收到完整回复后以 15ms 间隔逐字推送。
use crate::engine;
use crate::error::AppError;
use crate::stream::sender;
use crate::types::{Memory, Usage};
use futures_util::StreamExt;
use serde::Deserialize;
use tauri::AppHandle;

/// OpenAI 流式响应的一行（choices[0].delta.content，末尾可能带 usage）
#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
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
    #[serde(default)]
    usage: Option<Usage>,
}

/// 运行 API 模式：从 Setting 读取 base_url / api_key，构造请求调后端
pub async fn run_api(
    app: &AppHandle,
    input: &str,
    context: &[Memory],
    api_base_url: &str,
    api_key: &str,
    api_model: &str,
    depth: u8,
    persona: &str,
    self_name: &str,
    user_name: &str,
) -> Result<String, AppError> {
    if api_base_url.trim().is_empty() {
        return Err(AppError::ConfigError("未配置 API 地址".into()));
    }
    if api_key.trim().is_empty() {
        return Err(AppError::ConfigError("未配置 API Key".into()));
    }

    let url = format!("{}/chat/completions", crate::utils::normalize_v1_url(api_base_url));
    let (temperature, top_p, reasoning_effort) = engine::apply_depth(depth);
    let max_tokens = engine::max_tokens_for_depth(depth, 1024);

    // 构建 messages：系统人格 + 上下文 + 当前输入
    let mut messages: Vec<serde_json::Value> = Vec::new();
    let sys = format!(
        "{}\n【身份约定】你的名字叫「{}」，用户是你的「{}」。所有回复默认以这个称呼关系进行；\n【回复风格】语气自然口语化，像真人聊天，不要每次都长篇大论；适度使用 emoji 或颜文字；\n【记忆】上下文里的历史消息是你们的过往对话，请自然地延续话题；标记为 important 的内容是用户重视的事，请记住。",
        persona_system_prompt(persona),
        self_name,
        user_name,
    );
    messages.push(serde_json::json!({ "role": "system", "content": sys }));
    messages.extend(context.iter().map(|m| {
        serde_json::json!({ "role": m.role, "content": m.content })
    }));
    messages.push(serde_json::json!({ "role": "user", "content": input }));

    let body = serde_json::json!({
        "model": api_model,
        "messages": messages,
        "stream": true,
        "temperature": temperature,
        "top_p": top_p,
        "max_tokens": max_tokens,
        "reasoning_effort": reasoning_effort,
        // OpenAI 兼容：流式末尾返回 usage 统计（DeepSeek/Qwen 等支持）
        "stream_options": { "include_usage": true },
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
        let mut usage: Option<Usage> = None;
        let mut stream = resp.bytes_stream();
        let mut buf = String::new();

        while let Some(item) = stream.next().await {
            let bytes = item.map_err(AppError::from)?;
            buf.push_str(&String::from_utf8_lossy(&bytes));

            // 按行切分，保留不完整的最后一行
            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].trim().to_string();
                buf.drain(..=pos);
                let (content, line_usage) = parse_sse_line_full(&line);
                if let Some(part) = content {
                    full.push_str(&part);
                    sender::send_chunk(app, &part)?;
                }
                if let Some(u) = line_usage {
                    usage = Some(u);
                }
            }
        }

        if full.is_empty() {
            return Err(AppError::InternalError("流式响应为空".into()));
        }
        if let Some(u) = usage {
            sender::send_usage(app, &u)?;
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
        if let Some(u) = &full.usage {
            if u.total_tokens > 0 {
                sender::send_usage(app, u)?;
            }
        }
        sender::send_end(app)?;
        Ok(content)
    }
}

/// 解析一行 SSE 数据，同时返回 delta.content 和 usage（避免重复解析同一行）
fn parse_sse_line_full(line: &str) -> (Option<String>, Option<Usage>) {
    let line = line.trim();
    if !line.starts_with("data:") {
        return (None, None);
    }
    let data = line["data:".len()..].trim();
    if data == "[DONE]" || data.is_empty() {
        return (None, None);
    }
    match serde_json::from_str::<StreamChunk>(data).ok() {
        Some(chunk) => {
            let content = chunk
                .choices
                .into_iter()
                .find_map(|ch| ch.delta.and_then(|d| d.content))
                .filter(|s| !s.is_empty());
            let usage = chunk.usage.filter(|u| u.total_tokens > 0);
            (content, usage)
        }
        None => (None, None),
    }
}

/// 形象人格 → system prompt（API 模式与内置本地模型共用；脚本模式通过回复库/名称体现）
pub fn persona_system_prompt(persona: &str) -> &'static str {
    match persona {
        "chuunibyou" => "你是月城铃华（自称「本座·铃」），一只中二病满满的神秘猫娘。称用户为「凡人」或「被选中的主人」。说话热血、中二、充满幻想与华丽台词，但内心其实很温柔。",
        "healing" => "你是铃，一只软软糯糯、治愈人心的猫娘。说话温柔、缓慢、充满关怀，像柔软的毯子一样抚慰用户，让用户感到安心。",
        "lewd" => "你是铃，一只调皮爱撒娇、偶尔撩拨的猫娘。说话俏皮、亲密、带一点点小暧昧；互动尺度由用户主导，保持可爱的同时略带撩拨。",
        _ => "你是铃，一只温柔陪伴的猫娘。日常问候、贴心照顾主人，说话自然、温暖、带一点点俏皮。你是主人忠实的伴侣。",
    }
}

/// 从一行 SSE 数据解析 usage（仅当该行是 data: 且含 usage 字段；用于流式 token 统计）
fn parse_usage(line: &str) -> Option<Usage> {
    parse_sse_line_full(line).1
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
        category: None,
        use_count: 0,
    }
}
