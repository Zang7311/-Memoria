// 《铃·记忆体》AI-10 Agent 循环核心（loop_.rs）
//
// 这是 Agent 能力的核心：多轮 function calling 循环。
//
// 流程：
//   1. 组装 tools（工具箱 + 插件技能，见 tools.rs）
//   2. 拉取相关记忆作为规划上下文
//   3. POST /chat/completions，注入 tools
//   4. 解析响应：
//      - 有 tool_calls → 路由执行 → 结果回传 → 回到步骤 3
//      - 有纯文本 → 作为最终回复返回
//   5. 达到 max_steps 仍无最终回复 → 强制让 LLM 总结已执行的步骤并返回
//
// 安全约束：
//   - 危险工具已在 tools.rs 层面过滤，LLM 根本看不到
//   - 每次工具执行前，危险操作二次确认走 toolbox_execute 现有逻辑
//   - 工具结果摘要化（截断），防止上下文爆炸
use std::collections::HashMap;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::agent::evaluator;
use crate::agent::experience;
use crate::agent::planner;
use crate::agent::recovery;
use crate::agent::router::{dispatch_tool_call, ToolResult};
use crate::agent::stream_progress::StreamEmitter;
use crate::agent::tools::{build_tools, AgentPermissions};
use crate::commands::agent_run::{AgentRunRequest, AgentRunResponse};
use crate::config;
use crate::engine;
use crate::error::AppError;
use crate::types::Memory;

/// 真正的「只读」工具白名单：只查询、不改变系统状态，因此可以并发执行。
///
/// 只要本轮出现任何一个不在名单里的工具（安装、写文件、鼠标键盘、关机…），
/// 整批退回串行 —— 宁可慢一点，也不能让两个安装/写操作互相踩踏。
const READ_ONLY_TOOLS: &[&str] = &[
    "toolbox_agent_disk_space",
    "toolbox_agent_sys_info",
    "toolbox_agent_big_files",
    "toolbox_agent_proc_list",
    "toolbox_agent_service_list",
    "toolbox_agent_startup_list",
    "toolbox_agent_dev_env",
    "toolbox_agent_web_search",
    "toolbox_agent_web_fetch",
    "toolbox_agent_installed_apps",
    "toolbox_agent_winget_search",
    "toolbox_agent_list_dir",
    "toolbox_agent_read_file",
    "toolbox_agent_search_files",
    "toolbox_agent_search_content",
    "toolbox_agent_screenshot",
    "toolbox_agent_pdf_text",
    "toolbox_agent_sheet_read",
    "toolbox_agent_window_list",
    "toolbox_agent_ocr",
    "toolbox_agent_netdiag",
    "toolbox_agent_hardware",
    "toolbox_agent_look",
];

/// 该工具是否只读（可安全并发）
fn is_read_only_tool(name: &str) -> bool {
    READ_ONLY_TOOLS.contains(&name)
}

/// 合并相邻的同角色 user 消息。
///
/// 为什么需要：我们的消息流天然会出现「连续两条 user」——
/// 记忆注入、经验提示、任务拆解都是直接 push 进去的，中间没有 assistant 回应。
/// 后果有两层：
///   1. 语义混乱：两段 user 文字之间没有 assistant 回复，模型容易读串；
///   2. **直接报错**：Anthropic 原生 API 与部分严格的 OpenAI 兼容中转，
///      遇到连续同角色消息会返回 400，整个 Agent 直接跑不起来。
///
/// 只合并 `user`：`assistant` 可能带 tool_calls、`tool` 带 tool_call_id，
/// 合并它们会破坏工具调用协议。
///
/// 非字符串 content（例如多模态数组）不参与合并，原样保留，避免静默丢内容。
fn normalize_roles(messages: &[Value]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::with_capacity(messages.len());
    for m in messages {
        if m.get("role").and_then(|r| r.as_str()) == Some("user") {
            if let Some(last) = out.last_mut() {
                let last_is_user = last.get("role").and_then(|r| r.as_str()) == Some("user");
                if last_is_user {
                    if let (Some(a), Some(b)) = (
                        last.get("content").and_then(|c| c.as_str()),
                        m.get("content").and_then(|c| c.as_str()),
                    ) {
                        *last = json!({ "role": "user", "content": format!("{a}\n\n{b}") });
                        continue;
                    }
                }
            }
        }
        out.push(m.clone());
    }
    out
}

