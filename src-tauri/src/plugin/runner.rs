// 《铃·记忆体》AI-5 插件执行器
// 三种执行模式：
// 1. `js:<name>` / 无前缀带 main → boa_engine 嵌入 JS 引擎执行（独立线程 + 30s 超时）
// 2. `command:<命令>` → tokio::process 系统命令（终端命令扩展，需 system 权限）
// 3. `builtin:<动作>` → 内置 Rust 动作
//
// 安全：JS 在独立线程运行，崩溃/死循环不影响主程序；超时强制返回错误；
// 沙箱不注入 process/require 等危险全局对象，白名单 IPC 见 sandbox.rs。
use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;

use crate::error::AppError;
use crate::plugin::sandbox;
use crate::types::{ExecuteSkillResponse, Plugin, Skill};

/// 插件执行超时（任务书默认 30 秒）
pub const EXEC_TIMEOUT_SECS: u64 = 30;

/// 执行技能入口（async，超时保护）
pub async fn execute_skill(
    plugin: &Plugin,
    skill: &Skill,
    granted: &[String],
    params: HashMap<String, Value>,
) -> ExecuteSkillResponse {
    let action = skill.action.trim();
    let result = if let Some(cmd) = action.strip_prefix("command:") {
        if let Err(e) = crate::plugin::permissions::check_granted(granted, crate::plugin::permissions::PERM_SYSTEM) {
            Err(e)
        } else {
            run_system_command(cmd, &params).await
        }
    } else if let Some(name) = action.strip_prefix("builtin:") {
        if let Err(e) = crate::plugin::permissions::required_permission(action)
            .map(|perm| crate::plugin::permissions::check_granted(granted, perm))
            .unwrap_or(Ok(()))
        {
            Err(e)
        } else {
            run_builtin(name, &params)
        }
    } else if plugin.manifest.main.trim().is_empty() {
        Err(AppError::PluginExecutionError(
            "该技能没有可执行的入口（main 为空且非 command:/builtin: 动作）".into(),
        ))
    } else {
        run_js(plugin, skill, &params, granted).await
    };

    match result {
        Ok(text) => {
            log::info!("技能「{}」（插件：{}）执行成功", skill.name, plugin.name);
            ExecuteSkillResponse {
                success: true,
                result: Some(text),
                error: None,
            }
        }
        Err(e) => {
            log::warn!("技能「{}」（插件：{}）执行失败：{}", skill.name, plugin.name, e);
            ExecuteSkillResponse {
                success: false,
                result: None,
                error: Some(e.to_string()),
            }
        }
    }
}

/// 把参数值转成 cmd /C 下的安全字面量：整体用双引号包裹，值内的双引号翻倍转义。
///
/// cmd.exe 的引号是「开关」语义 —— 每个 `"` 切换一次引用状态。值内 `"` 写成 `""`
/// 之后，`& | < > ^ ( )` 始终落在引用区内，失去命令分隔/重定向语义。例如
/// `a" & calc.exe & "b` → `"a"" & calc.exe & ""b"`，其中 ` & calc.exe & ` 仍在引号内，
/// calc 不会被执行。
///
/// 换行/回车/NUL 无法用引号表达（换行会直接另起一条命令），只能拒绝。
///
/// 已知残留：`%VAR%` 在双引号内仍会被 cmd 展开（`cmd /C` 下 `%%` 不是有效转义）。
/// 这只会把环境变量内容代入命令，不构成命令注入；要彻底消除需改为分离传参、不走 cmd /C。
fn quote_cmd_arg(key: &str, val: &str) -> Result<String, AppError> {
    if val.contains('\n') || val.contains('\r') || val.contains('\0') {
        return Err(AppError::PluginExecutionError(format!(
            "参数「{key}」含换行或空字符，已拒绝执行（防命令注入）"
        )));
    }
    Ok(format!("\"{}\"", val.replace('"', "\"\"")))
}

