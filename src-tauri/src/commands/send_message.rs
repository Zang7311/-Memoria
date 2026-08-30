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
/// session_id：当前会话 id（可选）。提供时用该会话的历史消息作为对话上下文主体，
/// 并叠加全局重要记忆兜底——多会话之间不再串味。
#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    content: String,
    depth: u8,
    session_id: Option<String>,
) -> Result<SendMessageResponse, AppError> {
    let message_id = crate::utils::gen_id();
    let stream_id = crate::utils::gen_id();
    let input = content.clone();

    log::info!("[send_message] 收到消息 id={message_id} depth={depth} session={session_id:?}");

    // 立即返回，后台异步生成
    tauri::async_runtime::spawn(async move {
        if let Err(e) = generate_and_emit(&app, &input, depth, session_id.as_deref()).await {
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
    session_id: Option<&str>,
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

    // 加载上下文（script 模式也读记忆：用于无关键词命中时的记忆兜底分类）
    // —— 会话感知：提供 session_id 时用该会话历史消息为主体上下文（多会话隔离），
    //    再叠加全局重要记忆兜底；未提供时保持原行为（全局记忆最近 N 条）——
    let index_path = memory::storage::default_index_path();
    let all_memories = memory::storage::read_all(&index_path)?;
    let memories = match setting.model_mode.as_str() {
        "script" => all_memories,
        _ => {
            let mut ctx = Vec::new();
            // 1) 会话历史（若提供了 session_id 且会话存在）→ 转 Memory + 截断
            if let Some(sid) = session_id {
                if let Ok(session) = crate::sessions::storage::load_session(sid) {
                    for m in &session.messages {
                        ctx.push(crate::types::Memory {
                            id: m.id.clone(),
                            role: m.role.clone(),
                            content: m.content.clone(),
                            timestamp: m.timestamp.clone(),
                            tags: None,
                            summary: None,
                        });
                    }
                    log::info!("[send_message] 会话 {sid} 上下文 {0} 条", ctx.len());
                    // 会话消息按 context_length 条数 + token 上限截断（取最近，防超限）
                    ctx = context::loader::build_context(
                        &ctx,
                        setting.context_length.max(8),
                        context_max_tokens(depth),
                    );
                }
            }
            // 2) 全局重要记忆兜底（不与会话消息重复时补充）
            if !ctx.is_empty() {
                // 已有会话上下文：只补充全局 important 记忆（最多 context_length 条）
                let important: Vec<crate::types::Memory> = all_memories
                    .iter()
                    .filter(|m| m.tags.as_ref().is_some_and(|t| t.iter().any(|x| x == "important")))
                    .take(setting.context_length as usize)
                    .cloned()
                    .collect();
                ctx.extend(important);
                ctx
            } else {
                // 无会话：原行为——全局记忆最近 N 条 + important 加权
                context::loader::build_context(
                    &all_memories,
                    setting.context_length,
                    context_max_tokens(depth),
                )
            }
        }
    };

    // 记录用户输入到记忆（先记用户，再生成回复）
    // —— 智能筛选：无信息量的废话（嗯/哦/哈哈等）不写入记忆，避免污染上下文 ——
    if should_memorize_user(input) {
        let _ = memory::storage::save_user_message(
            &index_path,
            &crate::utils::gen_id(),
            input,
        );
    }

    // 按 model_mode 选择引擎
    let reply = match setting.model_mode.as_str() {
        "api" => {
            let base = setting.api_base_url.clone().unwrap_or_default();
            let key = setting.api_key.clone().unwrap_or_default();
            let self_name = setting.self_name.clone().unwrap_or_else(|| "铃".to_string());
            let user_name = setting.user_name.clone().unwrap_or_else(|| "主人".to_string());
            engine::api::run_api(app, input, &memories, &base, &key, &setting.api_model, depth, &setting.persona, &self_name, &user_name).await?
        }
        "local" => {
            engine::local::run_local(app, input, &memories, depth).await?
        }
        _ => engine::script::run_script(app, input, &setting, depth, &memories).await?,
    };

    // 流结束后，将完整回复写入记忆
    // —— script 模式回复是固定模板（无新信息），不写入记忆，避免污染上下文 ——
    if setting.model_mode.as_str() != "script" {
        if let Err(e) = memory::storage::save_assistant_message(
            &index_path,
            &crate::utils::gen_id(),
            &reply,
        ) {
            log::warn!("写入助手回复失败：{e}");
        }
    }

    Ok(())
}

/// 用户消息记忆筛选：无信息量的短句/语气词不写入记忆
/// 规则：≤2 字的纯语气词（嗯/哦/啊/哈/好的/ok 等）、纯重复表情符 → 不记
fn should_memorize_user(input: &str) -> bool {
    let t = input.trim();
    if t.is_empty() {
        return false;
    }
    // 纯语气词/简短应答（≤2 字）：嗯 哦 啊 哈 好 是 对 行 嗯嗯 哦哦 好的 好滴 好吧 ok OK 嗯呐 知道了
    let short = ["嗯", "哦", "啊", "哈", "好", "是", "对", "行", "嗯嗯", "哦哦", "哈哈", "嘿嘿", "好的", "好滴", "好吧", "好哦", "嗯呐", "ok", "OK", "Ok", "oK", "知道了", "明白", "收到", "是嘛", "对啊", "哈哈", "嘻嘻", "哈哈哈", "嘿嘿嘿", "呜呜", "啊啊", "诶", "咦", "哦哦哦", "嗯嗯嗯", "好耶", "耶", "哇", "哇哦", "真的吗", "这样啊", "原来如此", "是的", "没错", "可以", "行吧", "随便", "都行", "嗯呢", "嗯嗯呢", "好的呢"];
    if short.contains(&t) {
        return false;
    }
    // 纯重复单字（>2字全是同一个语气字，如 哈哈哈哈、嗯嗯嗯嗯）
    let chars: Vec<char> = t.chars().collect();
    if chars.len() >= 3 && chars.iter().all(|c| *c == chars[0]) {
        // 允许 哈哈 2 字已在上方拦截；3+ 字重复且是语气字 → 不记
        let tone = ['哈', '嗯', '哦', '啊', '嘿', '呜', '诶', '咦', '哇'];
        if tone.contains(&chars[0]) {
            return false;
        }
    }
    // 表情符号串（全是非文字字符）
    if !t.chars().any(|c| c.is_alphanumeric() || c.is_ascii_punctuation()) && t.chars().count() <= 6 {
        return false;
    }
    true
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
