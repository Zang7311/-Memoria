use std::collections::HashMap;
use std::path::PathBuf;

use base64::Engine;
use sha2::{Digest, Sha256};

use super::*;
use crate::agent::open;
use crate::agent::tools::{build_tools, AgentPermissions};
use crate::error::AppError;
use crate::types::ToolboxItem;

fn presets(text: &str) -> Vec<ToolboxItem> {
    serde_json::from_str(text).unwrap()
}

fn agent_items() -> Vec<ToolboxItem> {
    presets(include_str!("../../../resources/agent_tools.json"))
}

fn all_permissions() -> AgentPermissions {
    AgentPermissions {
        allow_download: true,
        allow_software: true,
        allow_file_write: true,
        allow_shell: true,
        allow_tool_forge: true,
    }
}

fn all_tools() -> Vec<Value> {
    let mut items = agent_items();
    items.extend(presets(include_str!(
        "../../../resources/toolbox_presets.json"
    )));
    let mut tools = build_tools(&items, &[], &all_permissions());
    tools.extend(crate::agent::forged_tools::tool_definitions(true));
    tools.extend(crate::agent::goals::tool_definitions());
    tools
}

#[test]
fn all_fifty_three_definitions_have_intent_and_chinese_exclusions() {
    let definitions: Vec<Value> =
        serde_json::from_str(include_str!("../../../resources/agent_tools.json")).unwrap();
    assert_eq!(definitions.len(), 53);
    for definition in definitions {
        let intent = definition["intent"].as_str().unwrap();
        assert!(!intent.trim().is_empty(), "{}", definition["id"]);
        assert!(intent.contains('.'));
        let description = definition["description"].as_str().unwrap();
        assert!(
            description.contains("不要") && description.contains("请用"),
            "{}",
            definition["id"]
        );
        assert!(description.chars().count() <= 120, "{}", definition["id"]);
    }
}

#[test]
fn emitted_tools_keep_all_source_intents_and_exclusions() {
    let tools = all_tools();
    for definition in definitions() {
        let name = if definition["id"] == "spawn_sub_agents" {
            "spawn_sub_agents".to_owned()
        } else {
            format!("toolbox_{}", definition["id"].as_str().unwrap())
        };
        if let Some(tool) = tools.iter().find(|tool| tool["function"]["name"] == name) {
            assert_eq!(tool["x-intent"], definition["intent"]);
            assert_eq!(tool["function"]["description"], definition["description"]);
        } else {
            assert!(!definition["id"].as_str().unwrap().starts_with("agent_"));
        }
    }
    for tool in tools {
        assert!(!tool["x-intent"].as_str().unwrap().trim().is_empty());
        let description = tool["function"]["description"].as_str().unwrap();
        assert!(description.contains("不要") && description.contains("请用"));
        assert!(
            description.chars().count() <= 120,
            "{}",
            tool["function"]["name"]
        );
    }
}

#[test]
fn duplicate_intents_always_disambiguate_in_descriptions() {
    let mut grouped: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for definition in definitions() {
        grouped
            .entry(definition["intent"].as_str().unwrap())
            .or_default()
            .push(definition);
    }
    let mut duplicates = 0;
    for group in grouped.values().filter(|group| group.len() > 1) {
        duplicates += 1;
        for definition in group {
            let description = definition["description"].as_str().unwrap();
            assert!(
                description.contains("不要") && description.contains("请用"),
                "{}",
                definition["id"]
            );
        }
    }
    assert!(duplicates >= 5);
}

#[test]
fn compatibility_snapshot_preserves_every_original_field_and_permission() {
    let baseline: Value =
        serde_json::from_str(include_str!("compatibility_baseline.json")).unwrap();
    for (key, text) in [
        (
            "agent_tools",
            include_str!("../../../resources/agent_tools.json"),
        ),
        (
            "toolbox_presets",
            include_str!("../../../resources/toolbox_presets.json"),
        ),
    ] {
        let current: Vec<Value> = serde_json::from_str(text).unwrap();
        assert_eq!(current.len(), baseline[key].as_array().unwrap().len());
        for mut definition in current {
            let id = definition["id"].as_str().unwrap().to_owned();
            for field in ["intent", "description", "alias_for"] {
                definition.as_object_mut().unwrap().remove(field);
            }
            let digest = format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&definition).unwrap())
            );
            let original = baseline[key]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row[0] == id)
                .unwrap();
            assert_eq!(digest, original[1].as_str().unwrap(), "{id}");
        }
    }
}

#[test]
fn index_covers_every_intent_and_toolbox_only_capability() {
    let tools = all_tools();
    let index = generate_intent_index(&tools);
    for definition in definitions() {
        assert!(
            index.contains(definition["intent"].as_str().unwrap()),
            "{}",
            definition["id"]
        );
    }
    for tool in &tools {
        assert!(index.contains(tool["x-intent"].as_str().unwrap()));
    }
    assert!(index.contains("工具箱手动"));
    assert!(index.contains("格式化硬盘（工具箱手动）"));
    assert!(index.contains("像素画板（工具箱手动）"));
    assert!(index.lines().count() <= 40);
}

