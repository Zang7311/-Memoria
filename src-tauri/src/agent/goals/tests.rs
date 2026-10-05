use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture {
    store: GoalStore,
    directory: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("mem-goals-{}", uuid::Uuid::new_v4()));
        let store = GoalStore::open(directory.join("goals.json")).unwrap();
        Self { store, directory }
    }
    fn goal(&self) -> AgentGoal {
        self.store
            .create(
                "整理研究资料".into(),
                "整理资料并撰写报告".into(),
                vec![GoalStep {
                    text: "检索资料".into(),
                    status: "pending".into(),
                }],
                GoalBudget::default(),
            )
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn update(_goal: &AgentGoal) -> GoalUpdate {
    GoalUpdate {
        progress: "已找到相关资料".into(),
        next_action: Some("撰写报告".into()),
        steps: vec![
            GoalStep {
                text: "检索资料".into(),
                status: "done".into(),
            },
            GoalStep {
                text: "撰写报告".into(),
                status: "pending".into(),
            },
        ],
        outcome: "ok".into(),
        done: false,
        blocked_reason: None,
    }
}
fn unchanged(goal: &AgentGoal) -> GoalUpdate {
    GoalUpdate {
        progress: goal.progress.clone(),
        next_action: goal.next_action.clone(),
        steps: goal.steps.clone(),
        outcome: "partial".into(),
        done: false,
        blocked_reason: None,
    }
}
async fn once(store: &GoalStore, goal: &AgentGoal) -> GoalReport {
    store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |goal| async move { Ok(update(&goal)) },
            || {},
        )
        .await
        .unwrap()
}
fn auto(fixture: &Fixture, goal: &AgentGoal) {
    fixture
        .store
        .settings(&goal.id, true, 1, goal.budget.clone())
        .unwrap();
    fixture
        .store
        .edit(&goal.id, |goal| {
            goal.last_auto_at = 0;
            Ok(())
        })
        .unwrap();
}

#[test]
fn defaults_crud_and_restart_persistence() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    assert!(!goal.auto_advance);
    assert_eq!(goal.auto_interval_secs, 300);
    assert_eq!(goal.budget.max_runs, 20);
    assert_eq!(goal.budget.max_seconds_per_run, 600);
    fixture.store.set_status(&goal.id, "paused").unwrap();
    fixture.store.set_status(&goal.id, "active").unwrap();
    let reopened = GoalStore::open(fixture.store.path.clone()).unwrap();
    assert_eq!(reopened.get(&goal.id).unwrap().status, "active");
    reopened.delete(&goal.id).unwrap();
    assert!(GoalStore::open(fixture.store.path.clone())
        .unwrap()
        .snapshot()
        .unwrap()
        .goals
        .is_empty());
}

#[test]
fn old_json_defaults_and_invalid_inputs() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    let mut value = serde_json::to_value(&goal).unwrap();
    for key in [
        "auto_advance",
        "auto_interval_secs",
        "auto_runs",
        "no_change_runs",
        "last_auto_at",
    ] {
        value.as_object_mut().unwrap().remove(key);
    }
    let migrated: AgentGoal = serde_json::from_value(value).unwrap();
    assert!(!migrated.auto_advance);
    assert_eq!(migrated.auto_interval_secs, 300);
    assert!(fixture
        .store
        .create("".into(), "说明".into(), vec![], GoalBudget::default())
        .is_err());
    assert!(fixture
        .store
        .settings(&goal.id, true, 0, goal.budget.clone())
        .is_err());
    assert!(fixture.store.set_status(&goal.id, "done").is_err());
}

#[tokio::test]
async fn advance_updates_all_progress_fields_and_persists_usage() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    let report = once(&fixture.store, &goal).await;
    assert_eq!(report.goal.progress, "已找到相关资料");
    assert_eq!(report.goal.next_action.as_deref(), Some("撰写报告"));
    assert_eq!(report.goal.steps[0].status, "done");
    assert_eq!(report.goal.steps.len(), 2);
    assert_eq!(report.goal.used.runs, 1);
    assert_eq!(report.goal.checkpoints.len(), 1);
    assert_eq!(report.goal.checkpoints[0].outcome, "ok");
    assert_eq!(
        GoalStore::open(fixture.store.path.clone())
            .unwrap()
            .get(&goal.id)
            .unwrap()
            .used
            .runs,
        1
    );
    assert!(fixture.store.snapshot().unwrap().running_id.is_none());
}

