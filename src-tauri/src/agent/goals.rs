use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::sync::Notify;

use super::loop_::{run_one_task, TaskRuntime};
use super::stream_progress::StreamEmitter;
use super::tools::{build_tools, AgentPermissions};
use crate::error::AppError;
use crate::types::{AgentGoal, Checkpoint, GoalBudget, GoalStep, GoalUsage};

static STORE: OnceLock<GoalStore> = OnceLock::new();
static CHAT_BUSY: AtomicUsize = AtomicUsize::new(0);
static SCHEDULE_CHANGED: Notify = Notify::const_new();
const STARTED: &str = "推进中；若应用退出，本次不会自动续跑";

pub struct ChatBusyGuard;
impl ChatBusyGuard {
    pub fn new() -> Self {
        CHAT_BUSY.fetch_add(1, Ordering::SeqCst);
        Self
    }
}
impl Drop for ChatBusyGuard {
    fn drop(&mut self) {
        CHAT_BUSY.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct GoalState {
    goals: Vec<AgentGoal>,
    running: Option<(String, Arc<Notify>, bool)>,
}

pub struct GoalStore {
    path: PathBuf,
    state: Mutex<GoalState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalReport {
    pub goal: AgentGoal,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GoalSnapshot {
    pub goals: Vec<AgentGoal>,
    pub running_id: Option<String>,
    pub automatic: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoalUpdate {
    pub progress: String,
    pub next_action: Option<String>,
    pub steps: Vec<GoalStep>,
    pub outcome: String,
    #[serde(default)]
    pub done: bool,
    pub blocked_reason: Option<String>,
}

fn error(text: impl Into<String>) -> AppError {
    AppError::InternalError(text.into())
}
fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn redact(text: &str) -> String {
    let mut result = crate::config::store::get_config().redact_api_secrets(text);
    for (name, secret) in std::env::vars() {
        if !secret.is_empty()
            && ["_API_KEY", "_TOKEN", "_SECRET", "_PASSWORD"]
                .iter()
                .any(|suffix| name.ends_with(suffix))
        {
            result = result.replace(&secret, "[已隐藏]");
        }
    }
    result
        .lines()
        .map(|line| {
            let lower = line.to_lowercase();
            if [
                "api_key",
                "api-key",
                "token",
                "secret",
                "password",
                "authorization",
                "密钥",
                "密码",
            ]
            .iter()
            .any(|word| lower.contains(word))
            {
                if let Some(index) = line.find(['=', ':', '：']) {
                    return format!("{}：[已隐藏]", &line[..index]);
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn sanitize(goal: &mut AgentGoal) {
    goal.title = redact(&goal.title);
    goal.description = redact(&goal.description);
    goal.progress = redact(&goal.progress);
    goal.next_action = goal.next_action.as_deref().map(redact);
    goal.blocked_reason = goal.blocked_reason.as_deref().map(redact);
    for step in &mut goal.steps {
        step.text = redact(&step.text);
    }
    for checkpoint in &mut goal.checkpoints {
        checkpoint.summary = redact(&checkpoint.summary);
    }
}

fn validate_steps(steps: &[GoalStep]) -> Result<(), AppError> {
    if steps.iter().any(|step| {
        step.text.trim().is_empty()
            || !["pending", "doing", "done", "failed"].contains(&step.status.as_str())
    }) {
        return Err(error(
            "目标步骤必须有内容，状态只能是 pending/doing/done/failed",
        ));
    }
    Ok(())
}

impl GoalStore {
    pub fn open(path: PathBuf) -> Result<Self, AppError> {
        let mut goals: Vec<AgentGoal> = if path.exists() {
            serde_json::from_slice(&std::fs::read(&path)?)?
        } else {
            Vec::new()
        };
        for goal in &mut goals {
            if !["active", "paused", "done", "blocked", "cancelled"].contains(&goal.status.as_str())
                || goal.auto_interval_secs == 0
                || goal.budget.max_runs == 0
                || goal.budget.max_seconds_per_run == 0
            {
                return Err(error("目标文件包含无效状态或预算，请检查 goals.json"));
            }
            validate_steps(&goal.steps)?;
            sanitize(goal);
            if goal
                .checkpoints
                .last()
                .is_some_and(|checkpoint| checkpoint.summary == STARTED)
            {
                goal.status = "paused".into();
                goal.auto_advance = false;
                goal.checkpoints.last_mut().unwrap().summary =
                    "上次推进被应用退出中断，已暂停，请手动继续".into();
            }
        }
        let store = Self {
            path,
            state: Mutex::new(GoalState {
                goals,
                running: None,
            }),
        };
        let state = store.state.lock().map_err(|_| error("目标存储锁不可用"))?;
        if store.path.exists() {
            store.save(&state.goals)?;
        }
        drop(state);
        Ok(store)
    }

    fn save(&self, goals: &[AgentGoal]) -> Result<(), AppError> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| error("目标存储路径无效"))?;
        std::fs::create_dir_all(parent)?;
        let temporary = self.path.with_extension("json.tmp");
        let mut file = std::fs::File::create(&temporary)?;
        use std::io::Write;
        file.write_all(&serde_json::to_vec_pretty(goals)?)?;
        file.sync_all()?;
        std::fs::rename(&temporary, &self.path)?;
        Ok(())
    }

    pub fn snapshot(&self) -> Result<GoalSnapshot, AppError> {
        let state = self.state.lock().map_err(|_| error("目标存储锁不可用"))?;
        Ok(GoalSnapshot {
            goals: state.goals.clone(),
            running_id: state.running.as_ref().map(|running| running.0.clone()),
            automatic: state.running.as_ref().is_some_and(|running| running.2),
        })
    }

    pub fn get(&self, id: &str) -> Result<AgentGoal, AppError> {
        self.snapshot()?
            .goals
            .into_iter()
            .find(|goal| goal.id == id)
            .ok_or_else(|| error("目标不存在"))
    }

    fn edit(
        &self,
        id: &str,
        action: impl FnOnce(&mut AgentGoal) -> Result<(), AppError>,
    ) -> Result<AgentGoal, AppError> {
        let mut state = self.state.lock().map_err(|_| error("目标存储锁不可用"))?;
        let mut goals = state.goals.clone();
        let goal = goals
            .iter_mut()
            .find(|goal| goal.id == id)
            .ok_or_else(|| error("目标不存在"))?;
        action(goal)?;
        goal.updated_at = now();
        sanitize(goal);
        let result = goal.clone();
        self.save(&goals)?;
        state.goals = goals;
        Ok(result)
    }

    pub fn create(
        &self,
        title: String,
        description: String,
        steps: Vec<GoalStep>,
        budget: GoalBudget,
    ) -> Result<AgentGoal, AppError> {
        if title.trim().is_empty() || description.trim().is_empty() {
            return Err(error("请填写目标标题和详细说明"));
        }
        if budget.max_runs == 0 || budget.max_seconds_per_run == 0 {
            return Err(error("推进次数和单次秒数上限必须大于零"));
        }
        validate_steps(&steps)?;
        let mut goal = AgentGoal {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            description,
            steps,
            budget,
            status: "active".into(),
            progress: "尚未推进".into(),
            next_action: Some("拆解目标并开始第一步".into()),
            blocked_reason: None,
            used: GoalUsage::default(),
            checkpoints: Vec::new(),
            created_at: now(),
            updated_at: now(),
            auto_advance: false,
            auto_interval_secs: 300,
            auto_runs: 0,
            no_change_runs: 0,
            last_auto_at: now(),
        };
        sanitize(&mut goal);
        let mut state = self.state.lock().map_err(|_| error("目标存储锁不可用"))?;
        let mut goals = state.goals.clone();
        goals.push(goal.clone());
        self.save(&goals)?;
        state.goals = goals;
        Ok(goal)
    }

    pub fn set_status(&self, id: &str, status: &str) -> Result<AgentGoal, AppError> {
        if !["active", "paused", "cancelled"].contains(&status) {
            return Err(error("只能继续、暂停或取消目标"));
        }
        let result = self.edit(id, |goal| {
            if ["done", "cancelled"].contains(&goal.status.as_str()) {
                return Err(error("已完成或已取消的目标不能继续修改状态"));
            }
            if status == "active" && goal.used.runs >= goal.budget.max_runs {
                return Err(error("已达到设定的推进次数上限，请先调整预算"));
            }
            goal.status = status.into();
            goal.blocked_reason = None;
            if status == "active" {
                goal.no_change_runs = 0;
            } else {
                goal.auto_advance = false;
            }
            Ok(())
        })?;
        if status != "active" {
            let state = self.state.lock().map_err(|_| error("目标存储锁不可用"))?;
            if let Some((running_id, stop, _)) = &state.running {
                if running_id == id {
                    super::cancel::set_cancelled(&format!("goal_{id}_{}", result.used.runs));
                    stop.notify_one();
                }
            }
        }
        Ok(result)
    }

    pub fn settings(
        &self,
        id: &str,
        auto_advance: bool,
        interval: u32,
        budget: GoalBudget,
    ) -> Result<AgentGoal, AppError> {
        if interval == 0 || budget.max_runs == 0 || budget.max_seconds_per_run == 0 {
            return Err(error("间隔和预算必须大于零"));
        }
        self.edit(id, |goal| {
            if auto_advance && (goal.status != "active" || goal.used.runs >= budget.max_runs) {
                return Err(error("仅未达预算上限的进行中目标可以开启自动推进"));
            }
            if auto_advance && !goal.auto_advance {
                goal.last_auto_at = now();
            }
            goal.auto_advance = auto_advance;
            goal.auto_interval_secs = interval;
            goal.budget = budget;
            Ok(())
        })
    }

    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        let mut state = self.state.lock().map_err(|_| error("目标存储锁不可用"))?;
        if state
            .running
            .as_ref()
            .is_some_and(|running| running.0 == id)
        {
            return Err(error("请先暂停或取消正在推进的目标，再删除"));
        }
        let mut goals = state.goals.clone();
        let index = goals
            .iter()
            .position(|goal| goal.id == id)
            .ok_or_else(|| error("目标不存在"))?;
        goals.remove(index);
        self.save(&goals)?;
        state.goals = goals;
        Ok(())
    }

    fn due(&self, at: i64, busy: bool) -> Result<Option<String>, AppError> {
        let state = self.state.lock().map_err(|_| error("目标存储锁不可用"))?;
        if busy || state.running.is_some() {
            return Ok(None);
        }
        Ok(state
            .goals
            .iter()
            .filter(|goal| {
                goal.status == "active"
                    && goal.auto_advance
                    && at.saturating_sub(goal.last_auto_at) >= i64::from(goal.auto_interval_secs)
            })
            .min_by_key(|goal| (goal.last_auto_at, goal.created_at))
            .map(|goal| goal.id.clone()))
    }

    fn next_delay(&self, at: i64) -> Result<Duration, AppError> {
        let state = self.state.lock().map_err(|_| error("目标存储锁不可用"))?;
        if state.running.is_some() {
            return Ok(Duration::from_secs(300));
        }
        let seconds = state
            .goals
            .iter()
            .filter(|goal| goal.status == "active" && goal.auto_advance)
            .map(|goal| {
                goal.last_auto_at
                    .saturating_add(i64::from(goal.auto_interval_secs))
                    .saturating_sub(at)
                    .clamp(1, 300)
            })
            .min()
            .unwrap_or(300);
        Ok(Duration::from_secs(seconds as u64))
    }

    fn defer_busy_tick(&self, at: i64) -> Result<(), AppError> {
        let mut state = self.state.lock().map_err(|_| error("目标存储锁不可用"))?;
        let mut goals = state.goals.clone();
        let mut changed = false;
        for goal in &mut goals {
            if goal.status == "active"
                && goal.auto_advance
                && at.saturating_sub(goal.last_auto_at) >= i64::from(goal.auto_interval_secs)
            {
                goal.last_auto_at = at;
                changed = true;
            }
        }
        if changed {
            self.save(&goals)?;
            state.goals = goals;
        }
        Ok(())
    }

    pub async fn advance<Runner, Work, Observer>(
        &self,
        id: &str,
        automatic: bool,
        at: i64,
        timeout: Option<Duration>,
        runner: Runner,
        observer: Observer,
    ) -> Result<GoalReport, AppError>
    where
        Runner: FnOnce(AgentGoal) -> Work,
        Work: Future<Output = Result<GoalUpdate, AppError>>,
        Observer: Fn(),
    {
        let (before, stop, checkpoint_index, _lease) = {
            let mut state = self.state.lock().map_err(|_| error("目标存储锁不可用"))?;
            if state.running.is_some() {
                return Err(error("已有目标正在推进，请稍后再试"));
            }
            if automatic && CHAT_BUSY.load(Ordering::SeqCst) > 0 {
                return Err(error("用户正在对话，本轮自动推进已跳过"));
            }
            let mut goals = state.goals.clone();
            let goal = goals
                .iter_mut()
                .find(|goal| goal.id == id)
                .ok_or_else(|| error("目标不存在"))?;
            if goal.status != "active" {
                return Err(error("目标不在进行中，请先手动继续"));
            }
            if automatic
                && (!goal.auto_advance
                    || at.saturating_sub(goal.last_auto_at) < i64::from(goal.auto_interval_secs))
            {
                return Err(error("尚未到自动推进时间，或自动推进已关闭"));
            }
            if goal.used.runs >= goal.budget.max_runs {
                goal.status = "paused".into();
                goal.auto_advance = false;
                goal.updated_at = now();
                goal.checkpoints.push(Checkpoint {
                    at: now(),
                    summary: "已达到设定的推进次数上限".into(),
                    outcome: "partial".into(),
                });
                let report = GoalReport {
                    goal: goal.clone(),
                    message: "已达到设定的推进次数上限".into(),
                };
                self.save(&goals)?;
                state.goals = goals;
                drop(state);
                observer();
                return Ok(report);
            }
            let before = goal.clone();
            goal.used.runs = goal.used.runs.saturating_add(1);
            if automatic {
                goal.auto_runs = goal.auto_runs.saturating_add(1);
                goal.last_auto_at = at;
            }
            let checkpoint_index = goal.checkpoints.len();
            goal.checkpoints.push(Checkpoint {
                at: now(),
                summary: STARTED.into(),
                outcome: "partial".into(),
            });
            goal.updated_at = now();
            self.save(&goals)?;
            state.goals = goals;
            let stop = Arc::new(Notify::new());
            state.running = Some((id.into(), stop.clone(), automatic));
            (before, stop, checkpoint_index, RunLease(self))
        };
        observer();
        let started = Instant::now();
        let limit = timeout.unwrap_or(Duration::from_secs(u64::from(
            before.budget.max_seconds_per_run,
        )));
        let result = tokio::select! {
            biased;
            _ = stop.notified() => RunResult::Stopped,
            result = tokio::time::timeout(limit, runner(before.clone())) => match result {
                Ok(Ok(update)) if validate_update(&update).is_ok() => RunResult::Updated(update),
                Ok(Ok(_)) => RunResult::Failed,
                Ok(Err(_)) => RunResult::Failed,
                Err(_) => RunResult::Timeout,
            },
        };
        let result = if started.elapsed() >= limit && !matches!(result, RunResult::Stopped) {
            RunResult::Timeout
        } else {
            result
        };
        let elapsed = started.elapsed().as_secs().min(u64::from(u32::MAX)) as u32;
        let mut message = String::new();
        let goal = self.edit(id, |goal| {
            goal.used.total_seconds = goal.used.total_seconds.saturating_add(elapsed);
            let mut outcome = "partial".to_string();
            if goal.status == "cancelled" {
                message = "用户已取消目标，停止推进".into();
            } else if goal.status == "paused" {
                message = "用户已暂停目标，停止推进".into();
            } else {
                match result {
                    RunResult::Stopped => {
                        goal.status = "paused".into();
                        message = "用户已停止推进".into();
                    }
                    RunResult::Timeout => {
                        goal.status = "paused".into();
                        message = "单次推进运行超时，已暂停；请检查进展后手动继续".into();
                    }
                    RunResult::Failed => {
                        goal.status = "paused".into();
                        outcome = "failed".into();
                        message = "目标推进失败或返回格式无效，已暂停，请检查模型配置后重试".into();
                    }
                    RunResult::Updated(update) => {
                        validate_steps(&update.steps)?;
                        if !["ok", "partial", "failed", "blocked"]
                            .contains(&update.outcome.as_str())
                            || update.progress.trim().is_empty()
                        {
                            return Err(error("模型返回的目标进展格式无效"));
                        }
                        let progress = redact(update.progress.trim());
                        let next_action = update
                            .next_action
                            .as_deref()
                            .map(str::trim)
                            .filter(|text| !text.is_empty())
                            .map(redact);
                        let mut steps = update.steps;
                        for step in &mut steps {
                            step.text = redact(step.text.trim());
                        }
                        let changed = progress != before.progress.trim()
                            || next_action != before.next_action
                            || steps != before.steps;
                        goal.no_change_runs = if changed {
                            0
                        } else {
                            goal.no_change_runs.saturating_add(1)
                        };
                        goal.progress = progress;
                        goal.next_action = next_action;
                        goal.steps = steps;
                        goal.blocked_reason = update
                            .blocked_reason
                            .as_deref()
                            .map(str::trim)
                            .filter(|text| !text.is_empty())
                            .map(redact);
                        outcome = update.outcome;
                        message = goal.progress.clone();
                        if goal.blocked_reason.is_some() || outcome == "blocked" {
                            goal.status = "blocked".into();
                            outcome = "blocked".into();
                            let reason = goal.blocked_reason.get_or_insert_with(|| {
                                "需要你提供信息或决定是否授权，再继续推进".into()
                            });
                            message = format!("需要你决定：{reason}。已停止推进");
                        } else if update.done {
                            goal.status = "done".into();
                            outcome = "ok".into();
                            goal.next_action = None;
                            message = format!("目标已完成：{}", goal.progress);
                        } else if goal.used.runs >= goal.budget.max_runs {
                            goal.status = "paused".into();
                            message = format!("{}；已达到设定的推进次数上限", goal.progress);
                        } else if goal.no_change_runs >= 2 {
                            goal.status = "paused".into();
                            message = "连续两次推进没有实质变化，可能陷入死循环，已暂停".into();
                        } else if outcome == "failed" {
                            goal.status = "paused".into();
                            message = format!("推进失败，已暂停：{}", goal.progress);
                        }
                    }
                }
            }
            if goal.status != "active" {
                goal.auto_advance = false;
            }
            goal.checkpoints[checkpoint_index] = Checkpoint {
                at: now(),
                summary: message.clone(),
                outcome,
            };
            Ok(())
        });
        drop(_lease);
        observer();
        goal.map(|goal| GoalReport { goal, message })
    }

    async fn tick<Runner, Work, Observer>(
        &self,
        at: i64,
        busy: bool,
        timeout: Option<Duration>,
        runner: Runner,
        observer: Observer,
    ) -> Result<Option<GoalReport>, AppError>
    where
        Runner: FnOnce(AgentGoal) -> Work,
        Work: Future<Output = Result<GoalUpdate, AppError>>,
        Observer: Fn(),
    {
        if busy {
            self.defer_busy_tick(at)?;
            return Ok(None);
        }
        match self.due(at, busy)? {
            Some(id) => self
                .advance(&id, true, at, timeout, runner, observer)
                .await
                .map(Some),
            None => Ok(None),
        }
    }
}

enum RunResult {
    Updated(GoalUpdate),
    Timeout,
    Stopped,
    Failed,
}
struct RunLease<'a>(&'a GoalStore);
impl Drop for RunLease<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.state.lock() {
            if let Some((id, _, _)) = state.running.take() {
                if let Some(goal) = state.goals.iter_mut().find(|goal| goal.id == id) {
                    if goal
                        .checkpoints
                        .last()
                        .is_some_and(|checkpoint| checkpoint.summary == STARTED)
                    {
                        if goal.status == "active" {
                            goal.status = "paused".into();
                        }
                        goal.auto_advance = false;
                        goal.checkpoints.last_mut().unwrap().summary =
                            "本次推进被中断，已停止自动推进，请手动继续".into();
                        let _ = self.0.save(&state.goals);
                    }
                }
            }
        }
    }
}

pub fn store() -> Result<&'static GoalStore, AppError> {
    STORE.get().ok_or_else(|| error("目标存储尚未初始化"))
}

fn emit(app: &AppHandle) {
    wake_timer();
    if let Ok(snapshot) = store().and_then(GoalStore::snapshot) {
        let _ = app.emit("goals_changed", snapshot);
    }
}

pub fn wake_timer() {
    SCHEDULE_CHANGED.notify_one();
}

pub fn start(app: AppHandle) -> Result<(), AppError> {
    let goals = GoalStore::open(crate::config::data_dir().join("goals.json"))?;
    STORE.set(goals).map_err(|_| error("目标定时器已经启动"))?;
    tauri::async_runtime::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_secs(300));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        timer.reset_after(
            store()
                .and_then(|store| store.next_delay(now()))
                .unwrap_or(Duration::from_secs(300)),
        );
        loop {
            tokio::select! {
                _ = timer.tick() => {
                    let Ok(store) = store() else { break; };
                    let _ = store.tick(now(), CHAT_BUSY.load(Ordering::SeqCst) > 0, None,
                        |goal| run_goal_task(&app, goal), || emit(&app)).await;
                }
                _ = SCHEDULE_CHANGED.notified() => {}
            }
            timer.reset_after(
                store()
                    .and_then(|store| store.next_delay(now()))
                    .unwrap_or(Duration::from_secs(300)),
            );
        }
    });
    Ok(())
}