#[test]
fn default_permissions_keep_declarations_closed_but_all_intents_discoverable() {
    let tools = build_tools(&agent_items(), &[], &AgentPermissions::default());
    let index = generate_intent_index(&tools);
    for definition in definitions() {
        assert!(index.contains(definition["intent"].as_str().unwrap()));
    }
    assert!(index.contains("toolbox_agent_run_command（未开放）（需 shell 授权）"));
    assert!(!api_tools(&tools)
        .iter()
        .any(|tool| tool["function"]["name"] == "toolbox_agent_run_command"));
    assert!(index.lines().count() <= 40);
}

#[test]
fn index_is_definition_driven_and_deterministic() {
    let mut tools = all_tools();
    let before = generate_intent_index(&tools);
    tools.reverse();
    assert_eq!(generate_intent_index(&tools), before);
    tools.push(json!({"x-intent":"future.new_capability","function":{"name":"future_tool"}}));
    let after = generate_intent_index(&tools);
    assert!(after.contains("future.new_capability → future_tool"));
    assert!(!before.contains("future.new_capability"));
    tools.last_mut().unwrap()["function"]["name"] = json!("renamed_future_tool");
    assert!(generate_intent_index(&tools).contains("future.new_capability → renamed_future_tool"));
}

#[test]
fn index_stays_bounded_with_many_plugin_intents() {
    let tools: Vec<Value> = (0..1000)
        .map(|index| {
            json!({
                "x-intent": format!("plugin.test{index}"),
                "function": {"name": format!("skill_test{index}")},
            })
        })
        .collect();
    let index = generate_intent_index(&tools);
    assert!(index.lines().count() <= 40);
    for tool in tools {
        assert!(index.contains(tool["x-intent"].as_str().unwrap()));
    }
}

#[test]
fn api_advertises_one_open_entry_but_retains_both_legacy_aliases() {
    let tools = all_tools();
    for legacy in ["toolbox_agent_open_url", "toolbox_agent_open_path"] {
        let tool = tools
            .iter()
            .find(|tool| tool["function"]["name"] == legacy)
            .unwrap();
        assert_eq!(tool["x-alias-for"], open::TOOL_NAME);
        assert!(!api_tools(&tools)
            .iter()
            .any(|tool| tool["function"]["name"] == legacy));
        assert!(matches!(
            crate::agent::tools::classify_tool(legacy),
            crate::agent::tools::ToolKind::Toolbox(_)
        ));
        let id = legacy.strip_prefix("toolbox_").unwrap();
        assert!(crate::desktop::toolbox::find_item(std::path::Path::new(""), id).is_some());
    }
    let api = api_tools(&tools);
    let merged = api
        .iter()
        .find(|tool| tool["function"]["name"] == open::TOOL_NAME)
        .unwrap();
    assert_eq!(
        merged["function"]["parameters"]["properties"]["kind"]["enum"],
        json!(["url", "file", "folder"])
    );
    let index = generate_intent_index(&tools);
    assert!(index.contains("open.url → toolbox_agent_open(kind=url)"));
    assert!(index.contains("open.path → toolbox_agent_open(kind=file|folder)"));
}

#[test]
fn api_projection_never_sends_internal_taxonomy_fields() {
    for tool in api_tools(&all_tools()) {
        assert_eq!(tool.as_object().unwrap().len(), 2);
        assert_eq!(tool["function"].as_object().unwrap().len(), 3);
        assert!(tool["x-intent"].is_null());
        assert!(tool["x-alias-for"].is_null());
    }
}

#[test]
fn advertised_capabilities_preserve_every_non_duplicate_tool_schema() {
    let tools = all_tools();
    let published = api_tools(&tools);
    for tool in tools.iter().filter(|tool| tool["x-alias-for"].is_null()) {
        let name = tool["function"]["name"].as_str().unwrap();
        let advertised = published
            .iter()
            .find(|published| published["function"]["name"] == name)
            .unwrap();
        assert_eq!(
            advertised["function"]["parameters"], tool["function"]["parameters"],
            "{name}"
        );
    }
    assert_eq!(tools.len() - published.len(), 2);
    assert!(published
        .iter()
        .any(|tool| tool["function"]["name"] == "toolbox_agent_open_app"));
    assert!(published
        .iter()
        .any(|tool| tool["function"]["name"] == "toolbox_open-app"));
    assert!(published
        .iter()
        .any(|tool| tool["function"]["name"] == "toolbox_open-file"));
}