#[tokio::test]
async fn done_has_final_checkpoint_and_completion_summary() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    let report = fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |goal| async move {
                let mut result = update(&goal);
                result.done = true;
                Ok(result)
            },
            || {},
        )
        .await
        .unwrap();
    assert_eq!(report.goal.status, "done");
    assert!(report.message.contains("目标已完成"));
    assert_eq!(report.goal.checkpoints[0].summary, report.message);
    assert!(report.goal.next_action.is_none());
    assert!(fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |_| async { panic!("不应继续运行") },
            || {}
        )
        .await
        .is_err());
}

#[tokio::test]
async fn budget_pauses_after_last_run_and_prevents_extra_calls() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    fixture
        .store
        .settings(
            &goal.id,
            false,
            300,
            GoalBudget {
                max_runs: 1,
                max_seconds_per_run: 600,
            },
        )
        .unwrap();
    let report = once(&fixture.store, &goal).await;
    assert_eq!(report.goal.status, "paused");
    assert!(report.message.contains("已达到设定的推进次数上限"));
    assert_eq!(report.goal.used.runs, 1);
    assert!(fixture.store.set_status(&goal.id, "active").is_err());
    assert!(fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |_| async { panic!("超预算不应调用 API") },
            || {}
        )
        .await
        .is_err());
    fixture
        .store
        .settings(
            &goal.id,
            false,
            300,
            GoalBudget {
                max_runs: 2,
                max_seconds_per_run: 600,
            },
        )
        .unwrap();
    fixture.store.set_status(&goal.id, "active").unwrap();
    assert_eq!(once(&fixture.store, &goal).await.goal.used.runs, 2);
}

#[tokio::test]
async fn already_exhausted_budget_pauses_before_runner() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    fixture
        .store
        .edit(&goal.id, |goal| {
            goal.used.runs = goal.budget.max_runs;
            Ok(())
        })
        .unwrap();
    let report = fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |_| async { panic!("不应运行") },
            || {},
        )
        .await
        .unwrap();
    assert_eq!(report.goal.status, "paused");
    assert_eq!(report.goal.used.runs, 20);
    assert!(report.message.contains("已达到设定的推进次数上限"));
}

#[tokio::test]
async fn injected_short_timeout_pauses_and_records_partial() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    let report = fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            Some(Duration::from_millis(5)),
            |_| async { std::future::pending::<Result<GoalUpdate, AppError>>().await },
            || {},
        )
        .await
        .unwrap();
    assert_eq!(report.goal.status, "paused");
    assert_eq!(report.goal.checkpoints[0].outcome, "partial");
    assert!(report.message.contains("超时"));
    assert_eq!(report.goal.used.runs, 1);
}

#[tokio::test]
async fn missing_information_or_spending_decision_blocks() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    let report = fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |goal| async move {
                let mut result = update(&goal);
                result.blocked_reason = Some("购买数据库需要你决定是否花钱".into());
                Ok(result)
            },
            || {},
        )
        .await
        .unwrap();
    assert_eq!(report.goal.status, "blocked");
    assert!(report.goal.blocked_reason.unwrap().contains("花钱"));
    assert!(report.message.contains("需要你决定"));
    assert_eq!(report.goal.checkpoints[0].outcome, "blocked");
    assert!(fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |_| async { panic!("blocked 不应运行") },
            || {}
        )
        .await
        .is_err());
}

#[tokio::test]
async fn blocked_outcome_without_reason_gets_safe_chinese_reason() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    let report = fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |goal| async move {
                let mut result = update(&goal);
                result.outcome = "blocked".into();
                result.done = true;
                Ok(result)
            },
            || {},
        )
        .await
        .unwrap();
    assert_eq!(report.goal.status, "blocked");
    assert!(report.goal.blocked_reason.is_some());
}