#[cfg(test)]
mod parallel_tests {
    use super::*;

    #[test]
    fn 只读白名单_查询类放行() {
        for t in [
            "toolbox_agent_read_file",
            "toolbox_agent_list_dir",
            "toolbox_agent_disk_space",
            "toolbox_agent_search_content",
            "toolbox_agent_web_search",
            "toolbox_agent_web_fetch",
            "toolbox_agent_ocr",
            "toolbox_agent_hardware",
            "toolbox_agent_netdiag",
            "toolbox_agent_look",
        ] {
            assert!(is_read_only_tool(t), "{t} 应该被判定为只读");
        }
    }

    #[test]
    fn 只读白名单_状态改变类一律拒绝() {
        // 这些绝不能并发执行，否则可能互相踩踏（两个安装、两个写文件…）
        for t in [
            "toolbox_agent_winget_install",
            "toolbox_agent_winget_uninstall",
            "toolbox_agent_download",
            "toolbox_agent_write_file",
            "toolbox_agent_mkdir",
            "toolbox_agent_delete_file",
            "toolbox_agent_run_command",
            "toolbox_agent_power",
            "toolbox_agent_input_keyboard",
            "toolbox_agent_input_mouse",
            "toolbox_agent_window_control",
            "toolbox_agent_registry",
            "toolbox_agent_service",
            "toolbox_agent_schtask",
            "toolbox_agent_script",
            "toolbox_agent_git",
            "toolbox_agent_network",
            "toolbox_agent_compress",
            "toolbox_agent_extract",
        ] {
            assert!(!is_read_only_tool(t), "{t} 不该被当成只读工具并发执行");
        }
    }

    #[test]
    fn 只读白名单_未知工具保守拒绝() {
        // 新增工具若忘了登记，必须退回串行 —— 保守优于冒进
        assert!(!is_read_only_tool("toolbox_agent_brand_new_thing"));
        assert!(!is_read_only_tool(""));
        assert!(!is_read_only_tool("skill_file_search__by_keyword"));
    }

    #[test]
    fn 只读白名单_无重复项() {
        let mut sorted = READ_ONLY_TOOLS.to_vec();
        sorted.sort();
        let n = sorted.len();
        sorted.dedup();
        assert_eq!(n, sorted.len(), "白名单里有重复项");
    }

    #[test]
    fn 归一化_合并连续的_user() {
        // 这正是现实里的消息流：任务 / 经验提示 / 任务拆解 连着三条 user
        let msgs = vec![
            json!({"role": "system", "content": "S"}),
            json!({"role": "user", "content": "任务"}),
            json!({"role": "user", "content": "经验"}),
            json!({"role": "user", "content": "计划"}),
        ];
        let out = normalize_roles(&msgs);
        assert_eq!(out.len(), 2, "三条连续 user 应合并成一条");
        let c = out[1]["content"].as_str().unwrap();
        assert!(c.contains("任务") && c.contains("经验") && c.contains("计划"), "内容不能丢");
    }

    #[test]
    fn 归一化_绝不合并_assistant_与_tool() {
        // assistant 可能带 tool_calls、tool 带 tool_call_id，合并会破坏工具调用协议
        let msgs = vec![
            json!({"role": "user", "content": "U"}),
            json!({"role": "assistant", "content": "A"}),
            json!({"role": "tool", "tool_call_id": "1", "content": "T"}),
            json!({"role": "tool", "tool_call_id": "2", "content": "T2"}),
        ];
        assert_eq!(normalize_roles(&msgs).len(), 4, "非 user 消息一律不许动");
    }

    #[test]
    fn 归一化_非字符串内容不参与合并() {
        // 多模态数组内容若被当成空字符串合并，会把图片静默丢掉
        let msgs = vec![
            json!({"role": "user", "content": [{"type": "text", "text": "看图"}]}),
            json!({"role": "user", "content": "文字"}),
        ];
        assert_eq!(normalize_roles(&msgs).len(), 2, "含非字符串 content 时保持原样");
    }

    #[test]
    fn 归一化_正常交替对话不受影响() {
        let msgs = vec![
            json!({"role": "system", "content": "S"}),
            json!({"role": "user", "content": "你好"}),
            json!({"role": "assistant", "content": "你好呀"}),
            json!({"role": "user", "content": "在吗"}),
        ];
        assert_eq!(normalize_roles(&msgs).len(), 4, "角色本身就交替，不该有任何改动");
    }
}

