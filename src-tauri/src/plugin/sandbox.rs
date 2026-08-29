// 《铃·记忆体》AI-5 插件沙箱
// 强制隔离：插件不得直接访问宿主机文件系统/网络/系统进程，
// 所有高危操作必须通过 invoke_plugin 白名单命令，逐个校验权限。
// 插件默认无任何权限；未授权操作一律拒绝。
//
// 预留接口（返回"暂未实现"）：network.request / browser.open / clipboard.* / file.write.*
use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use crate::error::AppError;
use crate::plugin::permissions;

/// 无权限要求的白名单命令标记
const NO_PERMISSION: &str = "__none__";

/// 白名单命令 → 所需权限映射
fn whitelist_permission(cmd: &str) -> Result<&'static str, AppError> {
    let perm = match cmd {
        "echo" => NO_PERMISSION, // 调试用，无权限要求
        "file.search" | "file.read_text" => permissions::PERM_FILE_READ,
        "file.write_text" => permissions::PERM_FILE_WRITE,
        "network.request" => permissions::PERM_NETWORK,
        "browser.open" => permissions::PERM_BROWSER,
        "clipboard.read" | "clipboard.write" => permissions::PERM_CLIPBOARD,
        "system.exec" => permissions::PERM_SYSTEM,
        _ => {
            return Err(AppError::PermissionDenied(format!(
                "invoke_plugin 白名单外命令：{cmd}（仅限 file./network/browser/clipboard/system/echo）"
            )))
        }
    };
    Ok(perm)
}

/// 执行白名单命令（JS 插件通过全局 invoke_plugin 调用）
pub fn invoke_whitelisted(
    granted: &[String],
    cmd: &str,
    params: &HashMap<String, Value>,
) -> Result<String, AppError> {
    let perm = whitelist_permission(cmd)?;
    if perm != NO_PERMISSION {
        permissions::check_granted(granted, perm)?;
    }
    match cmd {
        "echo" => Ok(serde_json::to_string(params).unwrap_or_else(|_| "{}".to_string())),
        "file.search" => file_search(params),
        "file.read_text" => file_read_text(params),
        "file.write_text" | "network.request" | "browser.open" | "clipboard.read"
        | "clipboard.write" | "system.exec" => Err(AppError::PluginExecutionError(format!(
            "{cmd} 暂未实现（接口已预留，后续版本开放）"
        ))),
        _ => Err(AppError::PluginExecutionError(format!("未知白名单命令：{cmd}"))),
    }
}

// ---------- 已实现的白名单命令 ----------

/// 递归文件检索：按文件名关键词搜索（深度上限 4、结果上限 50，防止卡死）
fn file_search(params: &HashMap<String, Value>) -> Result<String, AppError> {
    let keyword = get_str(params, "keyword").unwrap_or_default();
    if keyword.is_empty() {
        return Err(AppError::PluginExecutionError("file.search 缺少 keyword 参数".into()));
    }
    let dir_str = get_str(params, "dir").unwrap_or_else(home_dir);
    let dir = Path::new(&dir_str);
    if !dir.exists() || !dir.is_dir() {
        return Err(AppError::PluginExecutionError(format!("目录不存在：{dir_str}")));
    }

    let mut results = Vec::new();
    search_recursive(dir, &keyword.to_lowercase(), &mut results, 0, 4, 50);
    log::info!("file.search：关键词「{keyword}」在 {dir_str} 找到 {} 个文件", results.len());
    Ok(serde_json::json!({ "count": results.len(), "files": results }).to_string())
}

/// 读取文本文件（限制大小 512KB）
fn file_read_text(params: &HashMap<String, Value>) -> Result<String, AppError> {
    let path_str = get_str(params, "path").ok_or_else(|| {
        AppError::PluginExecutionError("file.read_text 缺少 path 参数".into())
    })?;
    let meta = std::fs::metadata(&path_str)
        .map_err(|e| AppError::PluginExecutionError(format!("读取文件失败：{e}")))?;
    if meta.len() > 512 * 1024 {
        return Err(AppError::PluginExecutionError(format!(
            "文件过大（{} KB），超过 512KB 限制",
            meta.len() / 1024
        )));
    }
    let text = std::fs::read_to_string(&path_str)
        .map_err(|e| AppError::PluginExecutionError(format!("读取文件失败：{e}")))?;
    Ok(serde_json::json!({ "path": path_str, "content": text }).to_string())
}

fn search_recursive(
    dir: &Path,
    keyword: &str,
    out: &mut Vec<String>,
    depth: usize,
    max_depth: usize,
    max_results: usize,
) {
    if depth > max_depth || out.len() >= max_results {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return; // 无权限目录直接跳过，不报错
    };
    for entry in entries.flatten() {
        if out.len() >= max_results {
            return;
        }
        let path = entry.path();
        let fname = entry.file_name().to_string_lossy().to_lowercase();
        if fname.contains(keyword) {
            out.push(path.display().to_string());
        }
        if path.is_dir() {
            search_recursive(&path, keyword, out, depth + 1, max_depth, max_results);
        }
    }
}

// ---------- 工具 ----------

fn get_str<'a>(params: &'a HashMap<String, Value>, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 获取用户主目录（Windows USERPROFILE）
fn home_dir() -> String {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| "C:/".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn granted(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn params(pairs: &[(&str, &str)]) -> HashMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), Value::String(v.to_string())))
            .collect()
    }

    #[test]
    fn echo无需权限() {
        let g = granted(&[]);
        let p = params(&[("text", "hi")]);
        let r = invoke_whitelisted(&g, "echo", &p).unwrap();
        assert!(r.contains("hi"));
    }

    #[test]
    fn 未授权file权限拒绝文件检索() {
        let g = granted(&[]);
        let p = params(&[("keyword", "nothing")]);
        let err = invoke_whitelisted(&g, "file.search", &p).unwrap_err();
        assert!(err.to_string().contains("权限"));
    }

    #[test]
    fn 授权后文件检索可执行() {
        let g = granted(&["file.read"]);
        let p = params(&[("keyword", "windows")]);
        // 搜索 C:/Windows 存在，keyword 用 "Win" 不区分大小写
        let r = invoke_whitelisted(&g, "file.search", &params(&[("keyword", "win"), ("dir", "C:/Windows")])).unwrap();
        assert!(r.contains("count"));
        let _ = p;
    }

    #[test]
    fn 白名单外命令拒绝() {
        let g = granted(&["file.read", "system", "network"]);
        let err = invoke_whitelisted(&g, "rm -rf /", &params(&[])).unwrap_err();
        assert!(err.to_string().contains("白名单外"));
    }

    #[test]
    fn system权限即使有网络等权限也不能执行白名单system命令() {
        // network 权限 ≠ system 权限
        let g = granted(&["network"]);
        let err = invoke_whitelisted(&g, "system.exec", &params(&[("command", "whoami")])).unwrap_err();
        assert!(err.to_string().contains("权限"));
    }
}