#[tokio::test]
async fn two_unchanged_runs_pause_and_counter_survives_restart() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    let first = fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |goal| async move { Ok(unchanged(&goal)) },
            || {},
        )
        .await
        .unwrap();
    assert_eq!(first.goal.status, "active");
    let reopened = GoalStore::open(fixture.store.path.clone()).unwrap();
    let second = reopened
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |goal| async move { Ok(unchanged(&goal)) },
            || {},
        )
        .await
        .unwrap();
    assert_eq!(second.goal.status, "paused");
    assert!(second.message.contains("死循环"));
    assert_eq!(second.goal.used.runs, 2);
}

#[tokio::test]
async fn real_change_resets_no_change_counter() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |goal| async move { Ok(unchanged(&goal)) },
            || {},
        )
        .await
        .unwrap();
    assert_eq!(once(&fixture.store, &goal).await.goal.no_change_runs, 0);
}

#[tokio::test]
async fn cancellation_prevents_manual_and_automatic_advances() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    auto(&fixture, &goal);
    let cancelled = fixture.store.set_status(&goal.id, "cancelled").unwrap();
    assert_eq!(cancelled.status, "cancelled");
    assert!(!cancelled.auto_advance);
    assert!(fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |_| async { panic!("取消后不能运行") },
            || {}
        )
        .await
        .is_err());
    assert!(fixture
        .store
        .tick(
            now(),
            false,
            None,
            |_| async { panic!("取消后不能自动运行") },
            || {}
        )
        .await
        .unwrap()
        .is_none());
    assert!(fixture.store.set_status(&goal.id, "active").is_err());
}

#[tokio::test]
async fn pause_and_cancel_interrupt_inflight_work_without_overwriting_status() {
    for status in ["paused", "cancelled"] {
        let fixture = Fixture::new();
        let goal = fixture.goal();
        let started = Notify::new();
        let work = fixture.store.advance(
            &goal.id,
            false,
            now(),
            None,
            |_| async {
                started.notify_one();
                std::future::pending::<Result<GoalUpdate, AppError>>().await
            },
            || {},
        );
        let control = async {
            started.notified().await;
            assert!(fixture.store.delete(&goal.id).is_err());
            fixture.store.set_status(&goal.id, status).unwrap();
        };
        let (report, _) = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(work, control)
        })
        .await
        .unwrap();
        let report = report.unwrap();
        assert_eq!(report.goal.status, status);
        assert_eq!(report.goal.used.runs, 1);
        assert!(report.message.contains(if status == "paused" {
            "暂停"
        } else {
            "取消"
        }));
    }
}