#[test]
fn plugin_and_custom_tools_generate_their_own_navigation_entries() {
    let plugin = crate::types::Plugin {
        id: "example".into(),
        name: "示例插件".into(),
        version: "1.0".into(),
        author: "测试".into(),
        description: "示例技能".into(),
        enabled: true,
        path: ".".into(),
        granted: vec![],
        manifest: crate::types::PluginManifest {
            main: "index.js".into(),
            permissions: vec![],
            hermes_compatible: false,
            skills: vec![crate::types::Skill {
                name: "inspect".into(),
                description: "检查插件数据".into(),
                parameters: vec![],
                action: "js:inspect".into(),
            }],
        },
    };
    let mut custom = agent_items().remove(1);
    custom.id = "user_example".into();
    custom.name = "执行自定义检查".into();
    let tools = build_tools(&[custom], &[plugin], &AgentPermissions::default());
    let index = generate_intent_index(&tools);
    assert!(index.contains("plugin.example.inspect → skill_example__inspect"));
    assert!(index.contains("toolbox.custom → toolbox_user_example"));
    for tool in tools {
        let description = tool["function"]["description"].as_str().unwrap();
        assert!(description.contains("不要") && description.contains("请用"));
        assert!(description.chars().count() <= 120);
    }
}

#[test]
fn main_and_child_system_prompts_inject_generated_navigation() {
    let tools = all_tools();
    let index = generate_intent_index(&tools);
    let prompt = crate::agent::loop_::build_system_prompt(&tools);
    assert!(prompt.contains(&index));
    let task = crate::agent::sub_agents::SubTask {
        goal: "查询系统信息".into(),
        context: String::new(),
        truncated: false,
    };
    let messages = crate::agent::sub_agents::child_messages(&task, &tools);
    assert!(messages[0]["content"].as_str().unwrap().contains(&index));
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("ling-tool-taxonomy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let resolved = self.0.canonicalize().unwrap();
        let temp = std::env::temp_dir().canonicalize().unwrap();
        assert_eq!(resolved.parent(), Some(temp.as_path()));
        assert!(resolved
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("ling-tool-taxonomy-"));
        std::fs::remove_dir_all(resolved).unwrap();
    }
}

#[test]
fn merged_routes_cover_url_file_folder_and_inference() {
    let scratch = Scratch::new();
    let file = scratch.0.join("样本'[1];.txt");
    std::fs::write(&file, "样本").unwrap();
    for url in [
        "http://example.test",
        "https://example.test",
        "HTTPS://example.test",
    ] {
        let route = open::route(url, None).unwrap();
        assert_eq!(route.kind, "url");
        assert_eq!(route.item_id, "agent_open_url");
        assert_eq!(route.target, url);
    }
    for (path, kind) in [(file.as_path(), "file"), (scratch.0.as_path(), "folder")] {
        for explicit in [None, Some(kind)] {
            let route = open::route(path.to_str().unwrap(), explicit).unwrap();
            assert_eq!(route.kind, kind);
            assert_eq!(route.item_id, "agent_open_path");
            assert_eq!(route.target, path.to_str().unwrap());
        }
    }
    assert_eq!(
        open::route(" example.test ", Some("url")).unwrap().target,
        "https://example.test"
    );
}

#[test]
fn malformed_open_arguments_are_rejected() {
    for args in [
        HashMap::new(),
        HashMap::from([("target".into(), json!(" "))]),
        HashMap::from([("target".into(), json!(42))]),
        HashMap::from([("target".into(), json!("path")), ("kind".into(), json!(42))]),
        HashMap::from([
            ("target".into(), json!("path")),
            ("kind".into(), json!("app")),
        ]),
    ] {
        assert!(matches!(
            open::from_args(&args),
            Err(AppError::ToolboxError(_))
        ));
    }
}

#[test]
fn merged_branches_preserve_independent_permission_granularity() {
    let mut items = agent_items();
    items
        .iter_mut()
        .find(|item| item.id == "agent_open_url")
        .unwrap()
        .agent_permission = Some("download".into());
    items
        .iter_mut()
        .find(|item| item.id == "agent_open_path")
        .unwrap()
        .agent_permission = Some("file_write".into());
    let url = open::route("https://example.test", Some("url")).unwrap();
    let file = open::route(r"C:\example.txt", Some("file")).unwrap();
    let folder = open::route(r"C:\example", Some("folder")).unwrap();
    for route in [&url, &file, &folder] {
        assert!(matches!(
            open::authorized_item(route, &items, &AgentPermissions::default()),
            Err(AppError::PermissionDenied(_))
        ));
    }
    let download = AgentPermissions {
        allow_download: true,
        ..Default::default()
    };
    assert!(open::authorized_item(&url, &items, &download).is_ok());
    assert!(open::authorized_item(&file, &items, &download).is_err());
    let write = AgentPermissions {
        allow_file_write: true,
        ..Default::default()
    };
    assert!(open::authorized_item(&url, &items, &write).is_err());
    assert!(open::authorized_item(&file, &items, &write).is_ok());
    assert!(open::authorized_item(&folder, &items, &write).is_ok());
    for (perms, expected) in [
        (AgentPermissions::default(), None),
        (download, Some(json!(["url"]))),
        (write, Some(json!(["file", "folder"]))),
    ] {
        let tools = build_tools(&items, &[], &perms);
        let merged = tools
            .iter()
            .find(|tool| tool["function"]["name"] == open::TOOL_NAME);
        match expected {
            None => assert!(merged.is_none()),
            Some(kinds) => assert_eq!(
                merged.unwrap()["function"]["parameters"]["properties"]["kind"]["enum"],
                kinds
            ),
        }
    }
}

