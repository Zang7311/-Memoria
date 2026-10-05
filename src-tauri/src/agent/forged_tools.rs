// Agent 自制工具：持久化注册表、统一沙箱与安全拦截。
use std::collections::HashMap;
use std::io::{ErrorKind, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::error::AppError;

const MAX_CODE_BYTES: usize = 256 * 1024;
const MAX_ARGS_BYTES: usize = 64 * 1024;
const MAX_OUTPUT_BYTES: usize = 8 * 1024;
const MAX_FORGE_ATTEMPTS: u8 = 3;
pub const FORGED_TOOL_PREFIX: &str = "forged_";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgedTool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub params: Value,
    pub language: String,
    pub code: String,
    pub created_at: i64,
    pub call_count: u64,
    pub last_error: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteForgedToolRequest { pub id: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetForgedToolEnabledRequest { pub id: String, pub enabled: bool }

#[derive(Debug, Clone)]
struct ProcessOutput { stdout: String, stderr: String }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptLanguage { Python, PowerShell, Node }
impl ScriptLanguage {
    fn parse(value: &str) -> Result<Self, AppError> {
        match value { "python" => Ok(Self::Python), "powershell" => Ok(Self::PowerShell), "node" => Ok(Self::Node), _ => Err(AppError::ToolboxError("只支持 python、powershell、node 三种语言".into())) }
    }
}

static REGISTRY: OnceLock<RwLock<Vec<ForgedTool>>> = OnceLock::new();
static FORGE_ATTEMPTS: OnceLock<Mutex<HashMap<String, u8>>> = OnceLock::new();
fn registry() -> &'static RwLock<Vec<ForgedTool>> { REGISTRY.get_or_init(|| RwLock::new(Vec::new())) }
fn forge_attempts() -> &'static Mutex<HashMap<String, u8>> { FORGE_ATTEMPTS.get_or_init(|| Mutex::new(HashMap::new())) }
fn storage_path() -> PathBuf {
    #[cfg(test)]
    if let Some(path) = TEST_STORAGE_PATH.get() { return path.clone(); }
    crate::config::data_dir().join("forged_tools.json")
}

#[cfg(test)]
static TEST_STORAGE_PATH: OnceLock<PathBuf> = OnceLock::new();
#[cfg(test)]
static TEST_LIFECYCLE_MUTEX: Mutex<()> = Mutex::new(());

pub fn init() {
    if let Err(error) = reload() { log::error!("[forge] 自制工具加载失败：{}", safe_log_text(&error.to_string())); }
}
pub fn reload() -> Result<(), AppError> {
    let path = storage_path();
    let tools = if !path.exists() { Vec::new() } else {
        let text = std::fs::read_to_string(&path).map_err(|e| AppError::ConfigLoadError(format!("自制工具读取失败：{e}")))?;
        serde_json::from_str::<Vec<ForgedTool>>(&text).map_err(|e| AppError::ConfigLoadError(format!("自制工具文件格式错误：{e}")))?
    };
    *registry().write().unwrap_or_else(|poison| poison.into_inner()) = tools;
    Ok(())
}
fn save_tools(tools: &[ForgedTool]) -> Result<(), AppError> {
    let path = storage_path();
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).map_err(|e| AppError::ConfigSaveError(format!("自制工具目录创建失败：{e}")))?; }
    let text = serde_json::to_string_pretty(tools).map_err(|e| AppError::ConfigSaveError(format!("自制工具序列化失败：{e}")))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| AppError::ConfigSaveError(format!("自制工具临时文件写入失败：{e}")))?;
    if let Err(error) = std::fs::rename(&tmp, &path) {
        #[cfg(windows)]
        let replacement = if error.kind() == ErrorKind::AlreadyExists && path.exists() {
            std::fs::remove_file(&path).and_then(|_| std::fs::rename(&tmp, &path))
        } else {
            Err(error)
        };
        #[cfg(not(windows))]
        let replacement: Result<(), std::io::Error> = Err(error);
        if let Err(replacement_error) = replacement {
            let _ = std::fs::remove_file(&tmp);
            return Err(AppError::ConfigSaveError(format!("自制工具文件替换失败：{replacement_error}")));
        }
    }
    Ok(())
}
pub fn list() -> Result<Vec<ForgedTool>, AppError> { Ok(registry().read().unwrap_or_else(|poison| poison.into_inner()).clone()) }
pub fn delete(id: &str) -> Result<(), AppError> {
    let mut guard = registry().write().unwrap_or_else(|poison| poison.into_inner()); let before = guard.len(); guard.retain(|tool| tool.id != id);
    if guard.len() == before { return Err(AppError::ToolboxError("自制工具不存在".into())); }
    save_tools(&guard)?; log_event("delete", id, "已删除"); Ok(())
}
pub fn set_enabled(id: &str, enabled: bool) -> Result<(), AppError> {
    let mut guard = registry().write().unwrap_or_else(|poison| poison.into_inner());
    let tool = guard.iter_mut().find(|tool| tool.id == id).ok_or_else(|| AppError::ToolboxError("自制工具不存在".into()))?;
    tool.enabled = enabled; save_tools(&guard)?; log_event("toggle", id, if enabled { "已启用" } else { "已停用" }); Ok(())
}

