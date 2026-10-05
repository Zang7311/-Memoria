use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde_json::{json, Value};

pub fn definitions() -> &'static [Value] {
    static DEFINITIONS: OnceLock<Vec<Value>> = OnceLock::new();
    DEFINITIONS.get_or_init(|| {
        [
            include_str!("../../resources/agent_tools.json"),
            include_str!("../../resources/toolbox_presets.json"),
        ]
        .into_iter()
        .flat_map(|text| serde_json::from_str::<Vec<Value>>(text).expect("工具意图定义格式错误"))
        .collect()
    })
}

pub fn decorate_toolbox(mut tool: Value, id: &str, permission: Option<&str>) -> Value {
    let definition = definitions()
        .iter()
        .find(|definition| definition["id"] == id);
    if let Some(definition) = definition {
        tool["x-intent"] = definition["intent"].clone();
        tool["function"]["description"] = definition["description"].clone();
        if let Some(alias) = definition["alias_for"].as_str() {
            tool["x-alias-for"] = json!(alias);
        }
    } else {
        tool["x-intent"] = json!("toolbox.custom");
        let description = tool["function"]["description"].as_str().unwrap_or("");
        tool["function"]["description"] = json!(format!(
            "{}。不要用于其他自定义流程，请用对应工具箱工具。",
            description.chars().take(90).collect::<String>()
        ));
    }
    tool["x-source"] = json!(if id.starts_with("agent_") || id == "spawn_sub_agents" {
        "agent"
    } else {
        "toolbox"
    });
    tool["x-agent-permission"] = json!(permission);
    tool
}

pub fn api_tools(tools: &[Value]) -> Vec<Value> {
    tools
        .iter()
        .filter(|tool| tool["x-alias-for"].is_null())
        .map(|tool| {
            json!({"type": "function", "function": {
                "name": tool["function"]["name"],
                "description": tool["function"]["description"],
                "parameters": tool["function"]["parameters"],
            }})
        })
        .collect()
}

fn permission_suffix(permission: Option<&str>) -> String {
    permission
        .map(|permission| format!("（需 {permission} 授权）"))
        .unwrap_or_default()
}

pub fn intent_rows(tools: &[Value], include_toolbox: bool) -> BTreeMap<String, BTreeSet<String>> {
    let mut rows: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for tool in tools.iter().filter(|tool| tool["x-alias-for"].is_null()) {
        let name = tool["function"]["name"].as_str().unwrap_or("");
        let intent = tool["x-intent"].as_str().unwrap_or("tool.other");
        rows.entry(intent.to_owned()).or_default().insert(format!(
            "{name}{}",
            permission_suffix(tool["x-agent-permission"].as_str())
        ));
        if let Some(routes) = tool["x-routes"].as_array() {
            for route in routes {
                if let Some(intent) = route["intent"].as_str() {
                    rows.entry(intent.to_owned()).or_default().insert(format!(
                        "{name}({}){}",
                        route["usage"].as_str().unwrap_or(""),
                        permission_suffix(route["agent_permission"].as_str())
                    ));
                }
            }
        }
    }
    if include_toolbox {
        let agent_definitions: Vec<Value> =
            serde_json::from_str(include_str!("../../resources/agent_tools.json"))
                .expect("Agent 意图定义格式错误");
        for definition in agent_definitions {
            let id = definition["id"].as_str().unwrap_or("");
            let name = if id == "spawn_sub_agents" {
                id.to_owned()
            } else {
                format!("toolbox_{id}")
            };
            if !tools.iter().any(|tool| tool["function"]["name"] == name) {
                if let Some(intent) = definition["intent"].as_str() {
                    let name = definition["alias_for"].as_str().unwrap_or(&name);
                    rows.entry(intent.to_owned()).or_default().insert(format!(
                        "{name}（未开放）{}",
                        permission_suffix(definition["agent_permission"].as_str())
                    ));
                }
            }
        }
        let presets: Vec<Value> =
            serde_json::from_str(include_str!("../../resources/toolbox_presets.json"))
                .expect("工具箱意图定义格式错误");
        for preset in presets {
            let name = format!("toolbox_{}", preset["id"].as_str().unwrap_or(""));
            if !tools.iter().any(|tool| tool["function"]["name"] == name) {
                if let Some(intent) = preset["intent"].as_str() {
                    rows.entry(intent.to_owned()).or_default().insert(format!(
                        "{}：{}（工具箱手动）",
                        preset["id"].as_str().unwrap_or(""),
                        preset["name"].as_str().unwrap_or("")
                    ));
                }
            }
        }
    }
    rows
}

pub fn generate_intent_index(tools: &[Value]) -> String {
    let entries: Vec<String> = intent_rows(tools, true)
        .into_iter()
        .map(|(intent, names)| {
            format!(
                "{intent} → {}",
                names.into_iter().collect::<Vec<_>>().join(" / ")
            )
        })
        .collect();
    let per_line = ((entries.len() + 37) / 38).max(1);
    let mut lines = vec![
        "【工具意图速查】".to_owned(),
        "旧打开入口兼容调用；未开放须在设置中授权或启用；工具箱手动项不能由 Agent 直接调用。"
            .to_owned(),
    ];
    lines.extend(entries.chunks(per_line).map(|chunk| chunk.join("；")));
    lines.join("\n")
}

#[cfg(test)]
mod tests;