/// 单条工具结果的最大字符数（超出截断，防上下文爆炸）
const TOOL_RESULT_MAX_CHARS: usize = 2000;

/// 最大迭代步数（run 内部兜底，即使请求未传 max_steps）
const HARD_MAX_STEPS: usize = 20;

/// 工具执行结果摘要化：截断超长文本
fn summarize_tool_result(text: &str) -> String {
    if text.chars().count() <= TOOL_RESULT_MAX_CHARS {
        text.to_string()
    } else {
        let head: String = text.chars().take(TOOL_RESULT_MAX_CHARS).collect();
        format!("{head}\n…（结果过长，已截断）")
    }
}

/// 从配置读取 API 相关参数
fn api_config() -> Result<(String, String, String), AppError> {
    let cfg = config::store::get_config();
    let base = cfg.api_base_url.clone().ok_or_else(|| {
        AppError::ConfigError("未配置 API 地址（Agent 模式需要云端 API）".into())
    })?;
    // 优先加密 key，其次明文 key
    let key = cfg
        .api_key_encrypted
        .clone()
        .or_else(|| cfg.api_key_plain.clone())
        .ok_or_else(|| AppError::ConfigError("未配置 API Key".into()))?;
    let model = cfg.api_model.clone();
    Ok((base, key, model))
}