pub fn tool_definitions(allow_forge: bool) -> Vec<Value> {
    let mut tools = vec![
        json!({"type":"function","x-intent":"tool.list","x-source":"native","function":{"name":"list_forged_tools","description":"列出经过用户可见审核的自制工具，避免重复编写。不要用于创建新工具，请用 forge_tool（需 tool_forge 授权）。","parameters":{"type":"object","properties":{},"required":[]}}}),
        json!({"type":"function","x-intent":"tool.delete","x-source":"native","function":{"name":"delete_forged_tool","description":"删除已注册的自制工具，不能修改人格、权限或应用配置。不要用于删除普通文件，请用 toolbox_agent_delete_file（需 file_write 授权）。","parameters":{"type":"object","properties":{"id":{"type":"string","description":"自制工具 id"}},"required":["id"]}}}),
    ];
    for tool in list().unwrap_or_default().into_iter().filter(|tool| tool.enabled) {
        tools.push(json!({"type":"function","x-intent":"tool.execute","x-source":"native","function":{"name":format!("{FORGED_TOOL_PREFIX}{}",tool.name),"description":format!("{}。不要用于其他自制技能，请用 list_forged_tools 查找。",tool.description.chars().take(80).collect::<String>()),"parameters":tool.params}}));
    }
    if allow_forge {
        tools.push(json!({"type":"function","x-intent":"tool.forge","x-source":"native","x-agent-permission":"tool_forge","function":{"name":"forge_tool","description":"编写并试跑沙箱小工具（需 tool_forge 授权），成功才注册；拒绝联网、提权、持久化、注册表写、凭据读取及破坏。不要用于重复创建已有工具，请用 list_forged_tools。","parameters":{"type":"object","properties":{"name":{"type":"string","description":"英文小写字母、数字、下划线组成的唯一名称"},"description":{"type":"string","description":"给 Agent 看的中文用途说明"},"language":{"type":"string","enum":["python","powershell","node"]},"code":{"type":"string","description":"脚本源码；参数 JSON 位于 Python sys.argv[1]、PowerShell $args[0] 或 Node process.argv[2]"},"params":{"type":"object","description":"JSON Schema 参数对象"},"test_args":{"type":"object","description":"注册前试跑使用的参数对象"}},"required":["name","description","language","code","params","test_args"]}}}));
    }
    tools
}
pub fn is_forged_call(name: &str) -> bool { name.starts_with(FORGED_TOOL_PREFIX) && name != "forge_tool" && name != "forged_tool" }

