use crate::error::AppError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::fs::{self, OpenOptions};
use std::future::Future;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub at: i64,
    pub kind: String,
    pub title: String,
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub goal_id: Option<String>,
    pub tools: Vec<ToolCallRecord>,
    pub files_changed: Vec<String>,
    pub result: String,
    pub summary: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub name: String,
    pub intent: Option<String>,
    pub ok: bool,
    pub brief: String,
    pub at: i64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AuditQuery {
    pub keyword: Option<String>,
    pub kind: Option<String>,
    pub result: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
}

#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct AuditConfig {
    pub max_entries: usize,
    pub max_file_bytes: u64,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            max_entries: 2000,
            max_file_bytes: 5 * 1024 * 1024,
        }
    }
}

pub struct AuditStore {
    dir: PathBuf,
    config: AuditConfig,
    lock: Mutex<()>,
}

fn error() -> AppError {
    AppError::LogWriteError("执行记录存储不可用，请检查目录权限或磁盘空间".into())
}

impl AuditStore {
    pub fn new(dir: PathBuf, config: AuditConfig) -> Self {
        Self {
            dir,
            config: AuditConfig {
                max_entries: config.max_entries.max(1),
                max_file_bytes: config.max_file_bytes.max(1),
            },
            lock: Mutex::new(()),
        }
    }

    fn segments(&self) -> Result<Vec<PathBuf>, AppError> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }
        let mut archives = Vec::new();
        for entry in fs::read_dir(&self.dir).map_err(|_| error())? {
            let entry = entry.map_err(|_| error())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(number) = name
                .strip_prefix("audit-")
                .and_then(|name| name.strip_suffix(".jsonl"))
                .and_then(|name| name.parse::<u64>().ok())
            {
                archives.push((number, entry.path()));
            }
        }
        archives.sort_by_key(|(number, _)| *number);
        let mut paths: Vec<_> = archives.into_iter().map(|(_, path)| path).collect();
        let active = self.dir.join("audit.jsonl");
        if active.exists() {
            paths.push(active);
        }
        Ok(paths)
    }

    fn read_tail(&self) -> Result<(Vec<AuditEntry>, Vec<PathBuf>), AppError> {
        let mut retained = VecDeque::new();
        let mut expired = Vec::new();
        let mut remaining = self.config.max_entries;
        for path in self.segments()?.into_iter().rev() {
            if remaining == 0 {
                expired.push(path);
                continue;
            }
            let file = fs::File::open(&path).map_err(|_| error())?;
            let mut segment = VecDeque::new();
            for line in BufReader::new(file).lines() {
                let line = line.map_err(|_| error())?;
                match serde_json::from_str::<AuditEntry>(&line) {
                    Ok(entry) => {
                        segment.push_back(entry);
                        if segment.len() > remaining {
                            segment.pop_front();
                        }
                    }
                    Err(_) => log::warn!("[audit] 已跳过不完整或损坏的执行记录"),
                }
            }
            remaining -= segment.len();
            segment.append(&mut retained);
            retained = segment;
        }
        Ok((retained.into_iter().collect(), expired))
    }

    fn archive(&self, active: &PathBuf) -> Result<(), AppError> {
        let next = self
            .segments()?
            .iter()
            .filter_map(|path| {
                path.file_stem()?
                    .to_str()?
                    .strip_prefix("audit-")?
                    .parse::<u64>()
                    .ok()
            })
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        fs::rename(active, self.dir.join(format!("audit-{next}.jsonl"))).map_err(|_| error())
    }

    pub fn append(&self, entry: &AuditEntry) -> Result<(), AppError> {
        let _lock = self.lock.lock().map_err(|_| error())?;
        fs::create_dir_all(&self.dir).map_err(|_| error())?;
        let active = self.dir.join("audit.jsonl");
        if active.exists() && fs::metadata(&active).map_err(|_| error())?.len() > 0 {
            self.archive(&active)?;
        }
        let mut bytes = serde_json::to_vec(entry).map_err(|_| error())?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&active)
            .map_err(|_| error())?;
        file.write_all(&bytes).map_err(|_| error())?;
        file.flush().map_err(|_| error())?;
        let size = file.metadata().map_err(|_| error())?.len();
        drop(file);
        if size > self.config.max_file_bytes {
            self.archive(&active)?;
        }
        let paths = self.segments()?;
        for path in paths
            .iter()
            .take(paths.len().saturating_sub(self.config.max_entries))
        {
            fs::remove_file(path).map_err(|_| error())?;
        }
        Ok(())
    }

    pub fn record(&self, entry: &AuditEntry) {
        if self.append(entry).is_err() {
            log::warn!("[audit] 执行记录写入失败，主流程继续；请检查目录权限或磁盘空间");
        }
    }

    pub fn list(&self, query: &AuditQuery) -> Result<Vec<AuditEntry>, AppError> {
        let _lock = self.lock.lock().map_err(|_| error())?;
        let (entries, _) = self.read_tail()?;
        let keyword = query.keyword.as_deref().unwrap_or("").trim().to_lowercase();
        Ok(entries
            .into_iter()
            .rev()
            .filter(|entry| {
                query
                    .kind
                    .as_ref()
                    .filter(|value| !value.is_empty())
                    .is_none_or(|kind| kind == &entry.kind)
                    && query
                        .result
                        .as_ref()
                        .filter(|value| !value.is_empty())
                        .is_none_or(|result| result == &entry.result)
                    && query.since.is_none_or(|since| entry.at >= since)
                    && query.until.is_none_or(|until| entry.at <= until)
                    && (keyword.is_empty()
                        || std::iter::once(entry.title.as_str())
                            .chain(std::iter::once(entry.summary.as_str()))
                            .chain(entry.files_changed.iter().map(String::as_str))
                            .chain(
                                entry
                                    .tools
                                    .iter()
                                    .flat_map(|tool| [tool.name.as_str(), tool.brief.as_str()]),
                            )
                            .any(|text| text.to_lowercase().contains(&keyword)))
            })
            .collect())
    }

    pub fn clear(&self) -> Result<(), AppError> {
        let _lock = self.lock.lock().map_err(|_| error())?;
        for path in self.segments()? {
            fs::remove_file(path).map_err(|_| error())?;
        }
        Ok(())
    }
}