#[test]
fn original_dangerous_paths_still_reject_unauthorized_execution() {
    let items = agent_items();
    let mut checked = 0;
    for item in items.iter().filter(|item| item.agent_permission.is_some()) {
        checked += 1;
        assert!(matches!(
            crate::agent::router::authorize_toolbox(item, &AgentPermissions::default()),
            Err(AppError::PermissionDenied(_))
        ));
        assert!(crate::agent::router::authorize_toolbox(item, &all_permissions()).is_ok());
    }
    assert_eq!(checked, 17);
}

#[test]
fn unknown_or_disabled_open_permissions_fail_closed() {
    let mut items = agent_items();
    let route = open::route("https://example.test", None).unwrap();
    let item = items
        .iter_mut()
        .find(|item| item.id == route.item_id)
        .unwrap();
    item.agent_permission = Some("future_permission".into());
    assert!(matches!(
        open::authorized_item(&route, &items, &all_permissions()),
        Err(AppError::PermissionDenied(_))
    ));
    let item = items
        .iter_mut()
        .find(|item| item.id == route.item_id)
        .unwrap();
    item.agent_permission = None;
    item.enabled = false;
    assert!(matches!(
        open::authorized_item(&route, &items, &all_permissions()),
        Err(AppError::PermissionDenied(_))
    ));
}

#[cfg(windows)]
async fn mocked_open(route: &open::OpenRoute) -> std::process::Output {
    let (command, input) = open::execution(route);
    let encoded = command.split_whitespace().last().unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .unwrap();
    let utf16: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
        .collect();
    assert_eq!(String::from_utf16(&utf16).unwrap(), open::SCRIPT);
    let script = format!(
        r#"function Start-Process {{ param($FilePath) "route:url:$FilePath" }}
function Invoke-Item {{ param($LiteralPath) "route:path:$LiteralPath" }}
{}"#,
        open::SCRIPT
    );
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    tokio::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-EncodedCommand",
            &base64::engine::general_purpose::STANDARD.encode(bytes),
        ])
        .env(
            "TOOLBOX_INPUT",
            base64::engine::general_purpose::STANDARD.encode(input.as_bytes()),
        )
        .creation_flags(0x08000000)
        .output()
        .await
        .unwrap()
}

#[cfg(windows)]
#[tokio::test]
async fn shared_powershell_executor_routes_all_three_kinds_without_gui_side_effects() {
    let scratch = Scratch::new();
    let file = scratch.0.join("样本'[1];.txt");
    std::fs::write(&file, "样本").unwrap();
    for (target, kind, expected) in [
        ("https://example.test/?q=a&b=1", "url", "route:url:"),
        (file.to_str().unwrap(), "file", "route:path:"),
        (scratch.0.to_str().unwrap(), "folder", "route:path:"),
    ] {
        let route = open::route(target, Some(kind)).unwrap();
        let output = mocked_open(&route).await;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains(&format!("{expected}{target}")), "{stdout}");
    }
    for (target, kind) in [
        (file.to_str().unwrap(), "folder"),
        (scratch.0.to_str().unwrap(), "file"),
    ] {
        let output = mocked_open(&open::route(target, Some(kind)).unwrap()).await;
        assert!(!output.status.success());
    }
    let missing = scratch.0.join("missing.txt");
    let output = mocked_open(&open::route(missing.to_str().unwrap(), Some("file")).unwrap()).await;
    assert!(!output.status.success());
}

#[test]
fn print_generated_intent_table_for_delivery() {
    let tools = all_tools();
    for (intent, names) in intent_rows(&tools, true) {
        println!(
            "{intent} → {}",
            names.into_iter().collect::<Vec<_>>().join(" / ")
        );
    }
    let index = generate_intent_index(&tools);
    println!(
        "GENERATED_INDEX_LINES={} GENERATED_INDEX_CHARS={} PUBLISHED_TOOL_COUNT={}",
        index.lines().count(),
        index.chars().count(),
        api_tools(&tools).len()
    );
}
