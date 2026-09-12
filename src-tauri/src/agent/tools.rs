// 《铃·记忆体》Agent 能力 —— 工具描述生成器（tools.rs）
//
// 职责：把现有的「工具箱条目」和「插件技能」统一包装成 OpenAI Function Calling 的
// `tools` 数组（name / description / parameters JSON Schema）。
//
// 设计要点：
//   1. 不重复发明执行逻辑 —— 执行仍走 toolbox_execute / plugin runner / quick_command，
//      本模块只负责「告诉 LLM 有哪些工具、每个工具能干什么、要什么参数」。
//   2. 工具箱命令是 base64 编码的 PowerShell，无法从 command 反推语义，因此工具描述
//      依赖 name / input_label / input_placeholder 三个人类可读字段。
//   3. 所有工具箱工具统一暴露一个 `input` 字符串参数（对应现有 TOOLBOX_INPUT 传参机制），
//      简化 LLM 侧的参数理解；参数语义由 description 说明。
//   4. 危险工具（format-disk / shred 等）默认不暴露给 LLM，避免 Agent 误触发不可逆操作。
//      如需暴露，走白名单配置。
use serde_json::{json, Value};

use crate::types::{Plugin, ToolboxItem};

/// Agent 权限开关（从应用设置读取）
///
/// 危险类 Agent 工具（下载 / 安装卸载 / 写删文件）默认**不暴露**给 LLM，
/// 需用户在设置页显式开启对应开关后，相关工具才会出现在工具列表里。
#[derive(Debug, Clone, Copy, Default)]
pub struct AgentPermissions {
    /// 允许下载文件
    pub allow_download: bool,
    /// 允许安装 / 卸载软件
    pub allow_software: bool,
    /// 允许写入 / 删除文件
    pub allow_file_write: bool,
    /// 允许执行任意命令（shell，最高危）
    pub allow_shell: bool,
}

impl AgentPermissions {
    /// 判断某个权限分类是否已授权
    ///
    /// - `None`：该工具无需特殊授权（只读查询类），永远可用
    /// - 未识别的分类：保守拒绝（避免将来新增分类被意外放行）
    pub fn allows(&self, perm: Option<&str>) -> bool {
        match perm {
            None => true,
            Some("download") => self.allow_download,
            Some("software") => self.allow_software,
            Some("file_write") => self.allow_file_write,
            Some("shell") => self.allow_shell,
            Some(_) => false,
        }
    }
}

/// 默认不暴露给 Agent 的危险工具 id 黑名单（不可逆 / 高危操作）
/// 这些工具只在用户手动点击工具箱时才可用，Agent 不得自主调用。
const DANGEROUS_TOOL_IDS: &[&str] = &[
    "format-disk",   // 格式化硬盘
    "shred",         // 文件粉碎
    "shutdown-1h",   // 定时关机
    "cancel-shutdown", // 取消关机（相对安全，但配合 shutdown 场景，暂保守）
    "lock",          // 锁屏（会打断用户当前操作）
];

/// 工具 id 是否为危险工具（Agent 不暴露）
fn is_dangerous(id: &str) -> bool {
    DANGEROUS_TOOL_IDS.iter().any(|d| *d == id)
}

/// 把一个工具箱条目转成 OpenAI tool 定义。
///
/// 返回 None 表示该工具不应暴露给 Agent（危险工具 / 空命令纯前端工具）。
fn toolbox_to_tool(item: &ToolboxItem, perms: &AgentPermissions) -> Option<Value> {
    // 跳过危险工具
    if is_dangerous(&item.id) {
        return None;
    }
    // 权限过滤：需授权但用户未开启的工具，不暴露给 LLM
    if !perms.allows(item.agent_permission.as_deref()) {
        return None;
    }
    // 跳过无命令的纯前端工具（pixel-art / regex / ocr / qrcode-* 等 command 为空，
    // 它们依赖前端交互，Agent 无法通过命令行直接驱动）
    if item.command.trim().is_empty() {
        return None;
    }
    // 跳过禁用的工具
    if !item.enabled {
        return None;
    }

    // 描述：name 是主描述；有 input 提示的补充参数说明
    let mut desc = item.name.clone();
    if item.needs_input {
        if let Some(label) = &item.input_label {
            desc.push_str(&format!("。参数：{}", label));
        }
        if let Some(ph) = &item.input_placeholder {
            desc.push_str(&format!("（示例：{}）", ph));
        }
    } else {
        desc.push_str("（无需参数）");
    }

    // 参数 schema：统一一个 input 字符串参数
    let properties = if item.needs_input {
        json!({
            "input": {
                "type": "string",
                "description": item
                    .input_placeholder
                    .clone()
                    .unwrap_or_else(|| item.input_label.clone().unwrap_or_default()),
            }
        })
    } else {
        json!({})
    };
    let required: Vec<&str> = if item.needs_input { vec!["input"] } else { vec![] };

    Some(json!({
        "type": "function",
        "function": {
            "name": format!("toolbox_{}", item.id),
            "description": desc,
            "parameters": {
                "type": "object",
                "properties": properties,
                "required": required,
            }
        }
    }))
}