pub async fn dispatch(name: &str, args: &HashMap<String, Value>) -> Result<String, AppError> {
    if name == "list_forged_tools" {
        let tools = list()?;
        return serde_json::to_string(&tools.iter().map(|tool| json!({"id":tool.id,"name":tool.name,"description":tool.description,"language":tool.language,"created_at":tool.created_at,"call_count":tool.call_count,"enabled":tool.enabled,"last_error":tool.last_error})).collect::<Vec<_>>()).map_err(|e| AppError::InternalError(format!("自制工具列表序列化失败：{e}")));
    }
    if name == "delete_forged_tool" {
        let id = args.get("id").and_then(Value::as_str).filter(|id| !id.trim().is_empty()).ok_or_else(|| AppError::ToolboxError("删除自制工具需要提供 id".into()))?;
        delete(id)?; return Ok("自制工具已删除".into());
    }
    let tool_name = name.strip_prefix(FORGED_TOOL_PREFIX).ok_or_else(|| AppError::ToolboxError("未知自制工具".into()))?;
    if !valid_name(tool_name) {
        return Err(AppError::ToolboxError("自制工具名称无效".into()));
    }
    let tool = list()?.into_iter().find(|tool| tool.name == tool_name).ok_or_else(|| AppError::ToolboxError("自制工具不存在".into()))?;
    if !tool.enabled { return Err(AppError::ToolboxError("自制工具已停用".into())); }
    let value = Value::Object(args.clone().into_iter().collect());
    let serialized = serde_json::to_string(&value).map_err(|e| AppError::ToolboxError(format!("自制工具参数序列化失败：{e}")))?;
    let result = execute_script(&tool, &serialized, Duration::from_secs(30));
    let mut guard = registry().write().unwrap_or_else(|poison| poison.into_inner());
    if let Some(current) = guard.iter_mut().find(|current| current.id == tool.id) { current.call_count = current.call_count.saturating_add(1); current.last_error = result.as_ref().err().map(safe_error); let _ = save_tools(&guard); }
    match result { Ok(output) => Ok(combine_output(&output)), Err(error) => Err(error) }
}

pub async fn forge_from_args(args: &HashMap<String, Value>, allowed: bool, request_id: &str) -> Result<String, AppError> {
    let name = args.get("name").and_then(Value::as_str).unwrap_or("未命名工具");
    let audit = crate::audit::AuditRun::new("forge", name, Some(request_id), None, None);
    let result = audit.scope(async {
        let mut attempt = crate::audit::ToolAttempt::new("forge_tool", args, &tool_definitions(true));
        let result = forge_from_args_inner(args, allowed, request_id).await;
        attempt.finish(result.is_ok());
        result
    }).await;
    audit.finish(match &result { Ok(_) => "ok", Err(AppError::PermissionDenied(_)) => "blocked", Err(_) => "failed" });
    result
}

async fn forge_from_args_inner(args: &HashMap<String, Value>, allowed: bool, request_id: &str) -> Result<String, AppError> {
    if !allowed { return Err(AppError::PermissionDenied("未授权 Agent 自己编写小工具，请先在设置中开启".into())); }
    { let mut attempts = forge_attempts().lock().unwrap_or_else(|poison| poison.into_inner()); let count = attempts.entry(request_id.to_string()).or_default(); if *count >= MAX_FORGE_ATTEMPTS { return Err(AppError::ToolboxError("同一任务最多尝试造工具 3 次，已停止继续尝试".into())); } *count += 1; }
    let tool = parse_forged_tool(args)?;
    let test_args = args.get("test_args").and_then(Value::as_object).ok_or_else(|| AppError::ToolboxError("test_args 必须是 JSON 对象".into()))?;
    let test_args = serde_json::to_string(&Value::Object(test_args.clone())).map_err(|e| AppError::ToolboxError(format!("test_args 序列化失败：{e}")))?;
    match execute_script(&tool, &test_args, Duration::from_secs(30)) {
        Ok(output) => { let mut guard = registry().write().unwrap_or_else(|poison| poison.into_inner()); if guard.iter().any(|existing| existing.name == tool.name) { return Err(AppError::ToolboxError("自制工具名称已存在，请换一个名称".into())); } guard.push(tool.clone()); save_tools(&guard)?; log_event("forge", &tool.id, "试跑成功，已注册"); Ok(format!("造好了，可以用了。试跑输出：{}", combine_output(&output))) }
        Err(error) => { log_event("forge-reject", &tool.id, "试跑失败，未注册"); Err(error) }
    }
}