/// 执行系统命令（Windows 下通过 cmd /C；支持 {参数名} 占位符替换；30s 超时）
///
/// 安全：占位符替换后整条字符串交给 `cmd /C`，等价于 shell 拼接。因此每个参数值
/// 都先过 `quote_cmd_arg` 用双引号包裹并把值内双引号翻倍，
/// 避免 `& calc.exe`、`| del /F /S /Q C:\` 之类的注入。
/// 命令模板本身（manifest 的 action）仍按原样执行——那是插件作者声明的意图，
/// 且执行前已由 permissions 校验过 `system` 权限。
async fn run_system_command(cmd: &str, params: &HashMap<String, Value>) -> Result<String, AppError> {
    let mut expanded = cmd.to_string();
    for (k, v) in params {
        let placeholder = format!("{{{k}}}");
        if expanded.contains(&placeholder) {
            let val = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let quoted = quote_cmd_arg(k, &val)?;
            expanded = expanded.replace(&placeholder, &quoted);
        }
    }

    let command = tokio::process::Command::new("cmd")
        .args(["/C", &expanded])
        .output();

    let output = match tokio::time::timeout(Duration::from_secs(EXEC_TIMEOUT_SECS), command).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            return Err(AppError::PluginExecutionError(format!("命令启动失败：{e}")));
        }
        Err(_) => return Err(AppError::PluginTimeout(expanded.clone())),
    };

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let code = output.status.code();
    let text = if !stdout.is_empty() {
        stdout
    } else if !stderr.is_empty() {
        stderr
    } else {
        format!("执行完毕（退出码 {code:?}）")
    };
    if code == Some(0) {
        Ok(text)
    } else {
        Ok(format!("命令退出码 {code:?}：{text}"))
    }
}

/// 内置 Rust 动作（预留接口：文件检索基础版已实现，其余返回"未实现"）
fn run_builtin(name: &str, params: &HashMap<String, Value>) -> Result<String, AppError> {
    match name {
        "file_search" => sandbox::invoke_whitelisted(&[], "file.search", params),
        _ => Err(AppError::PluginExecutionError(format!(
            "内置动作「{name}」暂未实现（接口已预留）"
        ))),
    }
}

// ==================== JS 引擎（boa_engine）执行 ====================

/// JS 执行：独立线程 + 30s 超时（线程无法强杀，但主程序不受影响）
async fn run_js(
    plugin: &Plugin,
    skill: &Skill,
    params: &HashMap<String, Value>,
    granted: &[String],
) -> Result<String, AppError> {
    let entry = std::path::Path::new(&plugin.path).join(&plugin.manifest.main);
    let code = std::fs::read_to_string(&entry)
        .map_err(|e| AppError::PluginExecutionError(format!("读取入口文件失败：{e}")))?;

    let params_json = serde_json::to_string(params)
        .map_err(|e| AppError::PluginExecutionError(format!("参数序列化失败：{e}")))?;
    let skill_name = skill.name.clone();
    let granted = granted.to_vec();

    let handle = tokio::task::spawn_blocking(move || {
        run_js_blocking(&code, &skill_name, &params_json, &granted)
    });

    match tokio::time::timeout(Duration::from_secs(EXEC_TIMEOUT_SECS), handle).await {
        Ok(Ok(res)) => res,
        Ok(Err(e)) => Err(AppError::PluginExecutionError(format!(
            "JS 执行线程异常：{e}"
        ))),
        Err(_) => Err(AppError::PluginTimeout(plugin.name.clone())),
    }
}

