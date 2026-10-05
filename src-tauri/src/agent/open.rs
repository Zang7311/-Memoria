use std::collections::HashMap;

use base64::Engine;
use serde_json::{json, Value};

use super::tools::AgentPermissions;
use crate::error::AppError;
use crate::types::ToolboxItem;

pub const TOOL_NAME: &str = "toolbox_agent_open";

#[derive(Debug, PartialEq, Eq)]
pub struct OpenRoute {
    pub item_id: &'static str,
    pub target: String,
    pub kind: &'static str,
}

pub fn route(target: &str, kind: Option<&str>) -> Result<OpenRoute, AppError> {
    let target = target.trim();
    if target.is_empty() {
        return Err(AppError::ToolboxError(
            "打开目标不能为空，请提供 target".into(),
        ));
    }
    let lower = target.to_ascii_lowercase();
    let inferred = if lower.starts_with("http://") || lower.starts_with("https://") {
        "url"
    } else if std::path::Path::new(target).is_dir() {
        "folder"
    } else {
        "file"
    };
    let kind = match kind.unwrap_or(inferred) {
        "url" => "url",
        "file" => "file",
        "folder" => "folder",
        _ => {
            return Err(AppError::ToolboxError(
                "kind 只能为 url、file 或 folder".into(),
            ))
        }
    };
    let target = if kind == "url" && !lower.starts_with("http://") && !lower.starts_with("https://")
    {
        format!("https://{target}")
    } else {
        target.to_owned()
    };
    Ok(OpenRoute {
        item_id: if kind == "url" {
            "agent_open_url"
        } else {
            "agent_open_path"
        },
        target,
        kind,
    })
}

pub fn from_args(args: &HashMap<String, Value>) -> Result<OpenRoute, AppError> {
    let target = args
        .get("target")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::ToolboxError("target 必须是网址或本地路径字符串".into()))?;
    let kind = match args.get("kind") {
        None => None,
        Some(value) => Some(
            value
                .as_str()
                .ok_or_else(|| AppError::ToolboxError("kind 必须是字符串".into()))?,
        ),
    };
    route(target, kind)
}

pub fn authorized_item<'a>(
    route: &OpenRoute,
    items: &'a [ToolboxItem],
    perms: &AgentPermissions,
) -> Result<&'a ToolboxItem, AppError> {
    let item = items
        .iter()
        .find(|item| item.id == route.item_id)
        .ok_or_else(|| AppError::ToolboxError("对应打开能力不可用".into()))?;
    if !item.enabled || !perms.allows(item.agent_permission.as_deref()) {
        return Err(AppError::PermissionDenied(format!(
            "打开 {} 需要对应权限，未授权或已禁用，已拒绝",
            route.kind
        )));
    }
    Ok(item)
}

pub fn tool_definition(tools: &[Value]) -> Option<Value> {
    let mut kinds = Vec::new();
    let mut routes = Vec::new();
    for tool in tools.iter().filter(|tool| tool["x-alias-for"] == TOOL_NAME) {
        let usage = if tool["function"]["name"] == "toolbox_agent_open_url" {
            kinds.push("url");
            "kind=url"
        } else {
            kinds.extend(["file", "folder"]);
            "kind=file|folder"
        };
        routes.push(json!({
            "intent": tool["x-intent"],
            "usage": usage,
            "agent_permission": tool["x-agent-permission"],
        }));
    }
    if routes.is_empty() {
        return None;
    }
    Some(json!({
        "type": "function",
        "x-intent": "open.target",
        "x-source": "agent",
        "x-routes": routes,
        "function": {
            "name": TOOL_NAME,
            "description": "用默认浏览器打开网址，或用默认程序打开本地文件、文件夹；省略 kind 时自动推断。不要用于按名称查找软件，请用 toolbox_agent_open_app；任意命令请用 toolbox_agent_run_command（需授权）。",
            "parameters": {
                "type": "object",
                "properties": {
                    "target": {"type": "string", "description": "网址或本地完整路径"},
                    "kind": {"type": "string", "enum": kinds, "description": "可省略；http(s) 地址按 url，其余按本地文件或文件夹"},
                },
                "required": ["target"],
            },
        },
    }))
}

pub const SCRIPT: &str = r#"[Console]::OutputEncoding=[System.Text.Encoding]::UTF8
$ErrorActionPreference = "Stop"
$request = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($env:TOOLBOX_INPUT)) | ConvertFrom-Json
if ($request.kind -eq "url") {
    Start-Process $request.target
    "Opened in default browser: $($request.target)"
} else {
    if (-not (Test-Path -LiteralPath $request.target)) { throw "Not found: $($request.target)" }
    $item = Get-Item -LiteralPath $request.target
    if (($request.kind -eq "folder") -ne [bool]$item.PSIsContainer) { throw "目标类型与 kind 不符" }
    Invoke-Item -LiteralPath $request.target
    "Opened: $($request.target)"
}"#;

pub fn execution(route: &OpenRoute) -> (String, String) {
    let bytes: Vec<u8> = SCRIPT.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    (
        format!("powershell -NoProfile -EncodedCommand {encoded}"),
        json!({"target": route.target, "kind": route.kind}).to_string(),
    )
}
