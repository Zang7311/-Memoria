// 《铃·记忆体》AI-5 Hermes Agent 兼容层
// 目标：解析 Hermes 插件格式（.hermes/plugins/xxx/ 下的 config.json + main.js），
// 转换为《铃》标准 Plugin 结构，使现有 Hermes 插件可以加载并注册技能。
//
// 兼容范围说明：
// - 完整支持：config.json 解析（name/version/author/description/main/skills）
// - 完整支持：skills 定义（含 parameters）转换为标准 Skill
// - 部分支持：执行 —— 纯 JS 技能（globalThis.skills 约定）可直接运行；
//   依赖 Node API（require/process/网络）的 Hermes 插件会给出明确错误提示
use std::path::Path;

use crate::error::AppError;
use crate::types::{Plugin, PluginManifest, Skill, SkillParam};

/// 从目录加载 Hermes 风格插件（目录内含 config.json）
pub fn load_hermes_plugin(dir: &Path, id: &str) -> Result<Plugin, AppError> {
    let config_path = dir.join("config.json");
    if !config_path.exists() {
        return Err(AppError::PluginInstallError(format!(
            "Hermes 插件缺少 config.json：{}",
            dir.display()
        )));
    }
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| AppError::PluginInstallError(format!("读取 config.json 失败：{e}")))?;
    let raw: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::PluginInstallError(format!("config.json 不是合法 JSON：{e}")))?;

    let get_str = |key: &str, default: &str| -> String {
        raw.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or(default)
            .to_string()
    };

    let name = get_str("name", id);
    let version = get_str("version", "0.1.0");
    let author = get_str("author", "Hermes 社区");
    let description = get_str("description", "");
    let main = get_str("main", "index.js");

    // 转换 Hermes skills → 标准 Skill
    let mut skills = Vec::new();
    if let Some(arr) = raw.get("skills").and_then(|v| v.as_array()) {
        for s in arr {
            let s_name = s.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            if s_name.is_empty() {
                continue; // 跳过无名技能
            }
            let mut parameters = Vec::new();
            if let Some(ps) = s.get("parameters").and_then(|v| v.as_array()) {
                for p in ps {
                    parameters.push(SkillParam {
                        name: p.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                        type_: p
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("string")
                            .to_string(),
                        required: p.get("required").and_then(|v| v.as_bool()).unwrap_or(false),
                        description: p
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    });
                }
            }
            skills.push(Skill {
                name: s_name.to_string(),
                description: s
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                parameters,
                action: format!("js:{s_name}"), // Hermes 技能走 JS 引擎执行
            });
        }
    }

    Ok(Plugin {
        id: format!("hermes_{id}"),
        name,
        version,
        author,
        description,
        enabled: false, // Hermes 插件默认禁用，需用户启用
        path: dir.display().to_string(),
        manifest: PluginManifest {
            main,
            skills,
            permissions: Vec::new(), // Hermes 插件权限按需授予（默认无权限）
            hermes_compatible: true,
        },
        granted: Vec::new(), // 授权权限由注册表管理
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个临时 Hermes 插件目录
    fn make_hermes_plugin(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        let config = r#"{
            "name": "hello_hermes",
            "version": "1.0.0",
            "author": "Hermes 社区",
            "description": "测试插件",
            "main": "main.js",
            "skills": [
                {
                    "name": "hello",
                    "description": "问好",
                    "parameters": [
                        { "name": "who", "type": "string", "required": false, "description": "向谁问好" }
                    ]
                }
            ]
        }"#;
        std::fs::write(dir.join("config.json"), config).unwrap();
        std::fs::write(dir.join("main.js"), "globalThis.skills = { hello: function(p){ return '你好，' + (p.who || '世界'); } };").unwrap();
    }

    #[test]
    fn 解析hermes插件并转换结构() {
        let dir = std::env::temp_dir().join("hermes_compat_test");
        let _ = std::fs::remove_dir_all(&dir);
        make_hermes_plugin(&dir);

        let plugin = load_hermes_plugin(&dir, "hello_hermes").unwrap();
        assert_eq!(plugin.id, "hermes_hello_hermes");
        assert!(plugin.manifest.hermes_compatible);
        assert_eq!(plugin.manifest.main, "main.js");
        assert_eq!(plugin.manifest.skills.len(), 1);
        assert_eq!(plugin.manifest.skills[0].name, "hello");
        assert_eq!(plugin.manifest.skills[0].action, "js:hello");
        assert_eq!(plugin.manifest.skills[0].parameters[0].type_, "string");
        assert!(plugin.manifest.permissions.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 缺少configjson报错() {
        let dir = std::env::temp_dir().join("hermes_compat_none");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(load_hermes_plugin(&dir, "x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