pub fn store() -> Arc<AuditStore> {
    static STORE: OnceLock<Arc<AuditStore>> = OnceLock::new();
    STORE
        .get_or_init(|| {
            #[cfg(not(test))]
            let dir = crate::config::data_dir();
            #[cfg(test)]
            let dir =
                std::env::temp_dir().join(format!("mem-audit-tests-{}", uuid::Uuid::new_v4()));
            let config = fs::read(dir.join("audit-config.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or_default();
            Arc::new(AuditStore::new(dir, config))
        })
        .clone()
}

pub fn safe_text(text: &str, limit: usize) -> String {
    let mut text = crate::config::store::get_runtime_config().redact_api_secrets(text);
    for (name, secret) in std::env::vars() {
        if !secret.is_empty()
            && ["_API_KEY", "_TOKEN", "_SECRET", "_PASSWORD"]
                .iter()
                .any(|suffix| name.ends_with(suffix))
        {
            text = text.replace(&secret, "[已隐藏]");
        }
    }
    let lower = text.to_lowercase();
    if [
        "人格",
        "角色设定",
        "system prompt",
        "persona",
        "role setting",
    ]
    .iter()
    .any(|word| lower.contains(word))
    {
        return "[设定内容不记录]".into();
    }
    let sensitive = [
        "api_key",
        "api-key",
        "apikey",
        "password",
        "passwd",
        "secret",
        "token",
        "authorization",
        "bearer",
        "密码",
        "密钥",
        "口令",
        "sk-",
    ];
    let first_line = text.lines().next().unwrap_or_default();
    let lower = first_line.to_lowercase();
    let end = sensitive
        .iter()
        .filter_map(|word| lower.find(word))
        .min()
        .unwrap_or(first_line.len());
    let prefix = first_line.get(..end).unwrap_or_default();
    let mut output: String = prefix
        .split_whitespace()
        .map(|word| {
            if word.len() >= 24
                && word.is_ascii()
                && word.chars().any(|ch| ch.is_ascii_digit())
                && word.chars().any(|ch| ch.is_ascii_alphabetic())
            {
                "[已隐藏]"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if end < first_line.len() {
        output.push_str("[已隐藏]");
    }
    output.chars().take(limit).collect()
}

fn path_arg(args: &HashMap<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| args.get(*key).and_then(Value::as_str))
        .and_then(|path| {
            let safe = safe_text(path, 1024);
            if safe != path
                || path.is_empty()
                || path.contains(['\n', '\r', '=', '?'])
                || path.starts_with("http")
            {
                None
            } else {
                Some(safe)
            }
        })
}

fn tool_args(name: &str, args: &HashMap<String, Value>) -> HashMap<String, Value> {
    if let Some(input) = args.get("input") {
        if let Some(object) = input.as_object() {
            return object.clone().into_iter().collect();
        }
        if let Some(input) = input.as_str() {
            if let Ok(object) = serde_json::from_str::<HashMap<String, Value>>(input) {
                return object;
            }
            if ["delete_file", "mkdir"]
                .iter()
                .any(|suffix| name.ends_with(suffix))
            {
                return HashMap::from([("path".into(), Value::String(input.trim().into()))]);
            }
        }
    }
    args.clone()
}

fn call_secrets(value: &Value, secrets: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                let key = key.to_lowercase();
                if [
                    "api_key",
                    "apikey",
                    "api-key",
                    "password",
                    "passwd",
                    "token",
                    "secret",
                    "authorization",
                    "密码",
                    "密钥",
                ]
                .iter()
                .any(|name| key.contains(name))
                {
                    if let Some(secret) = value.as_str().filter(|value| !value.is_empty()) {
                        secrets.push(secret.into());
                    }
                }
                call_secrets(value, secrets);
            }
        }
        Value::Array(values) => {
            for value in values {
                call_secrets(value, secrets);
            }
        }
        _ => {}
    }
}

pub fn changed_files(name: &str, args: &HashMap<String, Value>) -> Vec<String> {
    let args = tool_args(name, args);
    let args = &args;
    let name = name.strip_prefix("toolbox_").unwrap_or(name);
    let name = name.strip_prefix("agent_").unwrap_or(name);
    let mut paths = Vec::new();
    let keys: &[&str] = match name {
        "file_write" | "write_file" | "file_delete" | "delete_file" | "mkdir" | "download"
        | "sheet_write" => &["target", "path", "file", "output"],
        "file_move" | "move_file" | "file_copy" | "copy_file" => {
            &["target", "destination", "dest", "to"]
        }
        "zip" | "compress" => &["target", "output", "destination"],
        "unzip" | "extract" => &["target", "destination", "output", "dest"],
        _ => return paths,
    };
    if let Some(path) = path_arg(args, keys) {
        paths.push(path);
    }
    if matches!(name, "file_move" | "move_file") {
        if let Some(path) = path_arg(args, &["source", "src", "from", "path"]) {
            paths.push(path);
        }
    }
    if paths.is_empty() && matches!(name, "zip" | "compress") {
        if let Some(path) = path_arg(args, &["source"]) {
            paths.push(
                PathBuf::from(path)
                    .with_extension("zip")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    if paths.is_empty() && matches!(name, "unzip" | "extract") {
        if let Some(path) = path_arg(args, &["archive"]) {
            let path = PathBuf::from(path);
            paths.push(path.with_extension("").to_string_lossy().into_owned());
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

tokio::task_local! { static CURRENT: Arc<Mutex<AuditEntry>>; }

pub struct AuditRun {
    entry: Arc<Mutex<AuditEntry>>,
    store: Arc<AuditStore>,
    started: Instant,
}

impl AuditRun {
    pub fn new(
        kind: &str,
        title: &str,
        request_id: Option<&str>,
        session_id: Option<&str>,
        goal_id: Option<&str>,
    ) -> Self {
        Self::with_store(kind, title, request_id, session_id, goal_id, store())
    }

    pub(crate) fn with_store(
        kind: &str,
        title: &str,
        request_id: Option<&str>,
        session_id: Option<&str>,
        goal_id: Option<&str>,
        store: Arc<AuditStore>,
    ) -> Self {
        Self {
            entry: Arc::new(Mutex::new(AuditEntry {
                id: uuid::Uuid::new_v4().to_string(),
                at: chrono::Utc::now().timestamp(),
                kind: kind.into(),
                title: safe_text(title, 60),
                request_id: request_id.map(|value| safe_text(value, 120)),
                session_id: session_id.map(|value| safe_text(value, 120)),
                goal_id: goal_id.map(|value| safe_text(value, 120)),
                tools: Vec::new(),
                files_changed: Vec::new(),
                result: "cancelled".into(),
                summary: String::new(),
                duration_ms: 0,
            })),
            store,
            started: Instant::now(),
        }
    }

    pub async fn scope<Output>(&self, work: impl Future<Output = Output>) -> Output {
        CURRENT.scope(self.entry.clone(), work).await
    }

    pub fn finish(&self, result: &str) {
        let mut entry = self
            .entry
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let failed = entry.tools.iter().filter(|tool| !tool.ok).count();
        entry.result = if result == "ok" && failed > 0 {
            "partial"
        } else {
            result
        }
        .into();
    }

    pub fn goal_progress(&self, completed_before: usize, completed_after: usize, total: usize) {
        let mut entry = self
            .entry
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        entry.summary = format!(
            "本轮新增完成 {} 个步骤，累计完成 {}/{} 个步骤",
            completed_after.saturating_sub(completed_before),
            completed_after,
            total
        );
    }
}

impl Drop for AuditRun {
    fn drop(&mut self) {
        let mut entry = self
            .entry
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        entry.duration_ms = self.started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let label = match entry.result.as_str() {
            "ok" => "成功",
            "partial" => "部分完成",
            "blocked" => "卡住",
            "cancelled" => "已取消",
            _ => "失败",
        };
        entry.summary = if entry.kind == "forge" {
            format!(
                "工具{}；{}",
                if entry.result == "ok" {
                    "注册成功"
                } else {
                    "未注册"
                },
                label
            )
        } else {
            let progress = if entry.summary.is_empty() {
                String::new()
            } else {
                format!("{}；", entry.summary)
            };
            format!(
                "{}本轮{}；调用 {} 次工具，改动 {} 个文件路径",
                progress,
                label,
                entry.tools.len(),
                entry.files_changed.len()
            )
        };
        self.store.record(&entry);
    }
}

pub struct ToolAttempt {
    entry: Option<Arc<Mutex<AuditEntry>>>,
    index: usize,
    record: ToolCallRecord,
    files: Vec<String>,
}

impl ToolAttempt {
    pub fn new(name: &str, args: &HashMap<String, Value>, tools: &[Value]) -> Self {
        let intent = tools
            .iter()
            .find(|tool| tool["function"]["name"].as_str() == Some(name))
            .and_then(|tool| tool["x-intent"].as_str())
            .map(|value| safe_text(value, 80));
        let mut files = changed_files(name, args);
        let normalized = tool_args(name, args);
        let mut secrets = Vec::new();
        call_secrets(
            &Value::Object(args.clone().into_iter().collect()),
            &mut secrets,
        );
        call_secrets(
            &Value::Object(normalized.clone().into_iter().collect()),
            &mut secrets,
        );
        let args = normalized;
        let brief = if let Some(path) = files.first() {
            format!("文件操作路径：{path}")
        } else if name.contains("read_file") {
            path_arg(&args, &["path", "target"])
                .map(|path| format!("读取文件：{path}"))
                .unwrap_or_else(|| "读取文件（不记录内容）".into())
        } else {
            "调用工具（不记录参数、命令或输出正文）".into()
        };
        let mut record = ToolCallRecord {
            name: safe_text(name, 100),
            intent,
            ok: false,
            brief: safe_text(&brief, 100),
            at: chrono::Utc::now().timestamp(),
        };
        let entry = CURRENT.try_with(Clone::clone).ok();
        files.retain(|path| !secrets.iter().any(|secret| path.contains(secret)));
        for secret in &secrets {
            record.name = record.name.replace(secret, "[已隐藏]");
            record.brief = record.brief.replace(secret, "[已隐藏]");
        }
        let index = if let Some(entry) = &entry {
            let mut entry = entry.lock().unwrap_or_else(|poison| poison.into_inner());
            let entry = &mut *entry;
            for secret in &secrets {
                entry.title = entry.title.replace(secret, "[已隐藏]");
                for value in [
                    &mut entry.request_id,
                    &mut entry.session_id,
                    &mut entry.goal_id,
                ]
                .into_iter()
                .flatten()
                {
                    *value = value.replace(secret, "[已隐藏]");
                }
            }
            entry.title = entry.title.chars().take(60).collect();
            record.name = record.name.chars().take(100).collect();
            record.brief = record.brief.chars().take(100).collect();
            let index = entry.tools.len();
            entry.tools.push(record.clone());
            index
        } else {
            0
        };
        Self {
            entry,
            index,
            record,
            files,
        }
    }

    pub fn finish(&mut self, ok: bool) {
        self.record.ok = ok;
    }
}

impl Drop for ToolAttempt {
    fn drop(&mut self) {
        if let Some(entry) = &self.entry {
            let mut entry = entry.lock().unwrap_or_else(|poison| poison.into_inner());
            entry.tools[self.index] = self.record.clone();
            if self.record.ok {
                for path in &self.files {
                    if !entry.files_changed.contains(path) {
                        entry.files_changed.push(path.clone());
                    }
                }
            }
        }
    }
}

pub fn agent_result(
    result: &Result<crate::commands::agent_run::AgentRunResponse, AppError>,
) -> &'static str {
    match result {
        Ok(response) if response.interrupted => "cancelled",
        Ok(response) if response.success => "ok",
        Err(AppError::PermissionDenied(_)) => "blocked",
        _ => "failed",
    }
}

#[cfg(test)]
mod tests;