/// 在 boa_engine 中执行插件代码并调用技能（阻塞，须在独立线程运行）
fn run_js_blocking(
    code: &str,
    skill_name: &str,
    params_json: &str,
    granted: &[String],
) -> Result<String, AppError> {
    use boa_engine::native_function::NativeFunction;
    use boa_engine::{Context, JsError, JsString, JsValue, Source};

    let mut context = Context::default();

    // M-2：设置资源限制，防止插件无限循环或深递归耗尽资源
    context.runtime_limits_mut().set_loop_iteration_limit(100_000);
    context.runtime_limits_mut().set_recursion_limit(256);

    // 1. 注入 console 日志（捕获到主日志）
    let console_log = NativeFunction::from_copy_closure(
        |_this: &JsValue, args: &[JsValue], ctx: &mut Context| {
            let parts: Vec<String> = args
                .iter()
                .map(|v| {
                    v.to_string(ctx)
                        .map(|s| s.to_std_string().unwrap_or_default())
                        .unwrap_or_else(|_| "[无法显示]".to_string())
                })
                .collect();
            log::info!("[插件JS] {}", parts.join(" "));
            Ok(JsValue::undefined())
        },
    );
    context.register_global_builtin_callable("__console_log".into(), 1, console_log)?;

    // 2. 注入 invoke_plugin 白名单函数（权限由 sandbox 校验）
    let invoke_fn = NativeFunction::from_copy_closure_with_captures(
        move |_this: &JsValue, args: &[JsValue], granted: &Vec<String>, ctx: &mut Context| {
            let name = args
                .first()
                .map(|v| {
                    v.to_string(ctx)
                        .map(|s| s.to_std_string().unwrap_or_default())
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            let params: HashMap<String, Value> = args
                .get(1)
                .map(|v| {
                    v.to_json(ctx)
                        .map(|j| serde_json::from_value(j).unwrap_or_default())
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            match sandbox::invoke_whitelisted(granted, &name, &params) {
                Ok(text) => Ok(JsValue::from(JsString::from(text))),
                Err(e) => Err(JsError::from_opaque(JsValue::from(JsString::from(
                    e.to_string(),
                )))),
            }
        },
        granted.to_vec(),
    );
    context.register_global_builtin_callable("invoke_plugin".into(), 2, invoke_fn)?;

    // 3. 胶水代码：定义 console 对象（插件里习惯用 console.log）
    let glue = r#"
        globalThis.console = {
            log: function () { __console_log(Array.prototype.slice.call(arguments).join(' ')); },
            error: function () { __console_log('[error] ' + Array.prototype.slice.call(arguments).join(' ')); },
            warn: function () { __console_log('[warn] ' + Array.prototype.slice.call(arguments).join(' ')); },
            info: function () { __console_log('[info] ' + Array.prototype.slice.call(arguments).join(' ')); }
        };
    "#;
    context.eval(Source::from_bytes(glue.as_bytes()))?;

    // 4. 执行插件入口（插件应定义 globalThis.skills = { 技能名: function(params){...} }）
    context.eval(Source::from_bytes(code.as_bytes())).map_err(|e| {
        AppError::PluginExecutionError(format!("插件代码执行失败：{e}"))
    })?;

    // 5. 调用目标技能，结果 JSON 序列化返回
    let skill_json = serde_json::to_string(skill_name)
        .map_err(|e| AppError::PluginExecutionError(format!("技能名序列化失败：{e}")))?;
    let call_code = format!(
        r#"(function(){{
            if (!globalThis.skills) {{
                return JSON.stringify({{ error: '插件未定义 skills 对象（入口需执行 globalThis.skills = {{...}}）' }});
            }}
            var fn = globalThis.skills[{skill_json}];
            if (typeof fn !== 'function') {{
                return JSON.stringify({{ error: '技能 {skill_json} 未在插件中定义' }});
            }}
            var __params = {params_json};
            var __ret = fn(__params);
            return JSON.stringify({{ result: __ret === undefined ? null : __ret }});
        }})()"#
    );
    let result = context.eval(Source::from_bytes(call_code.as_bytes())).map_err(|e| {
        AppError::PluginExecutionError(format!("技能执行抛错：{e}"))
    })?;
    let text = result
        .to_string(&mut context)
        .map(|s| s.to_std_string().unwrap_or_default())
        .map_err(|e| AppError::PluginExecutionError(format!("结果转换失败：{e}")))?;

    // 解析包装 JSON，取出 result 或 error
    let wrapped: Value = serde_json::from_str(&text).unwrap_or(Value::String(text));
    if let Some(err) = wrapped.get("error").and_then(|v| v.as_str()) {
        return Err(AppError::PluginExecutionError(err.to_string()));
    }
    let inner = wrapped.get("result").cloned().unwrap_or(Value::Null);
    Ok(inner.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js执行_简单函数() {
        let code = "globalThis.skills = { add: function(p){ return p.a + p.b; } };";
        let mut params = HashMap::new();
        params.insert("a".to_string(), Value::from(1));
        params.insert("b".to_string(), Value::from(2));
        let r = run_js_blocking(code, "add", &serde_json::to_string(&params).unwrap(), &[]).unwrap();
        assert!(r.contains("3"));
    }

    #[test]
    fn js执行_字符串返回() {
        let code = "globalThis.skills = { hi: function(p){ return '你好，' + p.who; } };";
        let mut params = HashMap::new();
        params.insert("who".to_string(), Value::from("主人"));
        let r = run_js_blocking(code, "hi", &serde_json::to_string(&params).unwrap(), &[]).unwrap();
        assert!(r.contains("你好，主人"));
    }

    #[test]
    fn js执行_技能未定义报错() {
        let code = "globalThis.skills = {};";
        let r = run_js_blocking(code, "nope", "{}", &[]);
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("未在插件中定义"));
    }

    #[test]
    fn js执行_插件抛异常被捕获() {
        let code = "globalThis.skills = { boom: function(){ throw new Error('炸了'); } };";
        let r = run_js_blocking(code, "boom", "{}", &[]);
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("炸了"));
    }

    #[test]
    fn js执行_console日志与invoke白名单() {
        let code = r#"
            globalThis.skills = {
                go: function(p){
                    console.log('日志测试', p.msg);
                    return invoke_plugin('echo', { text: 'echo测试' });
                }
            };
        "#;
        let mut params = HashMap::new();
        params.insert("msg".to_string(), Value::from("你好"));
        let r = run_js_blocking(code, "go", &serde_json::to_string(&params).unwrap(), &[]).unwrap();
        assert!(r.contains("echo测试"));
    }

    #[test]
    fn 参数转义_普通值加引号() {
        assert_eq!(quote_cmd_arg("p", "hello").unwrap(), "\"hello\"");
        // & | < > ^ ( ) 落在引号内，失去 shell 语义
        assert_eq!(
            quote_cmd_arg("p", "a & calc.exe").unwrap(),
            "\"a & calc.exe\""
        );
        assert_eq!(quote_cmd_arg("p", "a|b<c>d").unwrap(), "\"a|b<c>d\"");
    }

    #[test]
    fn 参数转义_内部双引号翻倍() {
        // 经典闭合注入：值内的 " 翻倍后无法闭合外层引号，& calc.exe 仍在引号内
        assert_eq!(
            quote_cmd_arg("p", "a\" & calc.exe & \"b").unwrap(),
            "\"a\"\" & calc.exe & \"\"b\""
        );
        // 单个双引号
        assert_eq!(quote_cmd_arg("p", "\"").unwrap(), "\"\"\"\"");
    }

    #[test]
    fn 参数转义_换行被拒绝() {
        // 换行会另起一条命令，引号无法表达 → 拒绝
        assert!(quote_cmd_arg("p", "a\ncalc.exe").is_err());
        assert!(quote_cmd_arg("p", "a\r\ncalc.exe").is_err());
        assert!(quote_cmd_arg("p", "a\0b").is_err());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn 系统命令_注入参数被包裹转义() {
        // 注入值不再逃出引号：命令实际执行 echo "x"" & calc.exe & """，
        // 只回显字面文本，不会启动 calc
        let mut params = HashMap::new();
        params.insert("name".to_string(), Value::from("x\" & calc.exe & \""));
        let r = run_system_command("echo {name}", &params).await.unwrap();
        assert!(r.contains("calc.exe"), "应作为文本回显，实际：{r}");
    }

    // 纯校验逻辑，不实际起进程，跨平台可跑
    #[tokio::test]
    async fn 系统命令_换行参数被拒绝执行() {
        let mut params = HashMap::new();
        params.insert("name".to_string(), Value::from("x\ncalc.exe"));
        let r = run_system_command("echo {name}", &params).await;
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("换行"));
    }

    #[test]
    fn js执行_未授权权限被拒绝() {
        let code = "globalThis.skills = { go: function(){ return invoke_plugin('file.search', {keyword:'x'}); } };";
        // 无任何权限 → 拒绝
        let r = run_js_blocking(code, "go", "{}", &[]);
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("权限"));
    }
}
