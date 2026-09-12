// 《铃·记忆体》AI-10 Agent 命令层：agent_run
//
// 这是 Agent 能力对外的唯一入口。前端调用 agent_run 后：
//   1. 拉取工具箱条目 + 已启用插件 → 生成 OpenAI tools 列表
//   2. 拉取相关记忆 → 作为规划上下文
//   3. 调用 API 模式引擎，注入 tools 参数，开启 function calling 循环
//   4. 工具调用由本模块路由到 toolbox_execute / plugin_execute / quick_command
//   5. 循环直到 LLM 返回纯文本最终回复，通过流式事件推送给前端
//
// 注意：Agent 循环本身在 agent::loop 模块实现，本文件只负责参数组装与事件推送。
use tauri::AppHandle;

use crate::agent::loop_::run_agent_loop;
use crate::error::AppError;

/// Agent 任务请求
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentRunRequest {
    /// 用户的自然语言任务描述
    pub task: String,
    /// 任务唯一标识（前端生成，供 agent_cancel 定位）
    #[serde(default = "default_request_id")]
    pub request_id: String,
    /// 最大工具调用步数（防止无限循环，默认 10）
    #[serde(default = "default_max_steps")]
    pub max_steps: usize,
    /// 是否在每次工具调用后向前端推送进度事件（默认 true）
    #[serde(default = "default_progress_events")]
    pub progress_events: bool,
}

fn default_request_id() -> String {
    // 未传时用时间戳兜底，保证唯一性
    format!("agent_{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis())
}
fn default_max_steps() -> usize {
    10
}
fn default_progress_events() -> bool {
    true
}

/// Agent 任务响应（最终结果，流式内容已通过事件推送）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentRunResponse {
    pub success: bool,
    pub final_reply: Option<String>,
    pub steps: usize,
    pub error: Option<String>,
    /// 是否被用户主动中断（true = 用户点了停止，不是错误）
    pub interrupted: bool,
}

/// Agent 任务入口（Tauri 命令）
///
/// 前端调用：invoke('agent_run', { task: '整理桌面' })
#[tauri::command]
pub async fn agent_run(
    app: AppHandle,
    request: AgentRunRequest,
) -> Result<AgentRunResponse, AppError> {
    run_agent_loop(&app, request).await
}