#[tokio::test]
async fn default_timer_never_calls_runner() {
    let fixture = Fixture::new();
    fixture.goal();
    assert!(fixture
        .store
        .tick(
            now() + 10000,
            false,
            None,
            |_| async { panic!("默认关闭自动推进") },
            || {}
        )
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn timer_respects_interval_and_one_goal_per_tick() {
    let fixture = Fixture::new();
    let first = fixture.goal();
    let second = fixture.goal();
    auto(&fixture, &first);
    auto(&fixture, &second);
    assert!(fixture
        .store
        .tick(0, false, None, |_| async { panic!("尚未到期") }, || {})
        .await
        .unwrap()
        .is_none());
    fixture
        .store
        .tick(
            1,
            false,
            None,
            |goal| async move { Ok(update(&goal)) },
            || {},
        )
        .await
        .unwrap()
        .unwrap();
    let snapshot = fixture.store.snapshot().unwrap();
    assert_eq!(
        snapshot
            .goals
            .iter()
            .map(|goal| goal.used.runs)
            .sum::<u32>(),
        1
    );
    assert_eq!(
        snapshot
            .goals
            .iter()
            .map(|goal| goal.auto_runs)
            .sum::<u32>(),
        1
    );
    fixture
        .store
        .tick(
            1,
            false,
            None,
            |goal| async move { Ok(update(&goal)) },
            || {},
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fixture.store.get(&first.id).unwrap().used.runs, 1);
    assert_eq!(fixture.store.get(&second.id).unwrap().used.runs, 1);
    assert!(fixture
        .store
        .tick(
            1,
            false,
            None,
            |_| async { panic!("两者都未到下次间隔") },
            || {}
        )
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn injected_busy_chat_skips_tick_without_consuming_budget() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    auto(&fixture, &goal);
    assert!(fixture
        .store
        .tick(
            now(),
            true,
            None,
            |_| async { panic!("忙碌时不能运行") },
            || {}
        )
        .await
        .unwrap()
        .is_none());
    assert_eq!(fixture.store.get(&goal.id).unwrap().used.runs, 0);
}

#[tokio::test]
async fn all_automatic_stop_conditions_disable_future_ticks() {
    for stop in ["budget", "timeout", "blocked", "done", "no_change"] {
        let fixture = Fixture::new();
        let goal = fixture.goal();
        auto(&fixture, &goal);
        if stop == "budget" {
            fixture
                .store
                .settings(
                    &goal.id,
                    true,
                    1,
                    GoalBudget {
                        max_runs: 1,
                        max_seconds_per_run: 600,
                    },
                )
                .unwrap();
        }
        if stop == "no_change" {
            fixture
                .store
                .edit(&goal.id, |goal| {
                    goal.no_change_runs = 1;
                    Ok(())
                })
                .unwrap();
        }
        let timeout = (stop == "timeout").then_some(Duration::from_millis(5));
        let report = fixture
            .store
            .tick(
                now(),
                false,
                timeout,
                |goal| async move {
                    if stop == "timeout" {
                        return std::future::pending::<Result<GoalUpdate, AppError>>().await;
                    }
                    let mut result = if stop == "no_change" {
                        unchanged(&goal)
                    } else {
                        update(&goal)
                    };
                    if stop == "blocked" {
                        result.blocked_reason = Some("需要安装软件授权".into());
                    }
                    if stop == "done" {
                        result.done = true;
                    }
                    Ok(result)
                },
                || {},
            )
            .await
            .unwrap()
            .unwrap();
        assert_ne!(report.goal.status, "active", "{stop}");
        assert!(!report.goal.auto_advance, "{stop}");
        assert!(fixture
            .store
            .tick(
                now() + 10000,
                false,
                None,
                |_| async { panic!("停手后不能再自动推进") },
                || {}
            )
            .await
            .unwrap()
            .is_none());
    }
}

#[tokio::test]
async fn concurrent_manual_and_timer_runs_are_rejected() {
    let fixture = Fixture::new();
    let first = fixture.goal();
    let second = fixture.goal();
    auto(&fixture, &second);
    let started = Notify::new();
    let running = fixture.store.advance(
        &first.id,
        false,
        now(),
        None,
        |_| async {
            started.notify_one();
            std::future::pending::<Result<GoalUpdate, AppError>>().await
        },
        || {},
    );
    let concurrent = async {
        started.notified().await;
        assert!(fixture
            .store
            .advance(
                &second.id,
                false,
                now(),
                None,
                |_| async { panic!("禁止并行推进") },
                || {}
            )
            .await
            .is_err());
        assert!(fixture
            .store
            .tick(
                now(),
                false,
                None,
                |_| async { panic!("禁止并行自动推进") },
                || {}
            )
            .await
            .unwrap()
            .is_none());
        fixture.store.set_status(&first.id, "paused").unwrap();
    };
    let (report, _) = tokio::join!(running, concurrent);
    assert!(report.is_ok());
    assert_eq!(fixture.store.get(&second.id).unwrap().used.runs, 0);
}

#[tokio::test]
async fn runner_errors_record_failed_checkpoint_and_pause() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    let report = fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |_| async { Err(error("password=不可记录的密钥")) },
            || {},
        )
        .await
        .unwrap();
    assert_eq!(report.goal.status, "paused");
    assert_eq!(report.goal.checkpoints[0].outcome, "failed");
    assert!(!report.message.contains("不可记录的密钥"));
    assert_eq!(report.goal.used.runs, 1);
}

#[tokio::test]
async fn dropping_run_future_releases_lock_and_preserves_reserved_budget() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    let mut work = Box::pin(fixture.store.advance(
        &goal.id,
        false,
        now(),
        None,
        |_| async { std::future::pending::<Result<GoalUpdate, AppError>>().await },
        || {},
    ));
    tokio::select! { biased; _ = &mut work => panic!("不应结束"), _ = tokio::time::sleep(Duration::from_millis(5)) => {} }
    drop(work);
    let saved = fixture.store.get(&goal.id).unwrap();
    assert_eq!(saved.status, "paused");
    assert_eq!(saved.used.runs, 1);
    assert!(fixture.store.snapshot().unwrap().running_id.is_none());
}

