// 《铃·记忆体》AI-5 插件加载器
// 读取插件目录下的 manifest.json（或 Hermes config.json），解析为 PluginManifest，
// 验证插件结构（入口文件存在、技能定义完整），加载失败仅记日志不阻塞主程序。
use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::types::{Plugin, PluginManifest};
use crate::plugin::hermes_compat;

/// 从目录加载插件（优先标准 manifest.json，其次 Hermes config.json）
pub fn load_plugin_from_dir(dir: &Path, id: &str) -> Result<Plugin, AppError> {
    let manifest_path = dir.join("manifest.json");
    if manifest_path.exists() {
        load_standard_plugin(dir, id, &manifest_path)
    } else if dir.join("config.json").exists() {
        hermes_compat::load_hermes_plugin(dir, id)
    } else {
        Err(AppError::PluginInstallError(format!(
            "插件目录缺少 manifest.json 或 config.json：{}",
            dir.display()
        )))
    }
}

/// 解析标准 manifest.json 插件
fn load_standard_plugin(dir: &Path, id: &str, manifest_path: &Path) -> Result<Plugin, AppError> {
    let content = std::fs::read_to_string(manifest_path)
        .map_err(|e| AppError::PluginInstallError(format!("读取 manifest.json 失败：{e}")))?;
    let raw: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::PluginInstallError(format!("manifest.json 不是合法 JSON：{e}")))?;

    let manifest: PluginManifest = serde_json::from_value(raw.clone())
        .map_err(|e| AppError::PluginInstallError(format!("manifest.json 字段缺失或类型错误：{e}")))?;

    // 元信息字段（任务书 PluginManifest 不含这些，从原始 JSON 读取）
    let get_str = |key: &str, default: &str| -> String {
        raw.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or(default)
            .to_string()
    };

    let plugin = Plugin {
        id: id.to_string(),
        name: get_str("name", id),
        version: get_str("version", "0.1.0"),
        author: get_str("author", "未知"),
        description: get_str("description", ""),
        enabled: false, // 启用状态由注册表决定
        path: dir.display().to_string(),
        manifest,
        granted: Vec::new(), // 授权权限由注册表管理
    };

    validate_plugin(&plugin)?;
    Ok(plugin)
}

