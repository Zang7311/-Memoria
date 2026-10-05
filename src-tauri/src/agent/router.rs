// 《铃·记忆体》AI-10 Agent 路由器（router.rs）
//
// 职责：根据 LLM 返回的 tool_call name + args，路由到对应执行器并返回结果文本。
//
// 三类路由：
//   1. toolbox_<id>  → toolbox::execute（复用现有工具箱执行器，走 cmd /C + 30s 超时）
//   2. skill_<pid>__<name> → plugin runner（复用现有插件技能执行器）
//   3. 其他 → 返回错误，避免 LLM 调用不存在的工具
//
// 例外：内置原生工具（toolbox_agent_look）在进入分类前先拦截，由 Rust 侧直接处理。
// 设计要点：不动现有执行器，只做路由 + 参数适配 + 结果包装。
use std::collections::HashMap;

use serde_json::Value;
use tauri::AppHandle;

use crate::agent::tools::classify_tool;
use crate::error::AppError;
use crate::types::{ExecuteSkillRequest, ExecuteToolboxRequest};

/// 工具执行结果（纯文本，直接回传给 LLM）
pub type ToolResult = Result<String, AppError>;

/// 路由执行一个 tool_call
///
/// # 参数
/// - `app`: Tauri AppHandle（复用现有执行器需要）
/// - `name`: 工具名（如 "toolbox_clean-temp"）
/// - `args`: LLM 传入的参数（HashMap）
pub async fn dispatch_tool_call(
    app: &AppHandle,
    name: &str,
    args: &HashMap<String, Value>,
) -> ToolResult {
    if name == super::open::TOOL_NAME {
        let route = super::open::from_args(args)?;
        let items = crate::desktop::toolbox::list_agent_items();
        let item = super::open::authorized_item(&route, &items, &current_permissions())?;
        let response = crate::desktop::toolbox::execute_open(item, &route).await?;
        return Ok(if response.success {
            response.output.unwrap_or_else(|| "执行成功（无输出）".into())
        } else {
            format!("执行失败：{}", response.error.unwrap_or_default())
        });
    }
    if ["create_goal", "goal_status", "goal_advance"].contains(&name) {
        return Box::pin(crate::agent::goals::dispatch(app, name, args)).await;
    }
    // 内置原生工具：看图。Rust 侧直连多模态模型，不走 PowerShell 执行器
    if name == "toolbox_agent_look" || name == "agent_look" {
        return dispatch_look(args).await;
    }

    match classify_tool(name) {
        crate::agent::tools::ToolKind::Toolbox(item_id) => {
            dispatch_toolbox(app, &item_id, args).await
        }
        crate::agent::tools::ToolKind::Skill(_plugin_id, skill_name) => {
            dispatch_skill(&skill_name, args).await
        }
        crate::agent::tools::ToolKind::Unknown => Err(AppError::ToolboxError(format!(
            "未知工具：{name}（请从工具列表中选择）"
        ))),
    }
}

pub(super) async fn dispatch_sub_agent_tool_call(
    name: &str,
    args: &HashMap<String, Value>,
) -> ToolResult {
    if name == "toolbox_agent_look" {
        return dispatch_look(args).await;
    }
    let presets: Vec<crate::types::ToolboxItem> = serde_json::from_str(
        include_str!("../../resources/agent_tools.json"),
    ).map_err(|_| AppError::ToolboxError("子 Agent 工具预设不可用".into()))?;
    let item = presets.iter().find(|item| name == format!("toolbox_{}", item.id))
        .ok_or_else(|| AppError::ToolboxError("子 Agent 无权调用此工具，已拒绝".into()))?;
    let input = args.get("input").and_then(Value::as_str).map(str::to_owned);
    let response = crate::desktop::toolbox::execute(item, input).await?;
    if response.success {
        Ok(response.output.unwrap_or_else(|| "执行成功（无输出）".into()))
    } else {
        Ok(format!("执行失败：{}", response.error.unwrap_or_default()))
    }
}

