// 《铃·记忆体》VBIL 模块 —— 响应桥接（三种模式）
//
// 数据流：recv_event() 收事件 → 白名单/总开关检查 → 规则引擎判断 → 响应策略 → send_action()。
// 三种模式：
//   - off       不响应任何事件（仅记日志）
//   - rule_only 用规则引擎固定回复，零 Token
//   - ai        调用铃对话引擎生成回复

use crate::engine;
use crate::vbil::config;
use crate::vbil::rules;
use crate::vbil::server::{recv_event, send_action};
use crate::vbil::types::IncomingEvent;
use serde_json::json;
use tauri::AppHandle;

/// 启动响应桥接后台任务（setup 时调用）
pub fn spawn_responder(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            // 阻塞等待入站事件；通道关闭时短暂休眠，避免忙循环
            let ev = match recv_event().await {
                Some(e) => e,
                None => {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    continue;
                }
            };
            handle_event(&app, ev).await;
        }
    });
}

/// 处理单个入站事件
async fn handle_event(app: &AppHandle, ev: IncomingEvent) {
    // 白名单检查（空白名单 = 全部允许）
    if !config::is_allowed(&ev.from) {
        log::debug!("[vbil] 客户端 {} 不在白名单，忽略事件 {}", ev.from, ev.event);
        return;
    }

    // 记录事件接收
    log::info!(
        "[vbil] 事件接收：id={} event={} data={:?}",
        ev.from,
        ev.event,
        ev.data
    );

    // 总开关
    if !config::get_vbil_status() {
        log::debug!("[vbil] VBIL 未启用，事件仅记录（id={}）", ev.from);
        return;
    }

    // 规则引擎判断
    let m = rules::match_event(&ev.from, &ev.event);
    log::info!(
        "[vbil] 规则匹配：id={} event={} should_respond={}",
        ev.from,
        ev.event,
        m.should_respond
    );
    if !m.should_respond {
        return;
    }

    // 响应策略（模式实时读取，切换即时生效）
    let mode = config::get_mode();
    match mode.as_str() {
        "off" => {
            log::debug!("[vbil] 模式 off，不响应");
        }
        "rule_only" => {
            if let Some(text) = m.response_text {
                log::info!("[vbil] rule_only 响应：{}", text);
                let _ = send_action(&ev.from, "show_text", json!({ "content": text })).await;
            }
        }
        "ai" => match generate_reply(app, &ev).await {
            Ok(reply) => {
                log::info!("[vbil] ai 响应：{}", reply);
                let _ = send_action(&ev.from, "show_text", json!({ "content": reply })).await;
            }
            Err(e) => {
                log::warn!("[vbil] ai 生成回复失败：{e}");
            }
        },
        other => {
            log::warn!("[vbil] 未知响应模式：{}", other);
        }
    }
}

/// ai 模式：调用对话引擎生成回复
async fn generate_reply(app: &AppHandle, ev: &IncomingEvent) -> Result<String, String> {
    // 事件 data 里的 text 作为输入（如 speaking 事件的 {text:"..."}），否则用事件类型名
    let input = ev
        .data
        .as_ref()
        .and_then(|d| d.get("text"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| ev.event.clone());

    let cfg = crate::config::store::get_config();
    let base = cfg.api_base_url.clone().unwrap_or_default();
    let key = decrypt_api_key(&cfg)?;
    let self_name = cfg.self_name.clone().unwrap_or_else(|| "铃".to_string());
    let user_name = cfg.user_name.clone().unwrap_or_else(|| "主人".to_string());
    let persona = cfg.persona.clone();

    match cfg.model_mode.as_str() {
        "api" => engine::api::run_api(
            app,
            &input,
            &[],
            &base,
            &key,
            &cfg.api_model,
            cfg.depth,
            &persona,
            &self_name,
            &user_name,
        )
        .await
        .map_err(|e| e.to_string()),
        "local" => engine::local::run_local(app, &input, &[], cfg.depth)
            .await
            .map_err(|e| e.to_string()),
        _ => {
            // script 离线模板模式不适合 VBIL 的 ai 响应，返回固定提示
            Ok(format!("{}在呢～", self_name))
        }
    }
}

/// 解密 API Key（复用 AI-7 密钥体系，逻辑同 send_message.rs）
fn decrypt_api_key(cfg: &crate::types::AppConfig) -> Result<String, String> {
    if let Some(enc) = &cfg.api_key_encrypted {
        if !enc.is_empty() {
            let key = crate::config::encryption::get_key().map_err(|e| e.to_string())?;
            return crate::config::encryption::decrypt_with_key(&key, enc)
                .map_err(|e| e.to_string());
        }
    }
    if let Some(plain) = &cfg.api_key_plain {
        if !plain.is_empty() {
            return Ok(plain.clone());
        }
    }
    Err("未配置 API Key".to_string())
}