#[test]
fn crash_recovery_does_not_automatically_resume_unfinished_work() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    fixture
        .store
        .edit(&goal.id, |goal| {
            goal.auto_advance = true;
            goal.used.runs = 1;
            goal.checkpoints.push(Checkpoint {
                at: now(),
                summary: STARTED.into(),
                outcome: "partial".into(),
            });
            Ok(())
        })
        .unwrap();
    let recovered = GoalStore::open(fixture.store.path.clone())
        .unwrap()
        .get(&goal.id)
        .unwrap();
    assert_eq!(recovered.status, "paused");
    assert!(!recovered.auto_advance);
    assert_eq!(recovered.used.runs, 1);
}

#[test]
fn context_contains_resume_state_and_only_last_three_checkpoints() {
    let fixture = Fixture::new();
    let mut goal = fixture.goal();
    goal.checkpoints = (0..5)
        .map(|index| Checkpoint {
            at: index,
            summary: format!("记录{index}"),
            outcome: "ok".into(),
        })
        .collect();
    let value: Value = serde_json::from_str(&context(&goal)).unwrap();
    assert_eq!(value["checkpoints"].as_array().unwrap().len(), 3);
    assert_eq!(value["checkpoints"][0]["summary"], "记录2");
    for key in ["description", "steps", "progress", "next_action"] {
        assert!(value.get(key).is_some());
    }
}

#[tokio::test]
async fn goal_content_and_checkpoints_redact_secret_assignments() {
    let fixture = Fixture::new();
    let goal = fixture
        .store
        .create(
            "password=title-secret".into(),
            "api_key:description-secret".into(),
            vec![],
            GoalBudget::default(),
        )
        .unwrap();
    let report = fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |_| async {
                Ok(GoalUpdate {
                    progress: "token=progress-secret".into(),
                    next_action: Some("secret=next-secret".into()),
                    steps: vec![GoalStep {
                        text: "password=step-secret".into(),
                        status: "pending".into(),
                    }],
                    outcome: "blocked".into(),
                    done: false,
                    blocked_reason: Some("password=reason-secret".into()),
                })
            },
            || {},
        )
        .await
        .unwrap();
    let saved = std::fs::read_to_string(&fixture.store.path).unwrap();
    for secret in [
        "title-secret",
        "description-secret",
        "progress-secret",
        "next-secret",
        "step-secret",
        "reason-secret",
    ] {
        assert!(!saved.contains(secret));
        assert!(!serde_json::to_string(&report).unwrap().contains(secret));
    }
}

#[test]
fn goal_tools_and_forging_obey_existing_permission_gates() {
    let cfg = crate::config::defaults::default_config();
    let perms = permissions(&cfg);
    let items = crate::desktop::toolbox::list_agent_items();
    let tools = build_tools(&items, &[], &perms);
    for item in items.iter().filter(|item| {
        item.agent_permission.is_some() && !perms.allows(item.agent_permission.as_deref())
    }) {
        assert!(!tools
            .iter()
            .any(|tool| tool["function"]["name"] == format!("toolbox_{}", item.id)));
    }
    assert!(!tools
        .iter()
        .any(|tool| tool["function"]["name"] == "toolbox_agent_write_file"));
    assert!(!tools
        .iter()
        .any(|tool| tool["function"]["name"] == "toolbox_agent_shell"));
    assert!(!super::super::forged_tools::tool_definitions(false)
        .iter()
        .any(|tool| tool["function"]["name"] == "forge_tool"));
    assert!(super::super::sub_agents::check_tool_call(
        "toolbox_agent_write_file",
        &HashMap::new(),
        &tools
    )
    .is_err());
}