pub fn context(goal: &AgentGoal) -> String {
    let recent: Vec<_> = goal.checkpoints.iter().rev().take(3).rev().collect();
    json!({"description": goal.description, "steps": goal.steps, "progress": goal.progress,
        "next_action": goal.next_action, "checkpoints": recent})
    .to_string()
}

pub fn permissions(cfg: &crate::types::AppConfig) -> AgentPermissions {
    AgentPermissions {
        allow_download: cfg.agent_allow_download,
        allow_software: cfg.agent_allow_software,
        allow_file_write: cfg.agent_allow_file_write,
        allow_shell: cfg.agent_allow_shell,
        allow_tool_forge: cfg.agent_allow_tool_forge,
    }
}

fn parse_update(reply: &str) -> Result<GoalUpdate, AppError> {
    let text = reply.trim();
    let text = if text.starts_with("\x60\x60\x60") {
        text.split_once('\n')
            .and_then(|(_, tail)| tail.strip_suffix("\x60\x60\x60"))
            .unwrap_or(text)
            .trim()
    } else {
        text
    };
    let update: GoalUpdate = serde_json::from_str(text)?;
    validate_update(&update)?;
    Ok(update)
}

fn validate_update(update: &GoalUpdate) -> Result<(), AppError> {
    validate_steps(&update.steps)?;
    if update.progress.trim().is_empty()
        || !["ok", "partial", "failed", "blocked"].contains(&update.outcome.as_str())
    {
        return Err(error("目标进展格式无效"));
    }
    Ok(())
}

