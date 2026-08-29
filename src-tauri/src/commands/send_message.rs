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
    // 读取设置（当前阶段 settings 尚未持久化，用默认值 + 可选覆盖）
    // 注意：设置由前端管理，这里先用默认 Setting；后续接入设置命令后替换。
    let setting = Setting::default();

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
            engine::api::run_api(app, input, &memories, &base, &key, depth).await?
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
