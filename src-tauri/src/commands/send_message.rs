// 《铃·记忆体》send_message 核心命令
// 输入 SendMessageRequest { content, depth }，立即返回 message_id + stream_id，
// 在后台异步生成回复并通过 chat_chunk / chat_end / chat_error 事件推送。
use crate::context;
use crate::engine;
use crate::error::AppError;
use crate::memory;
use crate::stream;
use crate::types::{SendMessageResponse, Setting};
use tauri::AppHandle;

/// 发送消息命令：启动流式对话，不阻塞等待完整生成
#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    content: String,
    depth: u8,
) -> Result<SendMessageResponse, AppError> {
    let message_id = crate::utils::gen_id();
    let stream_id = crate::utils::gen_id();
    let input = content.clone();

    log::info!("[send_message] 收到消息 id={message_id} depth={depth}");

    // 立即返回，后台异步生成
    tauri::async_runtime::spawn(async move {
        if let Err(e) = generate_and_emit(&app, &input, depth).await {
            log::error!("对话生成失败：{e}");
            // 尽力推送错误事件
            let _ = stream::sender::send_error(&app, &e.to_string());
        }
    });

    Ok(SendMessageResponse {
        message_id,
        stream_id,
    })
}

/// 后台生成回复并推送事件（真正的业务逻辑）
async fn generate_and_emit(
    app: &AppHandle,
    input: &str,
    depth: u8,
) -> Result<(), AppError> {
    // 读取配置中心（AI-7 实现）：真实配置从 ~/.铃记忆体/config.json 加载，
    // 不再使用硬编码默认值（修复 API 无法接入的问题）
    let cfg = crate::config::store::get_config();
    let setting = Setting {
        theme: cfg.theme.clone(),
        context_length: cfg.context_length,
        api_base_url: cfg.api_base_url.clone(),
        api_key: decrypt_api_key(&cfg)?,
        api_model: cfg.api_model.clone(),
        model_mode: cfg.model_mode.clone(),
        depth: cfg.depth,
        self_name: cfg.self_name.clone(),
        user_name: cfg.user_name.clone(),
        persona: cfg.persona.clone(),
    };

    // 加载上下文（脚本模式无需上下文）
    let index_path = memory::storage::default_index_path();
    let memories = match setting.model_mode.as_str() {
        "script" => Vec::new(),
        _ => {
            let all = memory::storage::read_all(&index_path)?;
            context::loader::build_context(
                &all,
                setting.context_length,
                context_max_tokens(depth),
            )
        }
    };

    // 记录用户输入到记忆（先记用户，再生成回复）
    let _ = memory::storage::save_user_message(
        &index_path,
        &crate::utils::gen_id(),
        input,
    );

    // 按 model_mode 选择引擎
    let reply = match setting.model_mode.as_str() {
        "api" => {
            let base = setting.api_base_url.clone().unwrap_or_default();
            let key = setting.api_key.clone().unwrap_or_default();
            engine::api::run_api(app, input, &memories, &base, &key, &setting.api_model, depth, &setting.persona).await?
        }
        "local" => {
            engine::local::run_local(app, input, &memories, depth).await?
        }
        _ => engine::script::run_script(app, input, &setting, depth).await?,
    };

    // 流结束后，将完整回复写入记忆
    if let Err(e) = memory::storage::save_assistant_message(
        &index_path,
        &crate::utils::gen_id(),
        &reply,
    ) {
        log::warn!("写入助手回复失败：{e}");
    }

    Ok(())
}

/// 根据深度估算上下文 token 上限（深度越高，上下文可越多）
fn context_max_tokens(depth: u8) -> usize {
    match depth {
        1 => 512,
        3 => 2048,
        4 => 4096,
        _ => 1024,
    }
}

/// 从配置中心解密 API Key（AES-256-GCM，复用 AI-7 密钥体系）
/// - 未配置密钥 → None（引擎会报「未配置 API Key」）
/// - 已加密但未解锁 → 报 Locked（提示用户先解锁）
fn decrypt_api_key(cfg: &crate::types::AppConfig) -> Result<Option<String>, AppError> {
    // 优先读取加密存储（需已解锁）
    if let Some(enc) = &cfg.api_key_encrypted {
        if !enc.is_empty() {
            let key = crate::config::encryption::get_key()?;
            return Ok(Some(crate::config::encryption::decrypt_with_key(&key, enc)?));
        }
    }
    // 回退明文存储（未设置主密码时保存的场景）
    if let Some(plain) = &cfg.api_key_plain {
        if !plain.is_empty() {
            return Ok(Some(plain.clone()));
        }
    }
    Ok(None)
}
