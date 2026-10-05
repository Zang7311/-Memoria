use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct Fixture {
    store: GoalStore,
    directory: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("mem-goal-recovery-{}", uuid::Uuid::new_v4()));
        Self {
            store: GoalStore::open(directory.join("goals.json")).unwrap(),
            directory,
        }
    }

    fn goal(&self, max_steps: u32) -> AgentGoal {
        let mut budget = GoalRunBudget::default();
        budget.max_steps_per_run = max_steps;
        self.store
            .create(
                "整理 README".into(),
                "读取资料并整理项目 README".into(),
                vec![],
                budget,
            )
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

struct Reply {
    status: u16,
    body: Value,
    delay: Duration,
}

impl Reply {
    fn message(message: Value) -> Self {
        Self {
            status: 200,
            body: json!({"choices":[{"message":message}]}),
            delay: Duration::ZERO,
        }
    }

    fn text(text: &str) -> Self {
        Self::message(json!({"role":"assistant","content":text}))
    }

    fn tool() -> Self {
        Self::message(json!({"role":"assistant","content":null,"tool_calls":[{
            "id":"read_call","type":"function","function":{"name":"toolbox_agent_read_file","arguments":"{}"}
        }]}))
    }

    fn update(outcome: &str, done: bool) -> Self {
        Self::text(
            &json!({"progress":"已读取 README 并整理结构","next_action":"补充使用说明",
            "steps":[{"text":"读取 README","status":"done"}],"outcome":outcome,"done":done})
            .to_string(),
        )
    }
}

struct Server {
    base: String,
    requests: Arc<Mutex<Vec<Value>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Server {
    async fn new(replies: Vec<Reply>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let received = requests.clone();
        let task = tokio::spawn(async move {
            for reply in replies {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buffer = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let size = socket.read(&mut chunk).await.unwrap();
                    assert!(size > 0, "请求在读取完成前断开");
                    buffer.extend_from_slice(&chunk[..size]);
                    if let Some(index) = buffer.windows(4).position(|window| window == b"\r\n\r\n")
                    {
                        let headers = String::from_utf8_lossy(&buffer[..index]);
                        let length: usize = headers
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|value| value.trim().parse().unwrap())
                            })
                            .unwrap();
                        if buffer.len() >= index + 4 + length {
                            received.lock().unwrap().push(
                                serde_json::from_slice(&buffer[index + 4..index + 4 + length])
                                    .unwrap(),
                            );
                            break;
                        }
                    }
                }
                tokio::spawn(async move {
                    tokio::time::sleep(reply.delay).await;
                    let body = reply.body.to_string();
                    let response = format!("HTTP/1.1 {} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", reply.status, body.len());
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        Self {
            base,
            requests,
            task,
        }
    }

    fn runtime(&self) -> TaskRuntime {
        let mut cfg = crate::config::defaults::default_config();
        cfg.self_check_enabled = false;
        TaskRuntime {
            base: self.base.clone(),
            key: "mock-recovery-key".into(),
            model: "mock-model".into(),
            depth: cfg.depth,
            perms: permissions(&cfg),
            cfg,
            dispatch: |_, _, _, _| {
                Box::pin(async { Ok("README 内容：项目安装与运行说明".into()) })
            },
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn advance_mock(
    fixture: &Fixture,
    goal: &AgentGoal,
    runtime: &TaskRuntime,
    timeout: Option<Duration>,
) -> GoalReport {
    let execution = Arc::new(Mutex::new(GoalExecution::default()));
    let run_record = execution.clone();
    let summary_record = execution.clone();
    fixture.store.advance_recovering(
        &goal.id, false, now(), timeout, execution,
        |goal| async move {
            let messages = vec![json!({"role":"user","content":context(&goal)})];
            let tools = vec![json!({"type":"function","function":{"name":"toolbox_agent_read_file","parameters":{"type":"object"}}})];
            run_goal_runtime(None, goal, runtime, tools, messages, String::new(), run_record).await
        },
        |goal| summarize_goal_runtime(goal, summary_record, &runtime.base, &runtime.key, &runtime.model, &runtime.cfg),
        || {},
    ).await.unwrap()
}

fn assert_retained(report: &GoalReport, outcome: &str) {
    assert!(!report.goal.progress.trim().is_empty());
    assert!(!report.goal.checkpoints[0].summary.trim().is_empty());
    assert_eq!(report.goal.checkpoints[0].outcome, outcome);
    assert!(!report.message.contains("模型配置"));
}

#[tokio::test]
async fn capped_real_loop_recovers_partial_and_remains_active() {
    let fixture = Fixture::new();
    let goal = fixture.goal(2);
    let server = Server::new(vec![
        Reply::tool(),
        Reply::tool(),
        Reply::update("ok", true),
    ])
    .await;
    let report = advance_mock(&fixture, &goal, &server.runtime(), None).await;
    assert_eq!(report.goal.status, "active");
    assert_eq!(report.goal.used.runs, 1);
    assert_eq!(report.goal.used.calls, 3);
    assert!(report.message.contains("本轮达到步数上限（2 轮）"));
    assert_eq!(report.goal.progress, "已读取 README 并整理结构");
    assert_eq!(report.goal.steps[0].status, "done");
    assert_retained(&report, "partial");
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests[2].get("tools").is_none());
    assert!(requests[2].get("tool_choice").is_none());
    assert!(requests[2]["messages"].to_string().contains("README 内容"));
}

#[tokio::test]
async fn malformed_reply_gets_one_tool_free_fallback_and_two_calls() {
    let fixture = Fixture::new();
    let goal = fixture.goal(30);
    let mut summary = Reply::update("ok", false);
    summary.delay = Duration::from_millis(1100);
    let server = Server::new(vec![Reply::text("已完成本轮资料整理"), summary]).await;
    let report = advance_mock(&fixture, &goal, &server.runtime(), None).await;
    assert_eq!(report.goal.status, "active");
    assert_eq!(report.goal.used.runs, 1);
    assert_eq!(report.goal.used.calls, 2);
    assert!(report.goal.used.total_seconds >= 1);
    assert_retained(&report, "ok");
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].get("tools").is_none());
    assert!(requests[1]["messages"]
        .to_string()
        .contains("已完成本轮资料整理"));
    let saved = GoalStore::open(fixture.store.path.clone())
        .unwrap()
        .get(&goal.id)
        .unwrap();
    assert_eq!(saved.used.calls, 2);
    assert_eq!(saved.progress, report.goal.progress);
}

#[tokio::test]
async fn both_reports_invalid_pause_with_retained_tool_progress() {
    let fixture = Fixture::new();
    let goal = fixture.goal(30);
    let server = Server::new(vec![
        Reply::tool(),
        Reply::text("已读取 README"),
        Reply::text("仍然不是 JSON"),
    ])
    .await;
    let report = advance_mock(&fixture, &goal, &server.runtime(), None).await;
    assert_eq!(report.goal.status, "paused");
    assert_eq!(report.goal.used.calls, 3);
    assert!(report.message.contains("模型两次都没按格式汇报"));
    assert!(report.message.contains("保留进展"));
    assert!(report.goal.progress.contains("读取文件 1 次"));
    assert_retained(&report, "failed");
}

#[tokio::test]
async fn cap_and_failed_summary_pause_partial_without_discarding_tools() {
    let fixture = Fixture::new();
    let goal = fixture.goal(1);
    let server = Server::new(vec![Reply::tool(), Reply::text("非 JSON")]).await;
    let report = advance_mock(&fixture, &goal, &server.runtime(), None).await;
    assert_eq!(report.goal.status, "paused");
    assert!(report.message.contains("本轮达到步数上限（1 轮）"));
    assert!(report.message.contains("已保留本次进展记录"));
    assert!(report.goal.progress.contains("读取文件 1 次"));
    assert_retained(&report, "partial");
}

#[tokio::test]
async fn timeout_after_tool_use_recovers_partial_with_timeout_reason() {
    let fixture = Fixture::new();
    let goal = fixture.goal(30);
    let mut delayed = Reply::text("过晚的回复");
    delayed.delay = Duration::from_millis(500);
    let server = Server::new(vec![Reply::tool(), delayed, Reply::update("failed", true)]).await;
    let report = advance_mock(
        &fixture,
        &goal,
        &server.runtime(),
        Some(Duration::from_millis(150)),
    )
    .await;
    assert_eq!(report.goal.status, "active");
    assert_eq!(report.goal.used.calls, 3);
    assert!(report.message.contains("本轮超时（0.15 秒）"));
    assert_retained(&report, "partial");
    let requests = server.requests.lock().unwrap();
    assert!(requests[2].get("tools").is_none());
    assert!(requests[2]["messages"].to_string().contains("README 内容"));
    assert!(requests[2]["messages"].to_string().contains("timed_out"));
}

#[tokio::test]
async fn timeout_with_failed_fallback_still_preserves_partial() {
    let fixture = Fixture::new();
    let goal = fixture.goal(30);
    let mut delayed = Reply::text("过晚的回复");
    delayed.delay = Duration::from_millis(500);
    let server = Server::new(vec![Reply::tool(), delayed, Reply::text("非 JSON")]).await;
    let report = advance_mock(
        &fixture,
        &goal,
        &server.runtime(),
        Some(Duration::from_millis(150)),
    )
    .await;
    assert_eq!(report.goal.status, "paused");
    assert!(report.message.contains("本轮超时"));
    assert!(report.message.contains("已保留本次进展记录"));
    assert!(report.goal.progress.contains("读取文件 1 次"));
    assert_retained(&report, "partial");
}

#[tokio::test]
async fn valid_json_needs_no_fallback_and_still_completes_goal() {
    let fixture = Fixture::new();
    let goal = fixture.goal(30);
    let server = Server::new(vec![Reply::update("ok", true)]).await;
    let report = advance_mock(&fixture, &goal, &server.runtime(), None).await;
    assert_eq!(report.goal.status, "done");
    assert_eq!(report.goal.used.calls, 1);
    assert_eq!(server.requests.lock().unwrap().len(), 1);
    assert_retained(&report, "ok");
}

#[tokio::test]
async fn empty_final_reply_also_gets_the_json_fallback() {
    let fixture = Fixture::new();
    let goal = fixture.goal(30);
    let server = Server::new(vec![Reply::text(""), Reply::update("partial", false)]).await;
    let report = advance_mock(&fixture, &goal, &server.runtime(), None).await;
    assert_eq!(report.goal.status, "active");
    assert_eq!(report.goal.used.calls, 2);
    assert_retained(&report, "partial");
}

#[tokio::test]
async fn goals_really_run_thirty_rounds_past_the_chat_hard_cap() {
    let fixture = Fixture::new();
    let goal = fixture.goal(GoalRunBudget::default().max_steps_per_run);
    let mut replies: Vec<_> = (0..30).map(|_| Reply::tool()).collect();
    replies.push(Reply::update("partial", false));
    let server = Server::new(replies).await;
    let report = advance_mock(&fixture, &goal, &server.runtime(), None).await;
    assert_eq!(report.goal.status, "active");
    assert_eq!(report.goal.used.calls, 31);
    assert_eq!(server.requests.lock().unwrap().len(), 31);
    assert!(report.message.contains("本轮达到步数上限（30 轮）"));
    assert_retained(&report, "partial");
}

#[tokio::test]
async fn real_api_error_reports_reason_without_trying_format_recovery() {
    let fixture = Fixture::new();
    let goal = fixture.goal(30);
    let server = Server::new(vec![Reply {
        status: 401,
        body: json!({"error":"unauthorized mock-recovery-key"}),
        delay: Duration::ZERO,
    }])
    .await;
    let report = advance_mock(&fixture, &goal, &server.runtime(), None).await;
    assert_eq!(report.goal.status, "paused");
    assert_eq!(report.goal.used.calls, 1);
    assert!(report.message.contains("调用模型失败"));
    assert!(report.message.contains("401"));
    assert!(!report.message.contains("mock-recovery-key"));
    assert_retained(&report, "failed");
}

#[tokio::test]
async fn recovered_partial_still_obeys_total_run_budget() {
    let fixture = Fixture::new();
    let mut goal = fixture.goal(1);
    goal.budget.max_runs = 1;
    fixture
        .store
        .settings(&goal.id, false, 300, goal.budget.clone())
        .unwrap();
    let server = Server::new(vec![Reply::tool(), Reply::update("ok", true)]).await;
    let report = advance_mock(&fixture, &goal, &server.runtime(), None).await;
    assert_eq!(report.goal.status, "paused");
    assert!(report.message.contains("推进次数上限"));
    assert_retained(&report, "partial");
}

#[test]
fn old_goals_file_defaults_steps_to_thirty_and_preserves_usage() {
    let fixture = Fixture::new();
    let goal = fixture.goal(7);
    let mut stored = serde_json::to_value(&goal).unwrap();
    stored["budget"]
        .as_object_mut()
        .unwrap()
        .remove("max_steps_per_run");
    stored["used"].as_object_mut().unwrap().remove("calls");
    std::fs::write(
        &fixture.store.path,
        serde_json::to_vec(&vec![stored]).unwrap(),
    )
    .unwrap();
    let reopened = GoalStore::open(fixture.store.path.clone()).unwrap();
    let loaded = reopened.get(&goal.id).unwrap();
    assert_eq!(loaded.budget.max_steps_per_run, 30);
    assert_eq!(loaded.used.calls, 0);
    assert_eq!(loaded.status, "active");
    assert_eq!(loaded.progress, goal.progress);
    let persisted: Value =
        serde_json::from_slice(&std::fs::read(&fixture.store.path).unwrap()).unwrap();
    assert_eq!(persisted[0]["budget"]["max_steps_per_run"], 30);
}

#[tokio::test]
async fn failed_fallback_api_reports_the_actual_reason_and_keeps_progress() {
    let fixture = Fixture::new();
    let goal = fixture.goal(30);
    let server = Server::new(vec![
        Reply::tool(),
        Reply::text("已读取文件"),
        Reply {
            status: 503,
            body: json!({"error":"暂时不可用"}),
            delay: Duration::ZERO,
        },
    ])
    .await;
    let report = advance_mock(&fixture, &goal, &server.runtime(), None).await;
    assert_eq!(report.goal.status, "paused");
    assert_eq!(report.goal.used.calls, 3);
    assert!(report.message.contains("调用模型失败"));
    assert!(report.message.contains("503"));
    assert!(report.message.contains("保留进展"));
    assert!(report.goal.progress.contains("读取文件 1 次"));
    assert_retained(&report, "failed");
}

#[tokio::test]
async fn pause_and_cancel_can_interrupt_the_fallback_summary() {
    for status in ["paused", "cancelled"] {
        let fixture = Fixture::new();
        let goal = fixture.goal(30);
        let mut summary = Reply::update("ok", true);
        summary.delay = Duration::from_millis(500);
        let server = Server::new(vec![Reply::text("需要总结"), summary]).await;
        let runtime = server.runtime();
        let (report, _) = tokio::join!(advance_mock(&fixture, &goal, &runtime, None), async {
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    if server.requests.lock().unwrap().len() == 2 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            fixture.store.set_status(&goal.id, status).unwrap();
        });
        assert_eq!(report.goal.status, status);
        assert_eq!(report.goal.used.calls, 2);
        assert!(report.message.contains("用户已"));
        assert_retained(&report, "partial");
        assert!(fixture.store.snapshot().unwrap().running_id.is_none());
    }
}

#[test]
fn step_budget_is_persisted_and_zero_is_rejected() {
    let fixture = Fixture::new();
    let goal = fixture.goal(7);
    assert_eq!(
        GoalStore::open(fixture.store.path.clone())
            .unwrap()
            .get(&goal.id)
            .unwrap()
            .budget
            .max_steps_per_run,
        7
    );
    let mut budget = goal.budget.clone();
    budget.max_steps_per_run = 0;
    assert!(fixture
        .store
        .settings(&goal.id, false, 300, budget.clone())
        .is_err());
    assert!(fixture
        .store
        .create("标题".into(), "说明".into(), vec![], budget.clone())
        .is_err());
    let mut stored = serde_json::to_value(&goal).unwrap();
    stored["budget"]["max_steps_per_run"] = json!(0);
    std::fs::write(
        &fixture.store.path,
        serde_json::to_vec(&vec![stored]).unwrap(),
    )
    .unwrap();
    assert!(GoalStore::open(fixture.store.path.clone()).is_err());
}
