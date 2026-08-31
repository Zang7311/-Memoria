// 《铃·记忆体》AI-5 插件执行器
// 三种执行模式：
// 1. `js:<name>` / 无前缀带 main → boa_engine 嵌入 JS 引擎执行（独立线程 + 30s 超时）
// 2. `command:<命令>` → tokio::process 系统命令（终端命令扩展，需 system 权限）
// 3. `builtin:<动作>` → 内置 Rust 动作
//
// 安全：JS 在独立线程运行，崩溃/死循环不影响主程序；超时强制返回错误；
// 沙箱不注入 process/require 等危险全局对象，白名单 IPC 见 sandbox.rs。
// command: 动作默认不经任何 shell（argv 数组直接 exec），详见 run_system_command。
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

/// 命令模板分词：按空白切分，支持单/双引号包裹含空格的整体参数。
///
/// 只对**插件作者写死的模板**分词，不对用户传入的参数值分词，
/// 因此参数值里的引号/空格不会改变 argv 结构。
fn tokenize_template(template: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut has_cur = false;
    let mut quote: Option<char> = None;

    for ch in template.chars() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                } else {
                    cur.push(ch);
                }
            }
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                has_cur = true; // 支持空字符串参数 ""
            }
            None if ch.is_whitespace() => {
                if has_cur {
                    tokens.push(std::mem::take(&mut cur));
                    has_cur = false;
                }
            }
            None => {
                cur.push(ch);
                has_cur = true;
            }
        }
    }
    if has_cur {
        tokens.push(cur);
    }
    tokens
}

/// 参数值转字符串（字符串取原值，其余走 JSON 表示）
fn param_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// 在**单个 token 内**替换 {参数名} 占位符。
///
/// 关键安全性质：替换后的结果始终作为一个独立 argv 元素传递，
/// 值里的 `& | > < ; $` 等 shell 元字符不再具有任何语法含义。
fn substitute_token(token: &str, params: &HashMap<String, Value>) -> String {
    let mut out = token.to_string();
    for (k, v) in params {
        let placeholder = format!("{{{k}}}");
        if out.contains(&placeholder) {
            out = out.replace(&placeholder, &param_to_string(v));
        }
    }
    out
}

/// 把命令模板 + 参数展开为 argv 数组（不经 shell）
///
/// 返回 (程序名, 参数列表)。模板为空时报错。
fn build_argv(
    template: &str,
    params: &HashMap<String, Value>,
) -> Result<(String, Vec<String>), AppError> {
    let tokens = tokenize_template(template);
    let mut argv: Vec<String> = tokens
        .iter()
        .map(|t| substitute_token(t, params))
        .collect();
    if argv.is_empty() {
        return Err(AppError::PluginExecutionError(
            "command: 动作为空，没有可执行的命令".into(),
        ));
    }
    let program = argv.remove(0);
    if program.trim().is_empty() {
        return Err(AppError::PluginExecutionError(
            "command: 动作的可执行程序名为空".into(),
        ));
    }
    Ok((program, argv))
}