/// 把一个插件技能转成 OpenAI tool 定义。
///
/// 插件技能已有 name / description / parameters 结构，直接映射。
fn skill_to_tool(plugin: &Plugin, skill: &crate::types::Skill) -> Value {
    // 参数 schema：把 SkillParam 列表转成 JSON Schema properties
    let mut properties = json!({});
    let mut required: Vec<String> = Vec::new();
    for p in &skill.parameters {
        let t = match p.type_.as_str() {
            "number" => "number",
            "boolean" => "boolean",
            "file" => "string", // file 在 LLM 侧当字符串路径传
            _ => "string",
        };
        properties[p.name.clone()] = json!({
            "type": t,
            "description": p.description,
        });
        if p.required {
            required.push(p.name.clone());
        }
    }

    json!({
        "type": "function",
        "function": {
            // 用 plugin_id::skill_name 做命名空间，避免不同插件技能同名冲突
            "name": format!("skill_{}__{}", plugin.id, skill.name),
            "description": skill.description,
            "parameters": {
                "type": "object",
                "properties": properties,
                "required": required,
            }
        }
    })
}

/// 生成全部可用的 Agent 工具列表（工具箱 + 已启用插件技能）。
///
/// 这是 Agent 循环开始时调用一次、注入到 system/工具声明里的唯一入口。
///
/// # 参数
/// - `toolbox_items`: 工具箱条目列表（`toolbox::list_items` 的结果）
/// - `plugins`: 已注册插件列表（`plugin::manager` 的结果）
///
/// # 返回
/// OpenAI Function Calling 的 `tools` JSON 数组。
pub fn build_tools(
    toolbox_items: &[ToolboxItem],
    plugins: &[Plugin],
    perms: &AgentPermissions,
) -> Vec<Value> {
    let mut tools: Vec<Value> = Vec::new();

    // 1. 工具箱（含 Agent 专用工具；按权限过滤）
    for item in toolbox_items {
        if let Some(t) = toolbox_to_tool(item, perms) {
            tools.push(t);
        }
    }

    // 2. 插件技能（仅已启用的插件）
    for plugin in plugins.iter().filter(|p| p.enabled) {
        for skill in &plugin.manifest.skills {
            tools.push(skill_to_tool(plugin, skill));
        }
    }

    // 3. 内置原生工具（Rust 侧直接执行，不走 PowerShell，所以不在 agent_tools.json 里）
    tools.push(look_tool());

    tools
}

/// 内置原生工具：看图（toolbox_agent_look）
///
/// 由 Rust 侧 `agent::vision` 直接调用多模态模型，把本地图片编码成 base64 送过去理解。
/// 与 `agent_screenshot` 配合即可形成「截屏 → 看懂屏幕上发生了什么」的闭环。
fn look_tool() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "toolbox_agent_look",
            "description": "看图片内容：把本地图片交给视觉模型理解，能描述画面、读出图中的文字。想知道屏幕上现在是什么样，先用 agent_screenshot 截图，再把截图路径交给这个工具。",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "图片的完整路径，例如 C:\\Users\\me\\Desktop\\shot.png"
                    },
                    "question": {
                        "type": "string",
                        "description": "想问的问题，例如「这个报错是什么意思」。省略时做通用描述并读出文字。"
                    }
                },
                "required": ["path"]
            }
        }
    })
}

/// 工具名（function name）反查：判断它来自工具箱还是插件，返回分类与 id。
///
/// 供 router 使用：根据 LLM 返回的 tool_call name 路由到对应执行器。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolKind {
    /// 工具箱工具，携带原始 item id（如 "clean-temp"）
    Toolbox(String),
    /// 插件技能，携带 (plugin_id, skill_name)
    Skill(String, String),
    /// 未识别的工具名
    Unknown,
}