pub(super) fn check_goal_tool(
    name: &str,
    tools: &[Value],
    perms: &AgentPermissions,
) -> Result<(), AppError> {
    if name == "forge_tool" && !perms.allow_tool_forge {
        return Err(AppError::PermissionDenied(
            "目标推进未获得自造工具权限".into(),
        ));
    }
    if super::forged_tools::is_forged_call(name)
        || tools.iter().any(|tool| tool["function"]["name"] == name)
    {
        return Ok(());
    }
    Err(AppError::PermissionDenied(
        "目标推进没有获得此工具的权限，需要你决定是否授权".into(),
    ))
}

async fn run_goal_task(app: &AppHandle, goal: AgentGoal) -> Result<GoalUpdate, AppError> {
    let (base, key, model) = super::loop_::api_config()?;
    let cfg = crate::config::store::get_config();
    let perms = permissions(&cfg);
    let mut tools = build_tools(
        &crate::desktop::toolbox::list_agent_items(),
        &crate::plugin::with_manager(|manager| manager.plugins.clone()),
        &perms,
    );
    tools.extend(super::forged_tools::tool_definitions(
        perms.allow_tool_forge,
    ));
    let prompt = format!("推进这个长期目标一次，只完成本轮可以安全完成的工作。沿用现有权限；缺信息、需要花钱、安装软件或没有操作授权时立即停止并填写 blocked_reason，不得擅自决定。不要泄露密钥。根据实际结果更新步骤、进展和下一步；只有整个目标完成才能 done=true。最后只返回 JSON：{{\"progress\":\"中文进展\",\"next_action\":\"下一步或null\",\"steps\":[{{\"text\":\"步骤\",\"status\":\"pending/doing/done/failed\"}}],\"outcome\":\"ok/partial/failed/blocked\",\"done\":false,\"blocked_reason\":null}}。\n目标上下文：{}", context(&goal));
    let prompt = format!("禁止使用系统计划任务、注册表自启动项、启动文件夹或系统服务实现持续推进；只允许本次应用进程内任务。\n{prompt}");
    let messages = vec![
        json!({"role":"system","content":super::loop_::build_system_prompt(&tools)}),
        json!({"role":"user","content":prompt}),
    ];
    let request = crate::commands::agent_run::AgentRunRequest {
        task: prompt,
        request_id: format!("goal_{}_{}", goal.id, goal.used.runs.saturating_add(1)),
        max_steps: 10,
        progress_events: false,
    };
    let _cancel = super::cancel::CancelGuard::new(&request.request_id);
    let runtime = TaskRuntime {
        base,
        key,
        model,
        depth: cfg.depth,
        cfg,
        perms,
        dispatch: |app, name, args, sub_agent| {
            Box::pin(async move {
                if sub_agent {
                    return super::router::dispatch_sub_agent_tool_call(name, args).await;
                }
                let app = app.ok_or_else(|| error("目标工具执行环境不可用"))?;
                super::router::dispatch_tool_call(app, name, args).await
            })
        },
    };
    let trace = super::sub_agents::TaskTrace::default();
    let result = run_one_task(
        Some(app),
        request,
        &runtime,
        tools,
        messages,
        &StreamEmitter::silent(),
        None,
        false,
        Some(&trace),
    )
    .await?;
    if *trace
        .refused
        .lock()
        .map_err(|_| error("目标权限状态不可用"))?
    {
        return Ok(GoalUpdate {
            progress: goal.progress,
            next_action: goal.next_action,
            steps: goal.steps,
            outcome: "blocked".into(),
            done: false,
            blocked_reason: Some("本次操作需要额外授权，请在设置中确认权限后手动继续".into()),
        });
    }
    if result.interrupted {
        return Err(error("目标推进已取消"));
    }
    parse_update(&result.final_reply.unwrap_or_default())
}