/// 执行系统命令（支持 {参数名} 占位符替换；30s 超时）
///
/// 安全（修复命令注入）：**不经 shell**。命令模板先分词为 argv，
/// 用户参数只填入单个 argv 槽位，再用 Command::arg 逐个传递。
/// 因此参数中的 `&` `|` `>` `;` 等元字符只是普通字符，无法拼出新命令。
///
/// 代价：`command:` 动作不再支持管道/重定向/`del` 等 shell 内置命令。
/// 需要这类能力的插件应把脚本写成 `.bat`/`.ps1` 文件并直接调用该文件。
async fn run_system_command(cmd: &str, params: &HashMap<String, Value>) -> Result<String, AppError> {
    let (program, args) = build_argv(cmd, params)?;

    let mut builder = tokio::process::Command::new(&program);
    // 逐个 arg 传参：由操作系统直接 exec，不经 cmd/sh 解析
    for a in &args {
        builder.arg(a);
    }
    builder.kill_on_drop(true);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        builder.creation_flags(0x08000000); // CREATE_NO_WINDOW 隐藏控制台
    }

    let display = if args.is_empty() {
        program.clone()
    } else {
        format!("{program} {}", args.join(" "))
    };

    let output = match tokio::time::timeout(
        Duration::from_secs(EXEC_TIMEOUT_SECS),
        builder.output(),
    )
    .await
    {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            return Err(AppError::PluginExecutionError(format!(
                "命令「{program}」启动失败：{e}（提示：command: 动作不经 shell，无法直接使用 del/dir/copy 等 cmd 内置命令，请改为调用 .bat/.ps1 脚本）"
            )));
        }
        Err(_) => return Err(AppError::PluginTimeout(display)),
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
    fn js执行_未授权权限被拒绝() {
        let code = "globalThis.skills = { go: function(){ return invoke_plugin('file.search', {keyword:'x'}); } };";
        // 无任何权限 → 拒绝
        let r = run_js_blocking(code, "go", "{}", &[]);
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("权限"));
    }

    // ==================== 命令注入修复回归测试 ====================

    fn p(pairs: &[(&str, &str)]) -> HashMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), Value::from(*v)))
            .collect()
    }

    #[test]
    fn 分词_基础与引号() {
        assert_eq!(tokenize_template("ping -n 1 host"), vec!["ping", "-n", "1", "host"]);
        // 双引号包裹含空格的整体参数
        assert_eq!(
            tokenize_template(r#"prog "a b" c"#),
            vec!["prog", "a b", "c"]
        );
        // 单引号同理
        assert_eq!(tokenize_template("prog 'x y'"), vec!["prog", "x y"]);
        // 多余空白被折叠
        assert_eq!(tokenize_template("  prog   a  "), vec!["prog", "a"]);
    }

    #[test]
    fn 注入_shell元字符不再拆出新命令() {
        // 经典注入 payload：& 想串接第二条命令
        let params = p(&[("host", "127.0.0.1 & calc.exe")]);
        let (program, args) = build_argv("ping {host}", &params).unwrap();
        assert_eq!(program, "ping");
        // 整个恶意串仍是**一个** argv 元素，不会被解释为命令分隔
        assert_eq!(args, vec!["127.0.0.1 & calc.exe"]);
        assert!(!args.iter().any(|a| a == "calc.exe"));
    }

    #[test]
    fn 注入_管道与重定向被当作纯数据() {
        for payload in [
            "x | whoami",
            "x > C:\\Windows\\System32\\evil.txt",
            "x && shutdown /s",
            "x; rm -rf /",
            "$(whoami)",
            "`whoami`",
        ] {
            let params = p(&[("q", payload)]);
            let (program, args) = build_argv("findstr {q}", &params).unwrap();
            assert_eq!(program, "findstr");
            // 参数值原样保留为单个 token，未被切分成额外 argv
            assert_eq!(args, vec![payload.to_string()], "payload 被拆分：{payload}");
        }
    }

    #[test]
    fn 注入_参数无法篡改程序名() {
        // 参数值即使含空格，也只能落在它自己的槽位里，改不了 argv[0]
        let params = p(&[("arg", "calc.exe /c evil")]);
        let (program, _args) = build_argv("ping {arg}", &params).unwrap();
        assert_eq!(program, "ping");
    }

    #[test]
    fn 注入_参数中的引号不改变argv结构() {
        // 参数值自带引号：不参与分词，原样保留
        let params = p(&[("q", r#"a" "b"#)]);
        let (_program, args) = build_argv("findstr {q}", &params).unwrap();
        assert_eq!(args, vec![r#"a" "b"#.to_string()]);
    }

    #[test]
    fn 占位符_多参数与非字符串值() {
        let mut params = HashMap::new();
        params.insert("n".to_string(), Value::from(3));
        params.insert("host".to_string(), Value::from("example.com"));
        let (program, args) = build_argv("ping -n {n} {host}", &params).unwrap();
        assert_eq!(program, "ping");
        assert_eq!(args, vec!["-n", "3", "example.com"]);
    }

    #[test]
    fn 空模板报错() {
        assert!(build_argv("", &HashMap::new()).is_err());
        assert!(build_argv("   ", &HashMap::new()).is_err());
    }

    #[tokio::test]
    async fn 执行_未授权system权限被拒绝() {
        let plugin = Plugin {
            id: "t".into(),
            name: "测试插件".into(),
            version: "0.1.0".into(),
            author: "test".into(),
            description: String::new(),
            enabled: true,
            path: ".".into(),
            granted: vec![],
            manifest: crate::types::PluginManifest {
                main: String::new(),
                skills: vec![],
                permissions: vec!["system".into()],
                hermes_compatible: false,
            },
        };
        let skill = Skill {
            name: "danger".into(),
            description: String::new(),
            parameters: vec![],
            action: "command:ping 127.0.0.1".into(),
        };
        // granted 为空 → system 未授予 → 必须拒绝执行
        let resp = execute_skill(&plugin, &skill, &[], HashMap::new()).await;
        assert!(!resp.success);
        assert!(resp.error.unwrap_or_default().contains("权限"));
    }
}