fn parse_forged_tool(args: &HashMap<String, Value>) -> Result<ForgedTool, AppError> {
    let name = required_string(args, "name")?;
    if !valid_name(&name) { return Err(AppError::ToolboxError("工具名必须是 1-64 个英文小写字母、数字或下划线，且不能以数字开头".into())); }
    let description = required_string(args, "description")?;
    if description.chars().count() > 1000 { return Err(AppError::ToolboxError("工具描述过长".into())); }
    let language = required_string(args, "language")?; ScriptLanguage::parse(&language)?;
    let code = required_string(args, "code")?;
    if code.as_bytes().len() > MAX_CODE_BYTES || code.trim().is_empty() { return Err(AppError::ToolboxError("脚本为空或超过 256KB 限制".into())); }
    validate_code(&code)?;
    let params = args.get("params").filter(|value| value.is_object()).cloned().ok_or_else(|| AppError::ToolboxError("params 必须是 JSON Schema 对象".into()))?;
    Ok(ForgedTool { id: uuid::Uuid::new_v4().to_string(), name, description, params, language, code, created_at: chrono::Utc::now().timestamp(), call_count: 0, last_error: None, enabled: true })
}
fn required_string(args: &HashMap<String, Value>, key: &str) -> Result<String, AppError> { args.get(key).and_then(Value::as_str).map(str::to_owned).filter(|value| !value.trim().is_empty()).ok_or_else(|| AppError::ToolboxError(format!("{key} 必须是非空文本"))) }
fn valid_name(name: &str) -> bool { let mut chars = name.chars(); matches!(chars.next(), Some(first) if first.is_ascii_lowercase()) && name.len() <= 64 && name.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_') && !["forge_tool","list_forged_tools","delete_forged_tool"].contains(&name) }

// 黑名单故意偏宽：脚本不能启动其他解释器/命令，也不能触碰网络、凭据、注册表或持久化。
const DENY_PATTERNS: &[(&str, &str)] = &[
    ("关机/重启","shutdown"),("关机/重启","reboot"),("关机/重启","restart-computer"),("关机/重启","stop-computer"),
    ("磁盘/系统破坏","format"),("磁盘/系统破坏","format.com"),("磁盘/系统破坏","format.exe"),("磁盘/系统破坏","diskpart"),("磁盘/系统破坏","mkfs"),("磁盘/系统破坏","rm -rf"),("磁盘/系统破坏","rm -r"),("磁盘/系统破坏","rmdir /s"),("磁盘/系统破坏","rd /s"),("磁盘/系统破坏","del /s"),("磁盘/系统破坏","remove-item -recurse"),("磁盘/系统破坏","clear-disk"),("磁盘/系统破坏","initialize-disk"),("磁盘/系统破坏","remove-partition"),("磁盘/系统破坏","shred"),
    ("注册表写入","reg add"),("注册表写入","reg.exe add"),("注册表写入","reg delete"),("注册表写入","reg import"),("注册表写入","set-itemproperty hklm"),("注册表写入","set-itemproperty hkcu"),("注册表写入","new-itemproperty hklm"),("注册表写入","new-itemproperty hkcu"),("注册表写入","currentversion\\run"),("注册表写入","hkey_local_machine"),("注册表写入","hkey_current_user"),
    ("提权/绕过","runas"),("提权/绕过","start-process -verb runas"),("提权/绕过","bypass"),("提权/绕过","set-executionpolicy"),("提权/绕过","executionpolicy"),("提权/绕过","encodedcommand"),("提权/绕过","sudo"),("提权/绕过","invoke-command"),
    ("凭据/密钥",".env"),("凭据/密钥","id_rsa"),("凭据/密钥","id_dsa"),("凭据/密钥","known_hosts"),("凭据/密钥","credentials"),("凭据/密钥","credential"),("凭据/密钥","config.json"),("凭据/密钥","api_key"),("凭据/密钥","api-key"),("凭据/密钥","password"),("凭据/密钥","private_key"),("凭据/密钥","aws_access_key"),("凭据/密钥","kubeconfig"),
    ("持久化","schtasks"),("持久化","new-scheduledtask"),("持久化","register-scheduledtask"),("持久化","startup"),("持久化","shell:startup"),("持久化","autorun"),("持久化","crontab"),("持久化","launchagents"),("持久化","launchd"),("持久化","systemd"),("持久化","bashrc"),("持久化","powershell_profile"),
    ("网络外发","curl"),("网络外发","wget"),("网络外发","invoke-webrequest"),("网络外发","invoke-restmethod"),("网络外发","requests."),("网络外发","urllib"),("网络外发","urllib3"),("网络外发","socket"),("网络外发","http://"),("网络外发","https://"),("网络外发","http.client"),("网络外发","require('http')"),("网络外发","require(\"http\")"),("网络外发","require('https')"),("网络外发","require(\"https\")"),("网络外发","require('net')"),("网络外发","require(\"net\")"),("网络外发","webclient"),("网络外发","websocket"),("网络外发","axios"),("网络外发","ftp"),("网络外发","bitsadmin"),("网络外发","fetch("),
    ("脚本绕过","subprocess"),("脚本绕过","child_process"),("脚本绕过","os.system"),("脚本绕过","os.popen"),("脚本绕过","os.exec"),("脚本绕过","os.spawn"),("脚本绕过","invoke-expression"),("脚本绕过","iex "),("脚本绕过","cmd.exe"),("脚本绕过","powershell.exe"),("脚本绕过","mshta"),("脚本绕过","rundll32"),("脚本绕过","regsvr32"),("脚本绕过","process.binding"),("脚本绕过","process.dlopen"),("脚本绕过","ctypes"),("脚本绕过","importlib"),("脚本绕过","eval("),("脚本绕过","exec("),
    ("沙箱外写入","new-item"),("沙箱外写入","remove-item"),("沙箱外写入","os.mkdir"),("沙箱外写入","os.makedirs"),("沙箱外写入","os.remove"),("沙箱外写入","os.unlink"),("沙箱外写入","write_text"),("沙箱外写入","write_bytes"),("沙箱外写入","writefile"),("沙箱外写入","write_file"),("沙箱外写入","appendfile"),("沙箱外写入","set-content"),("沙箱外写入","out-file"),("沙箱外写入","add-content"),("沙箱外写入","copyfile"),("沙箱外写入","movefile"),("沙箱外写入","rename"),("沙箱外写入","unlink"),("沙箱外写入","fs.rm"),("沙箱外写入","fs.mkdir"),("沙箱外写入","fs.rename"),("沙箱外写入","fs.write"),("沙箱外写入","fs.open"),("沙箱外写入","createwritestream"),("沙箱外写入","tempfile"),("沙箱外写入","namedtemporaryfile"),("沙箱外写入","temporarydirectory"),("沙箱外写入","writealltext"),("沙箱外写入","writeallbytes"),("沙箱外写入","createfile"),("沙箱外写入","truncate"),("沙箱外写入","chmod"),("沙箱外写入","chown"),("沙箱外写入","remove("),
];
fn validate_code(code: &str) -> Result<(), AppError> {
    let lower = code.to_lowercase(); let compact: String = lower.chars().filter(|ch| !ch.is_whitespace() && !matches!(*ch, '\'' | '"' | '\u{60}')).collect();
    if lower.contains("open(") && ["\"w\"", "\"a\"", "\"x\"", "\"+\"", "\"wb\"", "\"ab\"", "\"xb\"", "\"r+\"", "\'w\'", "\'a\'", "\'x\'", "\'+\'", "\'wb\'", "\'ab\'", "\'xb\'", "\'r+\'"] .iter().any(|mode| lower.contains(mode)) { log_event("reject", "未注册", "自制工具不得以写入模式打开文件"); return Err(AppError::ToolboxError("自制工具不得以写入模式打开文件，已拒绝执行".into())); }
    for (category, pattern) in DENY_PATTERNS { let pattern_lower = pattern.to_lowercase(); let pattern_compact: String = pattern_lower.chars().filter(|ch| !ch.is_whitespace() && !matches!(*ch, '\'' | '"' | '\u{60}')).collect(); if lower.contains(&pattern_lower) || compact.contains(&pattern_compact) { let reason = match *category { "网络外发" => "自制工具默认禁止联网，已拒绝执行", "凭据/密钥" => "自制工具不得读取凭据、密钥或配置文件，已拒绝执行", "沙箱外写入" => "自制工具不得写入沙箱外文件，已拒绝执行", _ => "脚本包含被禁止的危险操作，已拒绝执行" }; log_event("reject", "未注册", reason); return Err(AppError::ToolboxError(format!("{reason}（命中：{category}规则）"))); } }
    Ok(())
}
fn sandbox_root(id: &str) -> PathBuf { std::env::temp_dir().join("memoria_forge").join(id) }
fn execute_script(tool: &ForgedTool, args_json: &str, timeout: Duration) -> Result<ProcessOutput, AppError> {
    if uuid::Uuid::parse_str(&tool.id).is_err() {
        return Err(AppError::ToolboxError("自制工具 id 无效，已拒绝执行".into()));
    }
    if args_json.as_bytes().len() > MAX_ARGS_BYTES { return Err(AppError::ToolboxError("自制工具参数超过 64KB 限制".into())); }
    validate_code(&tool.code)?; let language = ScriptLanguage::parse(&tool.language)?; let workdir = sandbox_root(&tool.id);
    std::fs::create_dir_all(&workdir).map_err(|e| AppError::ToolboxError(format!("沙箱目录创建失败：{e}")))?;
    let extension = match language { ScriptLanguage::Python => "py", ScriptLanguage::PowerShell => "ps1", ScriptLanguage::Node => "js" }; let script_path = workdir.join(format!("tool.{extension}"));
    log_event("execute", &tool.id, "开始执行自制工具");
    let result = (|| { std::fs::write(&script_path, tool.code.as_bytes()).map_err(|e| AppError::ToolboxError(format!("沙箱脚本写入失败：{e}")))?; let mut command = match language { ScriptLanguage::Python => { let mut c=Command::new("python"); c.arg(&script_path).arg(args_json); c }, ScriptLanguage::PowerShell => { let mut c=Command::new("powershell"); c.args(["-NoProfile","-NonInteractive","-File"]).arg(&script_path).arg(args_json); c }, ScriptLanguage::Node => { let mut c=Command::new("node"); c.arg(&script_path).arg(args_json); c } }; configure_environment(&mut command); command.current_dir(&workdir).stdout(Stdio::piped()).stderr(Stdio::piped()); run_process(command, timeout) })();
    let _ = std::fs::remove_dir_all(&workdir);
    match &result { Ok(_) => log_event("execute", &tool.id, "自制工具执行成功"), Err(error) => log_event("execute-error", &tool.id, &safe_error(error)) }
    result
}
fn configure_environment(command: &mut Command) { let path=std::env::var_os("PATH"); let root=std::env::var_os("SystemRoot"); let temp=std::env::temp_dir(); command.env_clear(); if let Some(path)=path { command.env("PATH",path); } if let Some(root)=root { command.env("SystemRoot",&root); command.env("WINDIR",&root); } command.env("TEMP",&temp).env("TMP",&temp); }
fn run_process(mut command: Command, timeout: Duration) -> Result<ProcessOutput, AppError> {
    let mut child=command.spawn().map_err(|e| AppError::ToolboxError(format!("沙箱解释器启动失败：{e}")))?; let stdout=child.stdout.take().map(|stream| thread::spawn(move||read_capped(stream))); let stderr=child.stderr.take().map(|stream| thread::spawn(move||read_capped(stream))); let deadline=Instant::now()+timeout;
    let status=loop { match child.try_wait() { Ok(Some(status))=>break status, Ok(None) if Instant::now()>=deadline=>{let _=child.kill();let _=child.wait();join_reader(stdout);join_reader(stderr);log_event("timeout","未注册","沙箱执行超时并已终止进程");return Err(AppError::ToolboxTimeout("自制工具执行超过 30 秒，已终止".into()));}, Ok(None)=>thread::sleep(Duration::from_millis(10)), Err(error)=>{let _=child.kill();let _=child.wait();join_reader(stdout);join_reader(stderr);return Err(AppError::ToolboxError(format!("沙箱进程状态读取失败：{error}")));} } };
    let output=ProcessOutput{stdout:join_reader(stdout).unwrap_or_default(),stderr:join_reader(stderr).unwrap_or_default()}; if status.success(){Ok(output)}else{let detail=if output.stderr.trim().is_empty(){"脚本返回失败状态"}else{&output.stderr};Err(AppError::ToolboxError(format!("自制工具试跑失败：{}",safe_error_text(detail))))}
}
fn read_capped<R: Read>(mut reader:R)->String { let mut kept=Vec::with_capacity(MAX_OUTPUT_BYTES);let mut buffer=[0u8;4096];let mut truncated=false;loop{match reader.read(&mut buffer){Ok(0)=>break,Ok(size)=>{if kept.len()<MAX_OUTPUT_BYTES{let take=(MAX_OUTPUT_BYTES-kept.len()).min(size);kept.extend_from_slice(&buffer[..take]);if take<size{truncated=true}}else{truncated=true}},Err(error) if error.kind()==ErrorKind::Interrupted=>continue,Err(_)=>break}}let mut text=String::from_utf8_lossy(&kept).into_owned();if truncated{text.push_str("\n（输出已截断）")}redact_text(&text)}
fn join_reader(reader:Option<thread::JoinHandle<String>>)->Option<String>{reader.and_then(|handle|handle.join().ok())}
fn combine_output(output:&ProcessOutput)->String{match(output.stdout.trim(),output.stderr.trim()){(stdout,"")=>stdout.to_string(),("",stderr)=>format!("stderr：{stderr}"),(stdout,stderr)=>format!("stdout：{stdout}\nstderr：{stderr}")}}
fn safe_error(error:&AppError)->String{redact_text(&error.to_string())}
fn safe_error_text(text:&str)->String{redact_text(text)}
fn redact_text(text:&str)->String{let cfg=crate::config::store::get_config();let mut result=cfg.redact_api_secrets(text);for(key,value)in std::env::vars(){let sensitive=key.ends_with("_API_KEY")||key.ends_with("_TOKEN")||key.ends_with("_SECRET");if sensitive&&!value.is_empty(){result=result.replace(&value,"[已隐藏]")}}result.lines().map(|line|{let lower=line.to_lowercase();if["api_key","api-key","token","secret","password"].iter().any(|key|lower.contains(key)){if let Some(index)=line.find('='){return format!("{}=[已隐藏]",&line[..index])}}line.to_string()}).collect::<Vec<_>>().join("\n")}
fn log_event(action:&str,id:&str,detail:&str){log::info!("[forge] action={} tool={} detail={}",action,id,safe_log_text(detail));}
fn safe_log_text(text:&str)->String{redact_text(text).chars().take(300).collect()}

#[cfg(test)]
mod tests { use super::*;
    fn args(code:&str,language:&str)->HashMap<String,Value>{HashMap::from([("name".into(),json!("count_lines")),("description".into(),json!("测试工具")),("language".into(),json!(language)),("code".into(),json!(code)),("params".into(),json!({"type":"object"})),("test_args".into(),json!({}))])}
    #[test] fn 名称和语言严格校验(){assert!(parse_forged_tool(&args("print('ok')","python")).is_ok());let mut invalid=args("print('ok')","python");invalid.insert("name".into(),json!("Bad-Name"));assert!(parse_forged_tool(&invalid).is_err());assert!(parse_forged_tool(&args("print('ok')","ruby")).is_err());}
    #[test] fn 黑名单覆盖危险和网络关键字(){for code in["shutdown /s","format C:","rm -rf /","reg add HKLM\\x","Set-ItemProperty HKLM:\\x","requests.get('https://x')"]{assert!(validate_code(code).is_err(),"应拒绝：{code}");}}
    #[test] fn 输出各自限制到八千字节并标记截断(){let output=read_capped(std::io::Cursor::new(vec![b'x';MAX_OUTPUT_BYTES+1]));assert!(output.contains("输出已截断"));assert!(output.len()<=MAX_OUTPUT_BYTES+30);}
    #[test]
    fn 工具定义_未授权不暴露造工具_授权后暴露() {
        assert!(!tool_definitions(false).iter().any(|tool| tool["function"]["name"] == "forge_tool"));
        assert!(tool_definitions(true).iter().any(|tool| tool["function"]["name"] == "forge_tool"));
        assert!(tool_definitions(false).iter().any(|tool| tool["function"]["name"] == "list_forged_tools"));
    }

    #[test]
    fn 造工具成功失败和重载持久化() {
        let _guard = TEST_LIFECYCLE_MUTEX.lock().unwrap_or_else(|poison| poison.into_inner());
        let path = std::env::temp_dir().join(format!("memoria_forge_test_{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let _ = TEST_STORAGE_PATH.set(path.clone());
        *registry().write().unwrap_or_else(|poison| poison.into_inner()) = Vec::new();
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let name = format!("forge_test_{}", std::process::id());
        let success_args = HashMap::from([
            ("name".into(), json!(name)),
            ("description".into(), json!("测试注册与持久化")),
            ("language".into(), json!("node")),
            ("code".into(), json!("console.log('ok')")),
            ("params".into(), json!({"type":"object"})),
            ("test_args".into(), json!({})),
        ]);
        let result = runtime.block_on(forge_from_args(&success_args, true, "forge-success"));
        assert!(result.is_ok(), "成功试跑应注册：{result:?}");
        assert!(list().unwrap().iter().any(|tool| tool.name == name));
        assert!(path.exists(), "注册后应写入独立 forged_tools.json");
        *registry().write().unwrap_or_else(|poison| poison.into_inner()) = Vec::new();
        reload().expect("reload");
        assert!(list().unwrap().iter().any(|tool| tool.name == name));
        let forged_id = list()
            .unwrap()
            .into_iter()
            .find(|tool| tool.name == name)
            .expect("重载后应能找到自制工具")
            .id;
        set_enabled(&forged_id, false).expect("重复保存应成功");
        assert!(!list()
            .unwrap()
            .into_iter()
            .find(|tool| tool.id == forged_id)
            .expect("停用后工具仍应存在")
            .enabled);
        set_enabled(&forged_id, true).expect("再次保存应成功");
        delete(&forged_id).expect("删除应成功并持久化");
        assert!(!list().unwrap().into_iter().any(|tool| tool.id == forged_id));

        let failed_args = HashMap::from([
            ("name".into(), json!("failed_forge_test")),
            ("description".into(), json!("失败测试")),
            ("language".into(), json!("node")),
            ("code".into(), json!("process.exit(1)")),
            ("params".into(), json!({"type":"object"})),
            ("test_args".into(), json!({})),
        ]);
        let failure = runtime.block_on(forge_from_args(&failed_args, true, "forge-failure"));
        assert!(failure.is_err());
        assert!(!list().unwrap().iter().any(|tool| tool.name == "failed_forge_test"));
        let _ = std::fs::remove_file(path);
    }

    #[test] fn 超时使用注入时限并终止进程(){let mut command=if cfg!(windows){let mut c=Command::new("powershell");c.args(["-NoProfile","-NonInteractive","-Command","Start-Sleep -Seconds 2"]);c}else{let mut c=Command::new("sh");c.args(["-c","sleep 2"]);c};configure_environment(&mut command);command.stdout(Stdio::piped()).stderr(Stdio::piped());let started=Instant::now();let result=run_process(command,Duration::from_millis(80));assert!(matches!(result,Err(AppError::ToolboxTimeout(_))));assert!(started.elapsed()<Duration::from_secs(1));}
}
