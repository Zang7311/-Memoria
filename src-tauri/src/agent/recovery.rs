// 《铃·记忆体》AI-10 错误恢复模块（recovery.rs）
//
// 职责：当 Agent 工具执行失败后，对错误文本做「结构化诊断 → 换方案」。
//   1. classify_error   将原始错误文本分类为 ErrorKind（关键词匹配）
//   2. suggest_fallback 根据 ErrorKind + 工具名 + 参数，产出一条给 LLM 的中文建议
//
// 设计约束：
//   - 纯函数模块：不涉及 async、不依赖 AppHandle / Tauri 状态，只依赖标准库。
//   - 输入输出皆可序列化无关的普通类型，方便在 loop_.rs 里接在失败分支之后调用。
//   - 分类规则刻意保守：关键词命中才分类，全部未命中才落到 Unknown，
//     避免把「工具本身输出的正常文本」误判成错误类型。

use std::collections::HashMap;

/// 结构化的错误分类
///
/// 六个变体覆盖 Agent 工具执行中最常见的失败原因。
/// 每个变体携带一条面向 LLM 的、可读的中文标签，见 `label` 方法。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// 权限不足（Access Denied / 拒绝访问 / 需要管理员权限）
    PermissionDenied,
    /// 路径/文件不存在（not found / 找不到 / 不存在）
    PathNotFound,
    /// 命令不存在（command not found / 不是内部或外部命令 / 未找到命令）
    CommandNotFound,
    /// 超时（timed out / 超时）
    Timeout,
    /// 参数错误（invalid argument / 参数错误 / 缺少参数 / 类型不匹配）
    InvalidArgument,
    /// 未知错误（无法归类，回退到默认处理）
    Unknown,
}

impl ErrorKind {
    /// 返回该错误类型的中文标签，用于日志或提示前缀。
    pub fn label(&self) -> &'static str {
        match self {
            ErrorKind::PermissionDenied => "权限不足",
            ErrorKind::PathNotFound => "路径不存在",
            ErrorKind::CommandNotFound => "命令不存在",
            ErrorKind::Timeout => "超时",
            ErrorKind::InvalidArgument => "参数错误",
            ErrorKind::Unknown => "未知错误",
        }
    }
}

/// 将原始错误文本分类为 `ErrorKind`
///
/// 采用「关键词命中」策略：对错误文本做不区分大小写的子串匹配。
/// 匹配顺序很重要——更具体/更明确的分类优先，避免泛化关键词（如 "not found"）
/// 吞掉更精确的（如 "command not found"）。
///
/// 匹配顺序：
///   1. 超时          —— 最明确，避免 "timeout" 被其他类别误判
///   2. 权限不足      —— "Access Denied" / "denied" / "拒绝访问" / "权限"
///   3. 命令不存在    —— "command not found" / "不是内部或外部命令" / "未找到命令"
///   4. 参数错误      —— "invalid argument" / "参数" / "缺少" / "类型"
///   5. 路径不存在    —— "not found" / "找不到" / "不存在" / "no such file"
///   6. 未知错误      —— 兜底
pub fn classify_error(error_text: &str) -> ErrorKind {
    // 统一转小写，便于做不区分大小写的匹配。
    let lower = error_text.to_lowercase();

    // 1) 超时：最明确，先判。
    if lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("超时")
    {
        return ErrorKind::Timeout;
    }

    // 2) 权限不足。
    if lower.contains("access denied")
        || lower.contains("access is denied")
        || lower.contains("permission denied")
        || lower.contains("eacces")
        || lower.contains("denied")
        || lower.contains("拒绝访问")
        || lower.contains("权限不足")
        || lower.contains("没有权限")
        || lower.contains("需要管理员")
        || lower.contains("管理员权限")
    {
        return ErrorKind::PermissionDenied;
    }

    // 3) 命令不存在：必须在 "not found" 之前判，否则会被误归为路径不存在。
    if lower.contains("command not found")
        || lower.contains("command not found in")
        || lower.contains("不是内部或外部命令")
        || lower.contains("不是内部或外部命令，也不是可运行的程序")
        || lower.contains("未找到命令")
        || lower.contains("无法识别")
        || lower.contains("no such command")
    {
        return ErrorKind::CommandNotFound;
    }

    // 4) 参数错误。
    if lower.contains("invalid argument")
        || lower.contains("invalid parameter")
        || lower.contains("参数错误")
        || lower.contains("参数无效")
        || lower.contains("缺少参数")
        || lower.contains("缺少必要")
        || lower.contains("参数缺失")
        || lower.contains("类型不匹配")
        || lower.contains("类型错误")
        || lower.contains("argument error")
    {
        return ErrorKind::InvalidArgument;
    }

    // 5) 路径/文件不存在：最泛化的 "not found" 放这里。
    if lower.contains("not found")
        || lower.contains("no such file")
        || lower.contains("no such directory")
        || lower.contains("找不到")
        || lower.contains("不存在")
        || lower.contains("无法找到")
        || lower.contains("enoent")
        || lower.contains("文件或目录不存在")
    {
        return ErrorKind::PathNotFound;
    }

    // 6) 兜底。
    ErrorKind::Unknown
}