/// 解析 tool_call 的 name，判断它属于哪类工具。
pub fn classify_tool(name: &str) -> ToolKind {
    if let Some(rest) = name.strip_prefix("toolbox_") {
        ToolKind::Toolbox(rest.to_string())
    } else if let Some(rest) = name.strip_prefix("skill_") {
        // 格式：skill_{plugin_id}__{skill_name}
        if let Some((pid, sname)) = rest.split_once("__") {
            ToolKind::Skill(pid.to_string(), sname.to_string())
        } else {
            ToolKind::Unknown
        }
    } else {
        ToolKind::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Skill, SkillParam, ToolboxItem};

    fn sample_item() -> ToolboxItem {
        ToolboxItem {
            id: "clean-temp".to_string(),
            name: "清理临时文件".to_string(),
            icon: "🧹".to_string(),
            command: "powershell -NoProfile -Command \"...\"".to_string(),
            enabled: true,
            needs_input: false,
            input_label: None,
            input_placeholder: None,
            steps: vec![],
            agent_permission: None,
        }
    }

    fn sample_item_with_input() -> ToolboxItem {
        ToolboxItem {
            id: "ping".to_string(),
            name: "网络 Ping 测试".to_string(),
            icon: "📡".to_string(),
            command: "powershell ...".to_string(),
            enabled: true,
            needs_input: true,
            input_label: Some("输入 IP 或域名".to_string()),
            input_placeholder: Some("例：8.8.8.8 或 www.bing.com".to_string()),
            steps: vec![],
            agent_permission: None,
        }
    }

    #[test]
    fn 工具描述_无参工具() {
        let t = toolbox_to_tool(&sample_item(), &AgentPermissions::default()).unwrap();
        assert_eq!(t["function"]["name"], "toolbox_clean-temp");
        assert!(t["function"]["description"].as_str().unwrap().contains("清理临时文件"));
        assert_eq!(t["function"]["parameters"]["required"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn 工具描述_带参工具() {
        let t = toolbox_to_tool(&sample_item_with_input(), &AgentPermissions::default()).unwrap();
        assert_eq!(t["function"]["name"], "toolbox_ping");
        let req = t["function"]["parameters"]["required"].as_array().unwrap();
        assert_eq!(req.len(), 1);
        assert_eq!(req[0], "input");
        assert!(t["function"]["parameters"]["properties"]["input"]["description"]
            .as_str()
            .unwrap()
            .contains("8.8.8.8"));
    }

    #[test]
    fn 工具描述_危险工具被过滤() {
        let mut item = sample_item();
        item.id = "format-disk".to_string();
        assert!(toolbox_to_tool(&item, &AgentPermissions::default()).is_none());
    }

    #[test]
    fn 工具描述_空命令前端工具被过滤() {
        let mut item = sample_item();
        item.command = "".to_string();
        item.id = "ocr".to_string();
        assert!(toolbox_to_tool(&item, &AgentPermissions::default()).is_none());
    }

    #[test]
    fn 工具分类_解析三类名称() {
        assert_eq!(classify_tool("toolbox_clean-temp"), ToolKind::Toolbox("clean-temp".into()));
        assert_eq!(
            classify_tool("skill_file_search__by_keyword"),
            ToolKind::Skill("file_search".into(), "by_keyword".into())
        );
        assert_eq!(classify_tool("foo_bar"), ToolKind::Unknown);
    }

    #[test]
    fn 插件技能_转工具描述() {
        let plugin = Plugin {
            id: "file_search".to_string(),
            name: "文件搜索".to_string(),
            version: "1.0".to_string(),
            author: "test".to_string(),
            description: "搜索文件".to_string(),
            enabled: true,
            path: ".".to_string(),
            manifest: crate::types::PluginManifest {
                main: "index.js".to_string(),
                skills: vec![],
                permissions: vec![],
                hermes_compatible: false,
            },
            granted: vec![],
        };
        let skill = Skill {
            name: "by_keyword".to_string(),
            description: "按关键词搜索文件".to_string(),
            parameters: vec![SkillParam {
                name: "keyword".to_string(),
                type_: "string".to_string(),
                required: true,
                description: "搜索关键词".to_string(),
            }],
            action: "js:by_keyword".to_string(),
        };
        let t = skill_to_tool(&plugin, &skill);
        assert_eq!(t["function"]["name"], "skill_file_search__by_keyword");
        assert_eq!(
            t["function"]["parameters"]["properties"]["keyword"]["type"],
            "string"
        );
    }

    // ==================== 权限过滤（Agent 安全边界） ====================

    /// 未授权的危险工具必须不出现在 Agent 工具列表中
    #[test]
    fn 权限过滤_未授权不暴露_授权后暴露() {
        let mut item = sample_item();
        item.id = "agent_write_file".to_string();
        item.agent_permission = Some("file_write".to_string());

        // 默认（三个开关全关）→ 不暴露
        assert!(
            toolbox_to_tool(&item, &AgentPermissions::default()).is_none(),
            "未开启 file_write 时不应暴露写文件工具"
        );

        // 开启 file_write → 暴露
        let perms = AgentPermissions {
            allow_file_write: true,
            ..Default::default()
        };
        assert!(
            toolbox_to_tool(&item, &perms).is_some(),
            "开启 file_write 后应暴露写文件工具"
        );
    }

    /// 只读查询类工具（agent_permission = None）永远可用，不受开关影响
    #[test]
    fn 权限过滤_自由工具始终可用() {
        let item = sample_item(); // agent_permission: None
        assert!(toolbox_to_tool(&item, &AgentPermissions::default()).is_some());
        let all_on = AgentPermissions {
            allow_download: true,
            allow_software: true,
            allow_file_write: true,
            allow_shell: true,
        };
        assert!(toolbox_to_tool(&item, &all_on).is_some());
    }

    /// 未识别的权限分类保守拒绝（避免将来新增分类被意外放行）
    #[test]
    fn 权限过滤_未知分类保守拒绝() {
        let mut item = sample_item();
        item.agent_permission = Some("unknown_kind".to_string());
        let all_on = AgentPermissions {
            allow_download: true,
            allow_software: true,
            allow_file_write: true,
            allow_shell: true,
        };
        assert!(
            toolbox_to_tool(&item, &all_on).is_none(),
            "未知权限分类应保守拒绝"
        );
    }

    /// allows() 的分发表覆盖
    #[test]
    fn 权限判断_分类分发正确() {
        let perms = AgentPermissions {
            allow_download: true,
            allow_software: false,
            allow_file_write: false,
            allow_shell: false,
        };
        assert!(perms.allows(None));
        assert!(perms.allows(Some("download")));
        assert!(!perms.allows(Some("software")));
        assert!(!perms.allows(Some("file_write")));
        assert!(!perms.allows(Some("bogus")));
    }

    /// 端到端：加载真实 agent_tools.json，默认配置下危险工具必须一个都不出现
    #[test]
    fn 端到端_默认配置只暴露安全工具() {
        let items = crate::desktop::toolbox::list_agent_items();
        let tools = build_tools(&items, &[], &AgentPermissions::default());
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t["function"]["name"].as_str())
            .collect();

        // 危险工具（需授权）不应出现
        for blocked in [
            "toolbox_agent_winget_install",
            "toolbox_agent_winget_uninstall",
            "toolbox_agent_download",
            "toolbox_agent_write_file",
            "toolbox_agent_mkdir",
            "toolbox_agent_delete_file",
            "toolbox_agent_run_command",
            "toolbox_agent_window_control",
            "toolbox_agent_input_mouse",
            "toolbox_agent_input_keyboard",
            "toolbox_agent_power",
            "toolbox_agent_network",
            "toolbox_agent_script",
            "toolbox_agent_registry",
            "toolbox_agent_service",
            "toolbox_agent_schtask",
        ] {
            assert!(
                !names.contains(&blocked),
                "默认配置下不应暴露危险工具：{blocked}"
            );
        }

        // 安全工具应正常出现
        for expected in [
            "toolbox_agent_disk_space",
            "toolbox_agent_web_search",
            "toolbox_agent_web_fetch",
            "toolbox_agent_installed_apps",
            // Agent 基本功（只读，无需授权）
            "toolbox_agent_list_dir",
            "toolbox_agent_read_file",
            "toolbox_agent_search_files",
            "toolbox_agent_search_content",
            "toolbox_agent_clipboard",
            "toolbox_agent_open_path",
            // 视觉 / 记忆 / 窗口 / 提醒（只读或本地无副作用）
            "toolbox_agent_screenshot",
            "toolbox_agent_remember",
            "toolbox_agent_window_list",
            "toolbox_agent_remind",
            // OCR / 网络 / 硬件 / 图片 / 语音 / 快捷指令
            "toolbox_agent_ocr",
            "toolbox_agent_netdiag",
            "toolbox_agent_hardware",
            "toolbox_agent_image",
            "toolbox_agent_speak",
            "toolbox_agent_shortcut",
        ] {
            assert!(
                names.contains(&expected),
                "默认配置下应暴露安全工具：{expected}"
            );
        }
    }

    /// 端到端：全部授权后，危险工具应全部出现
    #[test]
    fn 端到端_全部授权后危险工具出现() {
        let items = crate::desktop::toolbox::list_agent_items();
        let all_on = AgentPermissions {
            allow_download: true,
            allow_software: true,
            allow_file_write: true,
            allow_shell: true,
        };
        let tools = build_tools(&items, &[], &all_on);
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t["function"]["name"].as_str())
            .collect();

        for expected in [
            "toolbox_agent_winget_install",
            "toolbox_agent_winget_uninstall",
            "toolbox_agent_download",
            "toolbox_agent_write_file",
            "toolbox_agent_mkdir",
            "toolbox_agent_delete_file",
            "toolbox_agent_run_command",
            "toolbox_agent_window_control",
            "toolbox_agent_input_mouse",
            "toolbox_agent_input_keyboard",
            "toolbox_agent_power",
            "toolbox_agent_network",
            "toolbox_agent_script",
            "toolbox_agent_registry",
            "toolbox_agent_service",
            "toolbox_agent_schtask",
        ] {
            assert!(
                names.contains(&expected),
                "全部授权后应暴露危险工具：{expected}"
            );
        }
    }
}