/// Agent 循环主入口
pub async fn run_agent_loop(
    app: &AppHandle,
    request: AgentRunRequest,
) -> Result<AgentRunResponse, AppError> {
    let (base, key, model) = api_config()?;
    let depth = config::store::get_config().depth;

    // 流式发射器：后续所有进度提示与最终回复都通过它推给前端。
    // progress_events 由请求控制（默认 true），关闭时只推最终回复。
    let emitter = StreamEmitter::new(app.clone(), request.progress_events);

    // 1. 组装工具列表
    // 用 list_agent_items：在普通预设之外，额外包含「Agent 专用工具」
    //（如查磁盘空间/列进程等，前端工具箱 UI 不显示这些，避免界面变乱）
    let toolbox_items = crate::desktop::toolbox::list_agent_items();
    let plugins = crate::plugin::with_manager(|m| m.plugins.clone());
    // 权限开关：危险类工具（下载 / 安装卸载 / 写删文件）需用户在设置页开启后才注入
    let cfg = config::store::get_config();
    let perms = AgentPermissions {
        allow_download: cfg.agent_allow_download,
        allow_software: cfg.agent_allow_software,
        allow_file_write: cfg.agent_allow_file_write,
        allow_shell: cfg.agent_allow_shell,
    };
    let tools = build_tools(&toolbox_items, &plugins, &perms);

    // 2. 拉取相关记忆（默认记忆集，取最近 N 条作为上下文）
    let memories = load_recent_memories(10);

    // 3. 构造 system prompt（含工具说明 + 行为约束）
    let system = build_system_prompt(&tools);

    // 4. 初始化消息流
    let mut messages: Vec<Value> = Vec::new();
    messages.push(json!({ "role": "system", "content": system }));
    for m in &memories {
        messages.push(json!({ "role": m.role, "content": m.content }));
    }
    messages.push(json!({ "role": "user", "content": request.task }));

    // 经验记忆：命中相似任务时，把它上次成功的工具路线注入上下文。
    // 纯文本注入、不额外花 LLM 往返，所以短任务也照样受益。
    let exp_hint = experience::suggest(&request.task);
    if let Some(h) = &exp_hint {
        log::info!("[agent] 命中历史经验，已注入上下文");
        messages.push(json!({ "role": "user", "content": h.clone() }));
    }

    // 5. 循环
    let mut steps = 0usize;
    let max_steps = request.max_steps.min(HARD_MAX_STEPS);
    // 本次任务实际调用过的工具（成功结束后沉淀成经验）
    let mut used_tools: Vec<String> = Vec::new();
    // 动作账本：工具名 → 结果，供「任务完成自评」核对是不是真做成了
    let mut action_log: Vec<String> = Vec::new();
    // 自评只做一次 —— 避免「未通过 → 补一轮 → 又未通过」无限循环
    let mut self_checked = false;
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", crate::utils::normalize_v1_url(&base));
    let (temperature, top_p, _) = engine::apply_depth(depth);

    // 循环前：仅对「看起来是多步」的任务调用 planner
    //（单步任务如「打开QQ」直接跳过，省掉一次 LLM 往返，明显更快）
    let looks_multi_step = request.task.chars().count() > 15
        || ["然后", "并且", "接着", "之后", "同时", "再"]
            .iter()
            .any(|k| request.task.contains(k));

    if looks_multi_step {
        log::info!("[agent] 任务较长，启用多步规划");
        let tool_names: Vec<String> = tools
            .iter()
            .filter_map(|t| {
                t.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        let t_plan = std::time::Instant::now();
        let subtasks = planner::plan_task(
            &client,
            &url,
            &key,
            &model,
            &request.task,
            &tool_names,
            exp_hint.as_deref(),
        )
        .await;
        log::info!(
            "[agent] 规划完成：{} 个子任务，耗时 {}ms",
            subtasks.len(),
            t_plan.elapsed().as_millis()
        );
        if subtasks.len() > 1 {
            // 把子任务列表拼成文字，作为额外 user 上下文注入，帮助 LLM 按步骤执行
            let plan_text = subtasks
                .iter()
                .map(|s| {
                    if let Some(hint) = &s.tool_hint {
                        format!("{}. {}（建议工具：{}）", s.step, s.goal, hint)
                    } else {
                        format!("{}. {}", s.step, s.goal)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            messages.push(json!({
                "role": "user",
                "content": format!("任务已拆解为以下步骤，请依次执行：\n{plan_text}")
            }));
        }
    }

    loop {
        steps += 1;
        if steps > max_steps {
            // 超过步数：强制总结
            messages.push(json!({
                "role": "user",
                "content": "已达到最大工具调用步数，请基于已执行的结果，用自然语言向用户总结当前进度和完成情况，不要再调用任何工具。"
            }));
            // 兜底：这一步只是「生成总结」而不是干活，失败也必须降级返回 ——
            // 原本这里用了 `?`，一旦网络抖动，整个 Agent 请求就变成报错，前端什么内容都收不到。
            let summary = call_llm_text(&client, &url, &key, &model, &messages, temperature, top_p)
                .await
                .unwrap_or_else(|e| {
                    log::warn!("[agent] 超步总结调用失败，降级返回：{e}");
                    format!(
                        "已经连续执行了 {max_steps} 步并停下。上面是实际做过的操作，但我没能生成总结（模型调用失败）。"
                    )
                });
            // 流式推送：进度提示 + 最终总结
            emitter.push_progress("已达到最大工具调用步数，为你总结当前进度…");
            emitter.push_final_reply(&summary);
            return Ok(AgentRunResponse {
                success: true,
                final_reply: Some(summary),
                steps,
                error: None,
            });
        }

        // 调用 LLM（带 tools）
        let t_llm = std::time::Instant::now();
        let resp = call_llm(&client, &url, &key, &model, &messages, &tools, temperature, top_p)
            .await?;
        log::info!(
            "[agent] 第 {}/{} 轮决策完成，LLM 耗时 {}ms",
            steps,
            max_steps,
            t_llm.elapsed().as_millis()
        );

        // 解析：tool_calls 还是纯文本？
        let choice = resp
            .get("choices")
            .and_then(|c| c.get(0))
            .cloned()
            .ok_or_else(|| AppError::InternalError("LLM 响应缺少 choices".into()))?;

        let message = choice.get("message").cloned().unwrap_or(json!({}));

        // 有工具调用？
        let tool_calls = message.get("tool_calls").cloned();
        let has_tool_calls = tool_calls
            .as_ref()
            .and_then(|t| t.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);

        if has_tool_calls {
            // 把 assistant 消息（含 tool_calls）追加进消息流
            messages.push(json!({ "role": "assistant", "content": message.get("content"), "tool_calls": tool_calls }));

            // 逐个执行工具
            let calls = tool_calls.unwrap();
            if let Some(arr) = calls.as_array() {
                // 第一遍：把本轮所有 tool_call 解析出来
                let batch: Vec<(String, String, HashMap<String, Value>)> = arr
                    .iter()
                    .map(|call| {
                        let call_id = call
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let fn_name = call
                            .get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let args_raw = call
                            .get("function")
                            .and_then(|f| f.get("arguments"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("{}")
                            .to_string();
                        let args: HashMap<String, Value> =
                            serde_json::from_str(&args_raw).unwrap_or_default();
                        (call_id, fn_name, args)
                    })
                    .collect();

                // 第二遍：执行。本轮「全是只读工具」才并发，否则严格按顺序串行。
                let can_parallel =
                    batch.len() > 1 && batch.iter().all(|(_, n, _)| is_read_only_tool(n));

                let results: Vec<ToolResult> = if can_parallel {
                    log::info!("[agent] 本轮 {} 个只读工具，并发执行", batch.len());
                    let futs = batch.iter().map(|(_, n, a)| dispatch_tool_call(app, n, a));
                    futures_util::future::join_all(futs).await
                } else {
                    let mut rs = Vec::with_capacity(batch.len());
                    for (_, n, a) in batch.iter() {
                        rs.push(dispatch_tool_call(app, n, a).await);
                    }
                    rs
                };

                // 第三遍：按原顺序把结果回填为 tool 消息
                for ((call_id, fn_name, args), result) in batch.into_iter().zip(results) {
                    used_tools.push(fn_name.clone());

                    // 结果摘要化后回传
                    let content = match result {
                        Ok(text) => summarize_tool_result(&text),
                        Err(e) => {
                            // 诊断错误类型，给 LLM 提供结构化建议
                            let err_str = e.to_string();
                            let kind = recovery::classify_error(&err_str);
                            // HashMap<String,Value> → HashMap<String,String>，供 suggest_fallback 使用
                            let args_str_map: std::collections::HashMap<String, String> = args
                                .iter()
                                .map(|(k, v)| {
                                    let s = v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string());
                                    (k.clone(), s)
                                })
                                .collect();
                            let suggestion = recovery::suggest_fallback(&kind, &fn_name, &args_str_map);
                            let mut msg = format!(
                                "工具执行出错：{err_str}\n错误类型：{}",
                                kind.label()
                            );
                            if let Some(hint) = suggestion {
                                msg.push_str(&format!("\n建议：{hint}"));
                            }
                            msg
                        }
                    };

                    // 记进「动作账本」：任务完成自评要靠它核对「是不是真做成了」
                    action_log.push(format!("{fn_name} → {content}"));

                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "name": fn_name,
                        "content": content,
                    }));
                }
            }
            // 继续循环，让 LLM 看结果
            continue;
        }

        // 纯文本回复 → 最终答案
        let content = message
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if content.is_empty() {
            return Err(AppError::InternalError("LLM 返回空回复".into()));
        }
        // 【任务完成自评】依据 Anthropic 的 evaluator-optimizer 模式：
        // 动手类任务先让独立一次调用核对「是不是真的做成了」，再决定要不要收尾。
        // 纯聊天不触发（used_tools 为空直接跳过）= 零额外开销；
        // 只做一次，未通过就补一轮，补完无论结果如何都收尾 —— 绝不无限循环。
        if cfg.self_check_enabled && !used_tools.is_empty() && !self_checked {
            self_checked = true;
            if let Some(v) =
                evaluator::evaluate(&client, &url, &key, &model, &request.task, &action_log, &content)
                    .await
            {
                if !v.done {
                    log::warn!("[agent] 自检未通过：{}", v.reason);
                    emitter.push_progress("让我再确认一下…");
                    messages.push(json!({
                        "role": "user",
                        "content": format!(
                            "【验收未通过】{}\n请继续把这个任务做完；如果确实做不到，就直说卡在哪里，不要谎报成功。",
                            v.reason
                        )
                    }));
                    continue;
                }
                log::info!("[agent] 自检通过");
            }
        }

        // 流式推送最终回复（漏了这里会导致前端收不到）
        emitter.push_final_reply(&content);

        // 任务正常收尾：把「这个任务 → 用到的工具路线」沉淀成经验，下次少走弯路。
        // 失败 / 超步数的分支不记录，避免把错误路线也学进去。
        if !used_tools.is_empty() {
            experience::record(&request.task, &used_tools);
            log::info!(
                "[agent] 已沉淀经验：本次用 {} 个工具，经验库共 {} 条",
                used_tools.len(),
                experience::count()
            );
        }

        return Ok(AgentRunResponse {
            success: true,
            final_reply: Some(content),
            steps,
            error: None,
        });
    }
}

/// 加载最近 N 条记忆（默认记忆集，作为 Agent 规划上下文）
fn load_recent_memories(limit: usize) -> Vec<Memory> {
    let path = crate::memory::storage::default_index_path();
    let all = crate::memory::storage::read_all(&path).unwrap_or_default();
    // 取最后 N 条（最新在后）
    let start = if all.len() > limit { all.len() - limit } else { 0 };
    all[start..].to_vec()
}

/// 构造 system prompt
fn build_system_prompt(tools: &[Value]) -> String {
    let tool_names: Vec<String> = tools
        .iter()
        .filter_map(|t| {
            t.get("function")
                .and_then(|f| f.get("name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    format!(
        "你是「铃」，一只生活在用户 Windows 电脑里的猫娘助手。你可以调用工具帮用户完成任务。\n\
         \n\
         【可用工具】共 {} 个：{}\n\
         \n\
         【行为准则】\n\
         1. 用户提出任务后，先判断是否需要调用工具；能自己回答的就直接回答。\n\
         2. 需要执行操作时，调用对应的工具，根据工具返回结果判断下一步。\n\
         3. 每一步只调用必要的工具，不要重复调用同一工具。\n\
         4. 完成任务后，用自然、亲切的语气向用户汇报结果。\n\
         5. 遇到工具报错，如实告诉用户，并尝试换一种方式。\n\
         6. 语气自然口语化，像真人聊天，适度使用 emoji。\n\
         7. 【严禁编造·重要】只汇报工具实际返回的结果。工具没被调用、或调用失败时，绝不能声称操作已经完成（例如说「已经帮你打开啦」「窗口应该弹出来了」）——那是在骗主人。必须如实说明「我没能做到，原因是…」。\n\
         8. 【没有合适工具时】如果工具列表里没有能完成该任务的能力，如实告诉用户「我现在的工具做不到这件事」，绝不要假装做了。\n\
         9. 【格式·重要】不要使用 Markdown 符号——星号（*）、井号（#）、反引号、下划线这些都不要用，聊天气泡不渲染它们，只会变成乱糟糟的符号。要强调语气就用 emoji 或颜文字，不要用加粗/斜体标记。\n\
         10. 【动手后必须验证·重要】凡是会改变电脑状态的操作（启动或关闭软件、写删文件、安装卸载、改注册表或设置、关机等），做完之后必须再调用一次只读工具核对结果，确认真的生效了，才可以汇报成功。核对举例：启动软件后查进程或窗口列表；写完文件读回来看内容；安装或卸载后查已安装列表；改完启动项重新读一次注册表。核对没过就如实说明，绝不谎报成功。\n\
         \n\
         【重要】你只能调用上面列出的工具，不要调用不存在的工具。",
        tools.len(),
        tool_names.join("、")
    )
}

/// 调用 LLM（带 tools），返回完整 JSON 响应体
async fn call_llm(
    client: &reqwest::Client,
    url: &str,
    key: &str,
    model: &str,
    messages: &[Value],
    tools: &[Value],
    temperature: f64,
    top_p: f64,
) -> Result<Value, AppError> {
    let mut body = json!({
        "model": model,
        // 发请求前合并连续的 user 消息，避免严格 API 报 400
        "messages": normalize_roles(messages),
        // 规整到 2 位小数（部分中转 API 严格限制，超出即 400）
        "temperature": engine::round2(temperature),
        "top_p": engine::round2(top_p),
    });
    if !tools.is_empty() {
        body["tools"] = json!(tools);
    }

    let resp = client
        .post(url)
        .bearer_auth(key)
        .json(&body)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(AppError::from)?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::NetworkError(format!("API 返回 {status}：{text}")));
    }

    resp.json::<Value>().await.map_err(AppError::from)
}

/// 调用 LLM（纯文本，无 tools），返回文本内容
async fn call_llm_text(
    client: &reqwest::Client,
    url: &str,
    key: &str,
    model: &str,
    messages: &[Value],
    temperature: f64,
    top_p: f64,
) -> Result<String, AppError> {
    let body = json!({
        "model": model,
        // 发请求前合并连续的 user 消息，避免严格 API 报 400
        "messages": normalize_roles(messages),
        // 规整到 2 位小数（部分中转 API 严格限制，超出即 400）
        "temperature": engine::round2(temperature),
        "top_p": engine::round2(top_p),
    });

    let resp = client
        .post(url)
        .bearer_auth(key)
        .json(&body)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(AppError::from)?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::NetworkError(format!("API 返回 {status}：{text}")));
    }

    let v: Value = resp.json().await.map_err(AppError::from)?;
    let content = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    Ok(content)
}