/// 验证插件结构完整性
pub fn validate_plugin(plugin: &Plugin) -> Result<(), AppError> {
    let dir = PathBuf::from(&plugin.path);

    // 1. 技能不能为空（纯命令/内置动作插件除外——它们也有技能，只是无 main）
    if plugin.manifest.skills.is_empty() {
        return Err(AppError::PluginInstallError(format!(
            "插件「{}」没有注册任何技能",
            plugin.name
        )));
    }

    // 2. 入口文件检查：main 非空 → 文件必须存在；main 为空 → 所有动作必须是 command:/builtin: 前缀
    let main = plugin.manifest.main.trim();
    if !main.is_empty() {
        if !dir.join(main).exists() {
            return Err(AppError::PluginInstallError(format!(
                "插件「{}」的入口文件不存在：{}",
                plugin.name,
                dir.join(main).display()
            )));
        }
    } else {
        for skill in &plugin.manifest.skills {
            let action = skill.action.trim();
            if !(action.starts_with("command:") || action.starts_with("builtin:")) {
                return Err(AppError::PluginInstallError(format!(
                    "插件「{}」没有入口文件（main 为空），技能「{}」的动作必须是 command: 或 builtin: 前缀",
                    plugin.name, skill.name
                )));
            }
        }
    }

    // 3. 技能定义完整性：名称与动作非空
    for skill in &plugin.manifest.skills {
        if skill.name.trim().is_empty() {
            return Err(AppError::PluginInstallError(format!(
                "插件「{}」存在无名技能",
                plugin.name
            )));
        }
        if skill.action.trim().is_empty() {
            return Err(AppError::PluginInstallError(format!(
                "插件「{}」的技能「{}」缺少 action",
                plugin.name, skill.name
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Skill;

    fn make_manifest(dir: &Path, manifest_json: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("manifest.json"), manifest_json).unwrap();
    }

    #[test]
    fn 解析合法manifest() {
        let dir = std::env::temp_dir().join("loader_ok");
        let _ = std::fs::remove_dir_all(&dir);
        make_manifest(&dir, r#"{
            "name": "测试插件",
            "version": "1.0.0",
            "author": "测试",
            "description": "用于测试",
            "main": "index.js",
            "skills": [{ "name": "test", "description": "测试技能", "parameters": [], "action": "js:test" }],
            "permissions": ["file.read"],
            "hermes_compatible": false
        }"#);
        std::fs::write(dir.join("index.js"), "globalThis.skills = {};").unwrap();

        let plugin = load_plugin_from_dir(&dir, "test_plugin").unwrap();
        assert_eq!(plugin.id, "test_plugin");
        assert_eq!(plugin.name, "测试插件");
        assert_eq!(plugin.manifest.skills.len(), 1);
        assert_eq!(plugin.manifest.permissions, vec!["file.read".to_string()]);
        assert!(!plugin.manifest.hermes_compatible);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 入口文件缺失报错() {
        let dir = std::env::temp_dir().join("loader_missing_main");
        let _ = std::fs::remove_dir_all(&dir);
        make_manifest(&dir, r#"{
            "main": "index.js",
            "skills": [{ "name": "test", "description": "x", "parameters": [], "action": "js:test" }],
            "permissions": [],
            "hermes_compatible": false
        }"#);
        // 不创建 index.js
        let err = load_plugin_from_dir(&dir, "x").unwrap_err();
        assert!(err.to_string().contains("入口文件不存在"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 非法JSON报错() {
        let dir = std::env::temp_dir().join("loader_bad_json");
        let _ = std::fs::remove_dir_all(&dir);
        make_manifest(&dir, "{ 这不是 JSON !!!");
        assert!(load_plugin_from_dir(&dir, "x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 无main终端插件合法() {
        let dir = std::env::temp_dir().join("loader_terminal");
        let _ = std::fs::remove_dir_all(&dir);
        make_manifest(&dir, r#"{
            "name": "清临时文件",
            "main": "",
            "skills": [{ "name": "clean_temp", "description": "清理临时文件", "parameters": [], "action": "command:del /q %TEMP%\\*" }],
            "permissions": ["system"],
            "hermes_compatible": false
        }"#);
        let plugin = load_plugin_from_dir(&dir, "clean_temp").unwrap();
        assert_eq!(plugin.manifest.skills[0].action, "command:del /q %TEMP%\\*");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 技能缺少动作报错() {
        let dir = std::env::temp_dir().join("loader_no_action");
        let _ = std::fs::remove_dir_all(&dir);
        make_manifest(&dir, r#"{
            "main": "",
            "skills": [{ "name": "test", "description": "x", "parameters": [], "action": "" }],
            "permissions": [],
            "hermes_compatible": false
        }"#);
        assert!(load_plugin_from_dir(&dir, "x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_js技能需要main() {
        // 直接构造：main 为空但 action 是 js: 前缀 → 应报错
        let plugin = Plugin {
            id: "x".into(),
            name: "x".into(),
            version: "0.1.0".into(),
            author: "x".into(),
            description: "".into(),
            enabled: false,
            path: std::env::temp_dir().display().to_string(),
            manifest: PluginManifest {
                main: String::new(),
                skills: vec![Skill {
                    name: "s".into(),
                    description: "".into(),
                    parameters: vec![],
                    action: "js:s".into(),
                }],
                permissions: vec![],
                hermes_compatible: false,
            },
            granted: Vec::new(),
        };
        assert!(validate_plugin(&plugin).is_err());
    }
}