/// 路由到工具箱执行器
///
/// 参数适配：LLM 传入的 `input` 字符串 → ExecuteToolboxRequest.input
/// 其他字段忽略（confirm 默认 false，Agent 不触发危险工具，已在 tools.rs 层面过滤）
async fn dispatch_toolbox(
    app: &AppHandle,
    item_id: &str,
    args: &HashMap<String, Value>,
) -> ToolResult {
    let items = crate::desktop::toolbox::list_agent_items();
    let item = items.iter().find(|item| item.id == item_id)
        .ok_or_else(|| AppError::ToolboxError(format!("工具箱条目不存在：{item_id}")))?;
    authorize_toolbox(item, &current_permissions())?;
    let input = args.get("input").and_then(|v| v.as_str()).map(|s| s.to_string());

    let resp = crate::commands::toolbox_execute::execute_toolbox(
        app.clone(),
        ExecuteToolboxRequest {
            item_id: item_id.to_string(),
            input,
            confirm: false,
        },
    )
    .await?;

    if resp.success {
        Ok(resp.output.unwrap_or_else(|| "执行成功（无输出）".to_string()))
    } else {
        Ok(format!("执行失败：{}", resp.error.unwrap_or_default()))
    }
}

fn current_permissions() -> super::tools::AgentPermissions {
    let cfg = crate::config::store::get_config();
    super::tools::AgentPermissions {
        allow_download: cfg.agent_allow_download,
        allow_software: cfg.agent_allow_software,
        allow_file_write: cfg.agent_allow_file_write,
        allow_shell: cfg.agent_allow_shell,
        allow_tool_forge: cfg.agent_allow_tool_forge,
    }
}

pub(super) fn authorize_toolbox(
    item: &crate::types::ToolboxItem,
    perms: &super::tools::AgentPermissions,
) -> Result<(), AppError> {
    if !item.enabled || !perms.allows(item.agent_permission.as_deref()) {
        return Err(AppError::PermissionDenied(format!("工具 {} 未授权或已禁用，已拒绝", item.id)));
    }
    Ok(())
}

/// 路由到插件技能执行器
///
/// 参数适配：LLM 传入的参数 → ExecuteSkillRequest.params
async fn dispatch_skill(
    skill_name: &str,
    args: &HashMap<String, Value>,
) -> ToolResult {
    let resp = crate::commands::plugin_execute::execute_skill(
        ExecuteSkillRequest {
            skill_name: skill_name.to_string(),
            params: args.clone(),
        },
    )
    .await;

    if resp.success {
        Ok(resp.result.unwrap_or_else(|| "执行成功（无输出）".to_string()))
    } else {
        Ok(format!("执行失败：{}", resp.error.unwrap_or_default()))
    }
}

/// 内置工具「看图」：把本地图片交给视觉模型理解
///
/// 参数：`path`（必填，图片完整路径）、`question`（可选，想问的问题）
async fn dispatch_look(args: &HashMap<String, Value>) -> ToolResult {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AppError::ToolboxError(
                "看图需要给出 path（图片路径），可先用 agent_screenshot 截屏".into(),
            )
        })?;

    let question = args.get("question").and_then(|v| v.as_str());

    crate::agent::vision::look(path, question).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn 看图_缺少path时报错() {
        let args: HashMap<String, Value> = HashMap::new();
        let r = dispatch_look(&args).await;
        assert!(r.is_err());
        let msg = format!("{}", r.unwrap_err());
        assert!(msg.contains("path"), "错误信息应提示 path，实际：{msg}");
    }

    #[tokio::test]
    async fn 看图_空白path也报错() {
        let mut args: HashMap<String, Value> = HashMap::new();
        args.insert("path".into(), Value::String("   ".into()));
        assert!(dispatch_look(&args).await.is_err());
    }

    #[tokio::test]
    async fn 看图_文件不存在时给出可读错误() {
        let mut args: HashMap<String, Value> = HashMap::new();
        args.insert(
            "path".into(),
            Value::String("C:\\__ling_not_exist__\\nope.png".into()),
        );
        let r = dispatch_look(&args).await;
        assert!(r.is_err());
        assert!(format!("{}", r.unwrap_err()).contains("图片不存在"));
    }

    #[test]
    fn 分类_工具箱() {
        assert_eq!(
            classify_tool("toolbox_clean-temp"),
            crate::agent::tools::ToolKind::Toolbox("clean-temp".into())
        );
    }

    #[test]
    fn 分类_插件技能() {
        assert_eq!(
            classify_tool("skill_file_search__by_keyword"),
            crate::agent::tools::ToolKind::Skill(
                "file_search".into(),
                "by_keyword".into()
            )
        );
    }

    #[test]
    fn 分类_未知工具() {
        assert_eq!(classify_tool("foo_bar"), crate::agent::tools::ToolKind::Unknown);
    }
}