#[test]
fn strict_result_parser_and_required_creation_notice() {
    assert!(parse_update("我觉得完成了").is_err());
    let fixture = Fixture::new();
    let goal = fixture.goal();
    let value = json!({"progress":"实际进展","next_action":"继续","steps":goal.steps,"outcome":"ok","done":false,"blocked_reason":null});
    assert!(parse_update(&value.to_string()).is_ok());
    assert!(parse_update(&format!("\x60\x60\x60json\n{value}\n\x60\x60\x60")).is_ok());
    assert_eq!(
        creation_notice("整理资料"),
        "我建了一个目标：整理资料，你可以在「目标」面板里看到并让我继续"
    );
    let names: Vec<_> = tool_definitions()
        .iter()
        .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, ["create_goal", "goal_status", "goal_advance"]);
}

#[tokio::test]
async fn visible_observer_sees_running_state_before_work_and_clear_after() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    let calls = AtomicUsize::new(0);
    fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |goal| async move { Ok(update(&goal)) },
            || {
                let index = calls.fetch_add(1, Ordering::SeqCst);
                assert_eq!(
                    fixture.store.snapshot().unwrap().running_id.is_some(),
                    index == 0
                );
            },
        )
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn goal_feature_has_no_system_persistence_calls() {
    for source in [
        include_str!("../goals.rs"),
        include_str!("../../commands/goals.rs"),
    ] {
        let source = source.to_lowercase();
        for forbidden in [
            "schtasks",
            "new-scheduledtask",
            "register-scheduledtask",
            "currentversion\\run",
            "shell:startup",
            "start-menu\\programs\\startup",
            "createservice",
            "new-service",
            "set_autostart",
            "winreg",
            "std::process::command",
        ] {
            assert!(
                !source.contains(forbidden),
                "目标功能不能使用系统持久化：{forbidden}"
            );
        }
    }
}

#[tokio::test]
async fn denied_goal_tools_never_reach_executor() {
    let cfg = crate::config::defaults::default_config();
    let perms = permissions(&cfg);
    let tools = build_tools(&crate::desktop::toolbox::list_agent_items(), &[], &perms);
    let runtime = TaskRuntime {
        base: String::new(),
        key: String::new(),
        model: String::new(),
        depth: 1,
        cfg,
        perms,
        dispatch: |_, _, _, _| Box::pin(async { panic!("未授权工具不能到达执行器") }),
    };
    for name in [
        "toolbox_agent_write_file",
        "toolbox_agent_shell",
        "forge_tool",
        "toolbox_agent_install",
    ] {
        let trace = super::super::sub_agents::TaskTrace::default();
        let result = super::super::loop_::execute_task_tool(
            None,
            name,
            &HashMap::new(),
            &tools,
            &runtime,
            &StreamEmitter::silent(),
            "goal-test-permission",
            false,
            Some(&trace),
        )
        .await;
        assert!(
            matches!(result, Err(AppError::PermissionDenied(_))),
            "{name}"
        );
        assert!(*trace.refused.lock().unwrap());
    }
}

#[tokio::test]
async fn malformed_injected_update_records_failure_without_unlock_leak() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    let report = fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |goal| async move {
                let mut result = update(&goal);
                result.steps[0].status = "invalid".into();
                Ok(result)
            },
            || {},
        )
        .await
        .unwrap();
    assert_eq!(report.goal.status, "paused");
    assert_eq!(report.goal.checkpoints[0].outcome, "failed");
    assert!(fixture.store.snapshot().unwrap().running_id.is_none());
}

#[tokio::test]
async fn non_yielding_work_cannot_report_success_after_deadline() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    let report = fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            Some(Duration::from_millis(1)),
            |goal| async move {
                std::thread::sleep(Duration::from_millis(10));
                let mut result = update(&goal);
                result.done = true;
                Ok(result)
            },
            || {},
        )
        .await
        .unwrap();
    assert_eq!(report.goal.status, "paused");
    assert_eq!(report.goal.checkpoints[0].outcome, "partial");
    assert!(report.message.contains("超时"));
}

#[tokio::test]
async fn elapsed_seconds_accumulate_across_advances() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    let first = fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            None,
            |goal| async move {
                tokio::time::sleep(Duration::from_millis(1050)).await;
                Ok(update(&goal))
            },
            || {},
        )
        .await
        .unwrap();
    assert!(first.goal.used.total_seconds >= 1);
    let second = once(&fixture.store, &goal).await;
    assert!(second.goal.used.total_seconds >= first.goal.used.total_seconds);
    assert_eq!(second.goal.used.runs, 2);
}