/// 根据错误类型 + 工具名 + 参数，产出一条给 LLM 的中文「下一步」建议
///
/// 返回 `Option<String>`：`Some` 表示有明确的可替代方案，`None` 表示
/// 当前信息不足以给出建议（例如 Unknown，或参数缺失无法判断）。
/// 建议文本用中文书写，直接拼进回传 LLM 的消息里即可。
pub fn suggest_fallback(
    kind: &ErrorKind,
    tool_name: &str,
    args: &HashMap<String, String>,
) -> Option<String> {
    match kind {
        ErrorKind::PermissionDenied => Some(format!(
            "工具「{tool_name}」执行失败（权限不足）。请提示用户以管理员身份重试，\
             或改用无需管理员权限的替代工具；若涉及系统文件，请先向用户确认是否允许提升权限。"
        )),

        ErrorKind::PathNotFound => {
            // 尽力提取用户给的路径参数，让提示更具体。
            let path_hint = args
                .get("path")
                .or_else(|| args.get("input"))
                .map(|s| format!("（你传入的路径：{s}）"))
                .unwrap_or_default();
            Some(format!(
                "工具「{tool_name}」执行失败（路径不存在）。请先确认路径是否正确、\
                 文件/目录是否已被移动或删除{path_hint}；必要时先用列举/搜索工具确认实际位置后再重试。"
            ))
        }

        ErrorKind::CommandNotFound => Some(format!(
            "工具「{tool_name}」执行失败（命令不存在）。该命令可能未安装或不在 PATH 中。\
             请改用系统中已安装的等价命令，或建议用户先安装对应工具后重试。"
        )),

        ErrorKind::Timeout => Some(format!(
            "工具「{tool_name}」执行超时。请尝试缩小操作范围（更少文件、更短路径或更具体的过滤条件）\
             后重试；若为长时间任务，可拆分成多个小步骤分次执行。"
        )),

        ErrorKind::InvalidArgument => Some(format!(
            "工具「{tool_name}」执行失败（参数错误）。请检查传入参数的类型、拼写与取值范围是否正确，\
             并参考工具描述中的参数说明修正后重试。"
        )),

        ErrorKind::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 便捷构造：把 &[(&str, &str)] 转成 HashMap<String, String>。
    fn args(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    // ---------- classify_error 测试 ----------

    #[test]
    fn 分类_权限不足_英文() {
        assert_eq!(
            classify_error("Access Denied: cannot open file"),
            ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn 分类_权限不足_中文() {
        assert_eq!(
            classify_error("拒绝访问：权限不足"),
            ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn 分类_路径不存在_英文() {
        assert_eq!(
            classify_error("The system cannot find the file specified: not found"),
            ErrorKind::PathNotFound
        );
    }

    #[test]
    fn 分类_路径不存在_中文() {
        assert_eq!(
            classify_error("找不到指定的文件"),
            ErrorKind::PathNotFound
        );
    }

    #[test]
    fn 分类_命令不存在_英文() {
        assert_eq!(
            classify_error("bash: foo: command not found"),
            ErrorKind::CommandNotFound
        );
    }

    #[test]
    fn 分类_命令不存在_中文() {
        assert_eq!(
            classify_error("'foo' 不是内部或外部命令，也不是可运行的程序"),
            ErrorKind::CommandNotFound
        );
    }

    #[test]
    fn 分类_超时() {
        assert_eq!(
            classify_error("operation timed out after 30 seconds"),
            ErrorKind::Timeout
        );
    }

    #[test]
    fn 分类_参数错误() {
        assert_eq!(
            classify_error("invalid argument: expected number, got string"),
            ErrorKind::InvalidArgument
        );
    }

    #[test]
    fn 分类_未知错误() {
        assert_eq!(
            classify_error("something went terribly wrong"),
            ErrorKind::Unknown
        );
    }

    #[test]
    fn 分类_命令不存在优先于路径不存在() {
        // "command not found" 里也含 "not found"，应优先归为命令不存在。
        assert_eq!(
            classify_error("command not found"),
            ErrorKind::CommandNotFound
        );
    }

    #[test]
    fn 分类_大小写不敏感() {
        assert_eq!(
            classify_error("ACCESS DENIED"),
            ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn 分类_空文本回退未知() {
        assert_eq!(classify_error(""), ErrorKind::Unknown);
    }

    // ---------- suggest_fallback 测试 ----------

    #[test]
    fn 建议_权限不足() {
        let s = suggest_fallback(
            &ErrorKind::PermissionDenied,
            "toolbox_clean-temp",
            &args(&[]),
        );
        assert!(s.unwrap().contains("管理员身份"));
    }

    #[test]
    fn 建议_路径不存在_带路径提示() {
        let s = suggest_fallback(
            &ErrorKind::PathNotFound,
            "toolbox_open-file",
            &args(&[("path", "C:\\不存在\\a.txt")]),
        );
        let text = s.unwrap();
        assert!(text.contains("路径是否正确"));
        assert!(text.contains("C:\\不存在\\a.txt"));
    }

    #[test]
    fn 建议_命令不存在() {
        let s = suggest_fallback(
            &ErrorKind::CommandNotFound,
            "toolbox_run-cmd",
            &args(&[]),
        );
        assert!(s.unwrap().contains("未安装"));
    }

    #[test]
    fn 建议_超时() {
        let s = suggest_fallback(&ErrorKind::Timeout, "toolbox_search", &args(&[]));
        assert!(s.unwrap().contains("缩小操作范围"));
    }

    #[test]
    fn 建议_参数错误() {
        let s = suggest_fallback(
            &ErrorKind::InvalidArgument,
            "toolbox_convert",
            &args(&[]),
        );
        assert!(s.unwrap().contains("参数"));
    }

    #[test]
    fn 建议_未知错误返回None() {
        assert!(suggest_fallback(&ErrorKind::Unknown, "any", &args(&[])).is_none());
    }

    #[test]
    fn 标签_各类型中文标签() {
        assert_eq!(ErrorKind::PermissionDenied.label(), "权限不足");
        assert_eq!(ErrorKind::PathNotFound.label(), "路径不存在");
        assert_eq!(ErrorKind::CommandNotFound.label(), "命令不存在");
        assert_eq!(ErrorKind::Timeout.label(), "超时");
        assert_eq!(ErrorKind::InvalidArgument.label(), "参数错误");
        assert_eq!(ErrorKind::Unknown.label(), "未知错误");
    }
}
