// 《铃·记忆体》AI-5 插件权限系统
// 安全原则：插件默认无任何权限；所有权限需用户显式授予；
// 高风险权限（system）默认禁用，用户需手动开启。
use crate::error::AppError;

/// 权限常量（manifest.permissions 与注册表 granted 中使用的字符串）
pub const PERM_FILE_READ: &str = "file.read";
pub const PERM_FILE_WRITE: &str = "file.write";
pub const PERM_NETWORK: &str = "network";
pub const PERM_BROWSER: &str = "browser";
pub const PERM_CLIPBOARD: &str = "clipboard";
pub const PERM_SYSTEM: &str = "system";
pub const PERM_ADVANCED: &str = "advanced";

/// 全部权限及其中文说明（前端权限管理界面展示用）
pub const ALL_PERMISSIONS: [(&str, &str); 7] = [
    (PERM_FILE_READ, "读取文件"),
    (PERM_FILE_WRITE, "写入文件"),
    (PERM_NETWORK, "网络请求"),
    (PERM_BROWSER, "浏览器控制"),
    (PERM_CLIPBOARD, "剪贴板读写"),
    (PERM_SYSTEM, "系统命令（高风险，默认禁用）"),
    (PERM_ADVANCED, "高级 JS 能力（process/require，预留）"),
];

/// 权限中文名
pub fn permission_label(permission: &str) -> String {
    ALL_PERMISSIONS
        .iter()
        .find(|(p, _)| *p == permission)
        .map(|(_, label)| label.to_string())
        .unwrap_or_else(|| permission.to_string())
}

/// 根据技能动作前缀推断所需权限：
/// - `command:<命令>` → system（终端命令扩展）
/// - `builtin:file.*` → file.read；`builtin:clipboard.*` → clipboard；`builtin:browser.*` → browser
/// - `js:*` → 无固定权限，由 JS 内 invoke_plugin 白名单逐个校验
pub fn required_permission(action: &str) -> Option<&'static str> {
    if action.starts_with("command:") {
        return Some(PERM_SYSTEM);
    }
    if let Some(b) = action.strip_prefix("builtin:") {
        if b.starts_with("file.") {
            return Some(PERM_FILE_READ);
        }
        if b.starts_with("clipboard") {
            return Some(PERM_CLIPBOARD);
        }
        if b.starts_with("browser") {
            return Some(PERM_BROWSER);
        }
        if b.starts_with("network") {
            return Some(PERM_NETWORK);
        }
    }
    None
}

/// 校验插件是否已获得指定权限（granted = 用户实际授权的权限列表）
pub fn check_granted(granted: &[String], permission: &str) -> Result<(), AppError> {
    if granted.iter().any(|p| p == permission) {
        Ok(())
    } else {
        Err(AppError::PermissionDenied(format!(
            "插件未获得「{}」权限，请在插件管理中授权后再试",
            permission_label(permission)
        )))
    }
}

/// 校验高风险 system 权限（默认禁用，必须显式授予）
pub fn check_system_granted(granted: &[String]) -> Result<(), AppError> {
    check_granted(granted, PERM_SYSTEM)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn granted(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn 权限推断_command动作需要system() {
        assert_eq!(required_permission("command:del /q %TEMP%\\*"), Some(PERM_SYSTEM));
    }

    #[test]
    fn 权限推断_builtin动作映射() {
        assert_eq!(required_permission("builtin:file.search"), Some(PERM_FILE_READ));
        assert_eq!(required_permission("builtin:clipboard.read"), Some(PERM_CLIPBOARD));
        assert_eq!(required_permission("builtin:browser.open"), Some(PERM_BROWSER));
    }

    #[test]
    fn 权限推断_js动作无固定权限() {
        assert_eq!(required_permission("js:file_search"), None);
    }

    #[test]
    fn 未授权时拒绝() {
        let g = granted(&["file.read"]);
        assert!(check_granted(&g, PERM_NETWORK).is_err());
    }

    #[test]
    fn 授权后通过() {
        let g = granted(&["file.read", "network"]);
        assert!(check_granted(&g, PERM_NETWORK).is_ok());
    }

    #[test]
    fn system权限默认拒绝() {
        let g = granted(&["file.read"]); // 未显式授予 system
        assert!(check_system_granted(&g).is_err());
    }

    #[test]
    fn 空权限列表全部拒绝() {
        let g: Vec<String> = vec![];
        assert!(check_granted(&g, PERM_FILE_READ).is_err());
    }
}