#[test]
fn creation_notice_survives_failed_followup_reply() {
    let notice = creation_notice("长期研究");
    let response =
        super::super::loop_::goal_notice_response(&StreamEmitter::silent(), 2, &[notice.clone()]);
    assert!(response.success);
    assert!(response.final_reply.unwrap().contains(&notice));
}

#[tokio::test]
async fn scheduler_defaults_to_five_minutes_and_busy_skip_waits_next_interval() {
    let fixture = Fixture::new();
    let goal = fixture.goal();
    assert_eq!(
        fixture.store.next_delay(100).unwrap(),
        Duration::from_secs(300)
    );
    auto(&fixture, &goal);
    assert_eq!(fixture.store.next_delay(0).unwrap(), Duration::from_secs(1));
    fixture
        .store
        .tick(
            10,
            true,
            None,
            |_| async { panic!("用户忙碌时不能推进") },
            || {},
        )
        .await
        .unwrap();
    assert!(fixture
        .store
        .tick(
            10,
            false,
            None,
            |_| async { panic!("跳过后应等待下一轮") },
            || {}
        )
        .await
        .unwrap()
        .is_none());
    let next = fixture
        .store
        .tick(
            11,
            false,
            None,
            |goal| async move { Ok(update(&goal)) },
            || {},
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next.goal.used.runs, 1);
    assert_eq!(next.goal.auto_runs, 1);
}

#[tokio::test]
async fn real_run_one_task_returns_goal_update_and_receives_resume_context() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let fixture = Fixture::new();
    let goal = fixture.goal();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let received = Arc::new(Mutex::new(Vec::<Value>::new()));
    let requests = received.clone();
    let reply = json!({"progress":"完成检索", "next_action":"整理报告", "steps":[{"text":"检索资料","status":"done"}], "outcome":"partial", "done":false, "blocked_reason":null});
    let body = json!({"choices":[{"message":{"role":"assistant","content":reply.to_string()}}]})
        .to_string();
    let server = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let size = socket.read(&mut chunk).await.unwrap();
                if size == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..size]);
                if let Some(index) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&buffer[..index]);
                    let length: usize = headers
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(|value| value.trim().parse().unwrap())
                        })
                        .unwrap_or(0);
                    if buffer.len() >= index + 4 + length {
                        requests.lock().unwrap().push(
                            serde_json::from_slice(&buffer[index + 4..index + 4 + length]).unwrap(),
                        );
                        break;
                    }
                }
            }
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            socket.write_all(response.as_bytes()).await.unwrap();
        }
    });
    let cfg = crate::config::defaults::default_config();
    let runtime = TaskRuntime {
        base: format!("http://{address}"),
        key: "mock-key".into(),
        model: "mock-model".into(),
        depth: 1,
        perms: permissions(&cfg),
        cfg,
        dispatch: |_, _, _, _| Box::pin(async { panic!("该回复不包含工具调用") }),
    };
    let report = fixture
        .store
        .advance(
            &goal.id,
            false,
            now(),
            Some(Duration::from_secs(5)),
            |goal| async move {
                let resume = context(&goal);
                let request = crate::commands::agent_run::AgentRunRequest {
                    task: resume.clone(),
                    request_id: "goal-runtime-test".into(),
                    max_steps: 3,
                    progress_events: false,
                };
                let response = run_one_task(
                    None,
                    request,
                    &runtime,
                    vec![],
                    vec![json!({"role":"user","content":resume})],
                    &StreamEmitter::silent(),
                    None,
                    false,
                    None,
                )
                .await?;
                parse_update(&response.final_reply.unwrap_or_default())
            },
            || {},
        )
        .await
        .unwrap();
    server.abort();
    assert_eq!(report.goal.status, "active");
    assert_eq!(report.goal.progress, "完成检索");
    assert_eq!(report.goal.next_action.as_deref(), Some("整理报告"));
    assert_eq!(report.goal.steps[0].status, "done");
    assert_eq!(report.goal.used.runs, 1);
    assert!(received
        .lock()
        .unwrap()
        .iter()
        .any(|request| request["messages"]
            .to_string()
            .contains("整理资料并撰写报告")));
}