pub async fn advance(app: &AppHandle, id: &str) -> Result<GoalReport, AppError> {
    store()?
        .advance(
            id,
            false,
            now(),
            None,
            |goal| run_goal_task(app, goal),
            || emit(app),
        )
        .await
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{"name":"create_goal","description":"创建跨会话长期目标（默认不自动推进）。只有用户要求长期目标时使用。创建后必须原样告诉用户：我建了一个目标：<标题>，你可以在「目标」面板里看到并让我继续。","parameters":{"type":"object","properties":{"title":{"type":"string"},"description":{"type":"string"},"steps":{"type":"array","items":{"type":"object","properties":{"text":{"type":"string"},"status":{"type":"string","enum":["pending","doing","done","failed"]}},"required":["text","status"]}}},"required":["title","description"]}}}),
        json!({"type":"function","function":{"name":"goal_status","description":"读取长期目标状态、进展、下一步和已用次数。不传 id 则列出目标。","parameters":{"type":"object","properties":{"id":{"type":"string"}}}}}),
        json!({"type":"function","function":{"name":"goal_advance","description":"用户要求继续某个长期目标时推进一次。仅进行中目标可推进，暂停或被阻塞的目标必须由用户在目标面板继续。","parameters":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}}}),
    ]
}

pub fn creation_notice(title: &str) -> String {
    format!("我建了一个目标：{title}，你可以在「目标」面板里看到并让我继续")
}

pub fn dispatch<'a>(
    app: &'a AppHandle,
    name: &'a str,
    args: &'a HashMap<String, Value>,
) -> std::pin::Pin<Box<dyn Future<Output = Result<String, AppError>> + Send + 'a>> {
    Box::pin(async move {
        let result = match name {
            "create_goal" => {
                let title = args
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let description = args
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let steps =
                    serde_json::from_value(args.get("steps").cloned().unwrap_or(json!([])))?;
                let goal = store()?.create(title, description, steps, GoalBudget::default())?;
                json!({"goal":goal,"notice":creation_notice(&goal.title)}).to_string()
            }
            "goal_status" => match args.get("id").and_then(Value::as_str) {
                Some(id) => serde_json::to_string(&store()?.get(id)?)?,
                None => serde_json::to_string(&store()?.snapshot()?)?,
            },
            "goal_advance" => serde_json::to_string(
                &Box::pin(advance(
                    app,
                    args.get("id").and_then(Value::as_str).unwrap_or_default(),
                ))
                .await?,
            )?,
            _ => return Err(error("未知目标工具")),
        };
        emit(app);
        Ok(result)
    })
}

#[cfg(test)]
mod tests;
