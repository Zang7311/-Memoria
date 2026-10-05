use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn authorized_write_still_cannot_touch_personality_configuration() {
    let target = crate::config::config_path().to_string_lossy().to_string();
    let input = json!({"path": target, "content": "不应写入"}).to_string();
    let mut api = mock_api(move |request| {
        if request["messages"].as_array().unwrap().iter().any(|message| message["role"] == "tool") {
            (200, final_reply("人格配置修改被拒绝"))
        } else {
            (200, json!({"choices": [{"message": {"role": "assistant", "tool_calls": [{"id": "write", "type": "function", "function": {"name": "toolbox_agent_write_file", "arguments": json!({"input": input}).to_string()}}]}}]}))
        }
    }).await;
    api.runtime.perms.allow_file_write = true;
    api.runtime.dispatch =
        |_, _, _, _| Box::pin(async { panic!("保护路径不得到达工具执行器") });
    let args =
        serde_json::from_value(json!({"allow_write": true, "tasks": [{"goal": "修改配置"}]}))
            .unwrap();
    let result = spawn_sub_agents(
        None,
        &args,
        &parent_tools(&api.runtime.perms),
        &api.runtime,
        &StreamEmitter::silent(),
        "protected-write",
    )
    .await
    .unwrap();
    assert!(result.contains("状态：被拒"));
    let requests = api.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1]["messages"].to_string().contains("不得修改人格"));
}

#[tokio::test]
async fn failed_tool_execution_is_a_failed_child_not_a_parent_error() {
    let mut api = mock_api(|request| {
        if request["messages"].as_array().unwrap().iter().any(|message| message["role"] == "tool") {
            (200, final_reply("文件无法读取"))
        } else if request["messages"][1]["content"].as_str().unwrap().contains("失败") {
            (200, json!({"choices": [{"message": {"role": "assistant", "tool_calls": [{"id": "read", "type": "function", "function": {"name": "toolbox_agent_read_file", "arguments": "{}"}}]}}]}))
        } else { (200, final_reply("独立成功")) }
    }).await;
    api.runtime.dispatch =
        |_, _, _, _| Box::pin(async { Err(AppError::ToolboxError("文件读取失败".into())) });
    let args =
        serde_json::from_value(json!({"tasks": [{"goal": "失败任务"}, {"goal": "成功任务"}]}))
            .unwrap();
    let result = spawn_sub_agents(
        None,
        &args,
        &parent_tools(&api.runtime.perms),
        &api.runtime,
        &StreamEmitter::silent(),
        "failed-tool",
    )
    .await
    .unwrap();
    assert!(result.contains("[子任务 1] 状态：失败"));
    assert!(result.contains("[子任务 2] 状态：完成\n独立成功"));
}

#[tokio::test]
async fn credentials_are_redacted_before_summary_truncation() {
    let mut api = mock_api(|_| {
        (
            200,
            final_reply(&format!("{}other-model-secret", "猫".repeat(797))),
        )
    })
    .await;
    api.runtime.cfg.api_key_plain = Some("other-model-secret".into());
    let args = serde_json::from_value(json!({"tasks": [{"goal": "任务"}]})).unwrap();
    let output = spawn_sub_agents(
        None,
        &args,
        &parent_tools(&api.runtime.perms),
        &api.runtime,
        &StreamEmitter::silent(),
        "secret-boundary",
    )
    .await
    .unwrap();
    assert!(!output.contains("other"));
}

fn task(goal: &str) -> SubTask {
    SubTask {
        goal: goal.into(),
        context: String::new(),
        truncated: false,
    }
}

#[derive(Default)]
struct StreamCapture {
    chunks: Mutex<Vec<String>>,
    ends: AtomicUsize,
    events: Mutex<Vec<(String, SubAgentEvent)>>,
}

struct RecordingSink(Arc<StreamCapture>);

impl super::super::stream_progress::StreamSink for RecordingSink {
    fn chunk(&self, text: &str) {
        self.0.chunks.lock().unwrap().push(text.into());
    }
    fn end(&self) {
        self.0.ends.fetch_add(1, Ordering::SeqCst);
    }
    fn sub_agent(&self, event: &str, payload: &SubAgentEvent) {
        self.0
            .events
            .lock()
            .unwrap()
            .push((event.into(), payload.clone()));
    }
}

fn recording_emitter(progress_events: bool) -> (StreamEmitter, Arc<StreamCapture>) {
    let capture = Arc::new(StreamCapture::default());
    (
        StreamEmitter::with_sink(Box::new(RecordingSink(capture.clone())), progress_events),
        capture,
    )
}

#[test]
fn existing_parent_stream_chunks_and_end_events_remain_exact() {
    let (emitter, capture) = recording_emitter(true);
    emitter.push_progress("检查结果");
    emitter.push_final_reply("猫".repeat(17).as_str());
    assert_eq!(
        *capture.chunks.lock().unwrap(),
        vec![
            "\n\n🔧 检查结果\n".to_string(),
            "猫".repeat(16),
            "猫".to_string()
        ]
    );
    assert_eq!(capture.ends.load(Ordering::SeqCst), 2);
    assert!(capture.events.lock().unwrap().is_empty());
}

#[test]
fn delegation_progress_is_plain_text_and_never_ends_the_parent_stream() {
    let (emitter, capture) = recording_emitter(true);
    emitter.push_delegation_progress(3);
    assert_eq!(
        *capture.chunks.lock().unwrap(),
        vec!["\n\n派出 3 个子 Agent 并行处理…\n"]
    );
    assert_eq!(capture.ends.load(Ordering::SeqCst), 0);
}

#[test]
fn disabled_progress_emits_only_the_final_reply() {
    let (emitter, capture) = recording_emitter(false);
    emitter.push_progress("不显示");
    emitter.push_delegation_progress(2);
    emitter.push_sub_agent(
        "sub_agent_started",
        &SubAgentEvent {
            request_id: "parent".into(),
            sub_id: "child".into(),
            goal: "任务".into(),
            status: "处理中".into(),
            summary: String::new(),
        },
    );
    emitter.push_final_reply("最终回复");
    assert_eq!(*capture.chunks.lock().unwrap(), vec!["最终回复"]);
    assert_eq!(capture.ends.load(Ordering::SeqCst), 1);
    assert!(capture.events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn actual_child_events_are_bounded_redacted_and_do_not_interleave_contexts() {
    let raw = "原始文件资料".repeat(170);
    let model_raw = raw.clone();
    let mut api = mock_api(move |request| {
        if request["messages"].as_array().unwrap().iter().any(|message| message["role"] == "tool") {
            (200, final_reply(&format!("分析结论\n{model_raw}\ntest-private-key\nother-model-secret")))
        } else {
            (200, json!({"choices": [{"message": {"role": "assistant", "content": null, "tool_calls": [{"id": "read", "type": "function", "function": {"name": "toolbox_agent_read_file", "arguments": "{}"}}]}}]}))
        }
    }).await;
    api.runtime.cfg.api_key_plain = Some("other-model-secret".into());
    api.runtime.dispatch = |_, name, _, child| {
        assert!(child);
        assert_eq!(name, "toolbox_agent_read_file");
        Box::pin(async { Ok("原始文件资料".repeat(170)) })
    };
    let (emitter, capture) = recording_emitter(true);
    let args = serde_json::from_value(json!({"tasks": [{"goal": format!("{} test-private-key", "目标".repeat(80)), "context": "不能出现在事件中的独立上下文"}]})).unwrap();
    spawn_sub_agents(
        None,
        &args,
        &parent_tools(&api.runtime.perms),
        &api.runtime,
        &emitter,
        "event-parent",
    )
    .await
    .unwrap();
    let events = capture.events.lock().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].0, "sub_agent_started");
    assert_eq!(events[1].0, "sub_agent_finished");
    assert_eq!(events[0].1.sub_id, events[1].1.sub_id);
    assert_eq!(events[1].1.request_id, "event-parent");
    assert_eq!(events[1].1.status, "完成");
    assert!(events[1].1.summary.contains("分析结论"));
    for (_, payload) in events.iter() {
        assert!(payload.goal.chars().count() <= 80);
        assert!(payload.summary.chars().count() <= 800);
        let serialized = serde_json::to_string(payload).unwrap();
        for forbidden in [
            raw.as_str(),
            "原始文件资料",
            "不能出现在事件中的独立上下文",
            "test-private-key",
            "other-model-secret",
        ] {
            assert!(!serialized.contains(forbidden), "事件不能含有 {forbidden}");
        }
    }
    assert_eq!(capture.ends.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn failed_child_events_are_emitted_without_api_error_bodies() {
    let api = mock_api(|_| (500, json!({"error": "原始文件内容 test-private-key"}))).await;
    let (emitter, capture) = recording_emitter(true);
    let args = serde_json::from_value(json!({"tasks": [{"goal": "任务"}]})).unwrap();
    let output = spawn_sub_agents(
        None,
        &args,
        &parent_tools(&api.runtime.perms),
        &api.runtime,
        &emitter,
        "event-failure",
    )
    .await
    .unwrap();
    assert!(output.contains("状态：失败"));
    let events = capture.events.lock().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].1.status, "失败");
    assert!(!events[1].1.summary.contains("原始文件内容"));
    assert!(!events[1].1.summary.contains("test-private-key"));
}

#[tokio::test]
async fn child_cancellation_shares_parent_flag_without_registering_or_resetting_it() {
    let api = mock_api(|_| (200, final_reply("不应调用模型"))).await;
    let request_id = "cancel-child-shared-flag";
    let _guard = crate::agent::cancel::CancelGuard::new(request_id);
    crate::agent::cancel::set_cancelled(request_id);
    let args =
        serde_json::from_value(json!({"tasks": [{"goal": "任务一"}, {"goal": "任务二"}]})).unwrap();
    let output = spawn_sub_agents(
        None,
        &args,
        &parent_tools(&api.runtime.perms),
        &api.runtime,
        &StreamEmitter::silent(),
        request_id,
    )
    .await
    .unwrap();
    assert_eq!(output.matches("状态：被拒").count(), 2);
    assert!(crate::agent::cancel::is_cancelled(request_id));
    assert!(api.requests.lock().unwrap().is_empty());
}

fn parent_tools(perms: &AgentPermissions) -> Vec<Value> {
    let presets: Vec<crate::types::ToolboxItem> =
        serde_json::from_str(include_str!("../../../resources/agent_tools.json")).unwrap();
    build_tools(&presets, &[], perms)
}

fn names(tools: &[Value]) -> Vec<&str> {
    tools
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect()
}

fn completed(summary: &str) -> SubResult {
    SubResult {
        status: "完成".into(),
        summary: summary.into(),
    }
}

#[test]
fn tool_is_registered_with_structured_schema() {
    let tools = parent_tools(&AgentPermissions::default());
    let definition = tools
        .iter()
        .find(|tool| tool["function"]["name"] == "spawn_sub_agents")
        .unwrap();
    assert_eq!(
        definition["function"]["parameters"]["properties"]["tasks"]["maxItems"],
        4
    );
    assert_eq!(
        definition["function"]["parameters"]["properties"]["allow_write"]["default"],
        false
    );
}

#[test]
fn default_tools_are_strictly_read_only_and_never_nested() {
    let perms = AgentPermissions {
        allow_download: true,
        allow_software: true,
        allow_file_write: true,
        allow_shell: true,
    };
    let tools = child_tools(&parent_tools(&perms), &perms, false);
    assert!(!tools.is_empty());
    assert!(names(&tools).iter().all(|name| is_read_only_tool(name)));
    for blocked in [
        "spawn_sub_agents",
        "toolbox_agent_write_file",
        "toolbox_agent_run_command",
        "toolbox_agent_download",
        "toolbox_agent_winget_install",
        "toolbox_agent_remember",
        "toolbox_agent_shortcut",
    ] {
        assert!(!names(&tools).contains(&blocked), "不得提供 {blocked}");
    }
}

#[test]
fn writes_require_both_request_and_parent_authorization() {
    let denied = AgentPermissions::default();
    let allowed = AgentPermissions {
        allow_file_write: true,
        ..denied
    };
    let parent = parent_tools(&allowed);
    for tools in [
        child_tools(&parent, &allowed, false),
        child_tools(&parent, &denied, true),
        child_tools(&parent_tools(&denied), &allowed, true),
    ] {
        assert!(!names(&tools).contains(&"toolbox_agent_write_file"));
    }
    let tools = child_tools(&parent, &allowed, true);
    assert!(names(&tools).contains(&"toolbox_agent_write_file"));
    assert!(!names(&tools).contains(&"toolbox_agent_download"));
    assert!(!names(&tools).contains(&"spawn_sub_agents"));
    let download = AgentPermissions {
        allow_download: true,
        ..denied
    };
    assert!(
        names(&child_tools(&parent_tools(&download), &download, true))
            .contains(&"toolbox_agent_download")
    );
}

#[test]
fn unsafe_custom_tools_cannot_impersonate_read_only_presets() {
    let perms = AgentPermissions::default();
    let mut parent = parent_tools(&perms);
    let tool = parent
        .iter_mut()
        .find(|tool| tool["function"]["name"] == "toolbox_agent_read_file")
        .unwrap();
    tool["function"]["description"] = json!("执行自定义脚本");
    parent.push(json!({"function": {"name": "skill_plugin__write"}}));
    let tools = child_tools(&parent, &perms, true);
    assert!(!names(&tools).contains(&"toolbox_agent_read_file"));
    assert!(!names(&tools).contains(&"skill_plugin__write"));
}

#[test]
fn task_limits_and_unicode_truncation_are_explicit() {
    let args = serde_json::from_value(
        json!({"tasks": [{"goal": "铃".repeat(501), "context": "猫".repeat(4001)}]}),
    )
    .unwrap();
    let (tasks, allow_write) = parse_tasks(&args).unwrap();
    assert!(!allow_write);
    assert!(tasks[0].truncated);
    assert_eq!(tasks[0].goal.chars().count(), 500);
    assert_eq!(tasks[0].context.chars().count(), 4000);
    let too_many =
        serde_json::from_value(json!({"tasks": vec![json!({"goal": "任务"}); 5]})).unwrap();
    let error = parse_tasks(&too_many).unwrap_err().to_string();
    assert!(error.contains("最多") && error.contains('4') && error.contains("分批"));
}

#[test]
fn malformed_tasks_are_rejected_in_chinese() {
    for value in [
        json!({}),
        json!({"tasks": []}),
        json!({"tasks": [{"goal": " "}]}),
        json!({"tasks": [{"goal": "任务", "context": 123}]}),
        json!({"tasks": [{"goal": "任务"}], "allow_write": "true"}),
    ] {
        let args = serde_json::from_value(value).unwrap();
        let error = parse_tasks(&args).unwrap_err().to_string();
        assert!(error
            .chars()
            .any(|character| ('\u{4e00}'..='\u{9fff}').contains(&character)));
    }
}

#[test]
fn child_context_contains_only_its_goal_and_context() {
    let task = SubTask {
        context: "独立背景资料".into(),
        ..task("独立目标")
    };
    let messages = child_messages(&task, &[]);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(
        messages[1]["content"],
        "目标：独立目标\n\n背景：独立背景资料"
    );
    assert!(!serde_json::to_string(&messages)
        .unwrap()
        .contains("主对话历史中的密文"));
}

#[test]
fn unavailable_tools_are_rejected_in_chinese() {
    let args = HashMap::new();
    for name in [
        "spawn_sub_agents",
        "toolbox_agent_write_file",
        "skill_plugin__modify_persona",
        "未知工具",
    ] {
        let error = check_tool_call(name, &args, &[]).unwrap_err().to_string();
        assert!(error.contains("子 Agent") && error.contains("拒绝"));
    }
}

#[test]
fn protected_paths_and_aliases_cannot_be_written() {
    let directory = std::env::temp_dir().join(format!("mem-child-safety-{}", uuid::Uuid::new_v4()));
    let protected = directory.join("protected");
    let outside = directory.join("outside");
    std::fs::create_dir_all(&protected).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    assert!(!safe_write_path(
        &protected.join("config.json").to_string_lossy(),
        &[protected.clone()]
    ));
    assert!(!safe_write_path(
        &directory.to_string_lossy(),
        &[protected.clone()]
    ));
    assert!(!safe_write_path(
        &outside.join("..\\protected\\config.json").to_string_lossy(),
        &[protected.clone()]
    ));
    assert!(!safe_write_path(
        "relative/config.json",
        &[protected.clone()]
    ));
    assert!(safe_write_path(
        &outside.join("note.txt").to_string_lossy(),
        &[protected.clone()]
    ));
    #[cfg(windows)]
    {
        let alias = outside.join("alias");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&alias)
            .arg(&protected)
            .output()
            .unwrap();
        assert!(status.status.success());
        assert!(!safe_write_path(
            &alias.join("config.json").to_string_lossy(),
            &[protected.clone()]
        ));
        std::fs::remove_dir(alias).unwrap();
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn public_events_hide_keys_context_and_raw_file_contents() {
    let task = SubTask {
        context: "机密上下文原文".into(),
        ..task("分析文件")
    };
    let trace = TaskTrace::default();
    trace
        .sources
        .lock()
        .unwrap()
        .push("文件第一行\n文件第二行".into());
    let summary = public_summary("已分析：\n文件第一行\n文件第二行\n机密上下文原文\n私有密钥-123\nAuthorization: Bearer other-secret\n结论正确", &task, &trace, "私有密钥-123");
    for secret in [
        "文件第一行",
        "文件第二行",
        "机密上下文原文",
        "私有密钥-123",
        "other-secret",
    ] {
        assert!(!summary.contains(secret));
    }
    assert!(summary.contains("结论正确"));
    assert_eq!(truncate(&"猫".repeat(801), 800).chars().count(), 800);
}

#[tokio::test]
async fn dispatch_collects_two_results_concurrently_in_input_order() {
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let notifications = Mutex::new(Vec::new());
    let results = tokio::time::timeout(
        Duration::from_secs(5),
        collect_tasks(
            vec![task("第一任务"), task("第二任务")],
            Duration::from_secs(2),
            |_, task| {
                let barrier = barrier.clone();
                async move {
                    barrier.wait().await;
                    Ok(completed(&task.goal))
                }
            },
            |index, _, result| {
                notifications
                    .lock()
                    .unwrap()
                    .push((index, result.status.clone()))
            },
        ),
    )
    .await
    .expect("两个任务必须并行运行");
    assert_eq!(notifications.lock().unwrap().len(), 2);
    let summary = format_results(&results);
    assert!(summary.contains("[子任务 1] 状态：完成\n第一任务"));
    assert!(summary.contains("[子任务 2] 状态：完成\n第二任务"));
}

#[tokio::test]
async fn failure_is_isolated_and_does_not_escape_to_parent() {
    let results = collect_tasks(
        vec![task("失败任务"), task("成功任务")],
        Duration::from_secs(2),
        |index, _| async move {
            if index == 0 {
                Err(AppError::InternalError("秘密文件内容不应泄露".into()))
            } else {
                Ok(completed("成功结果"))
            }
        },
        |_, _, _| {},
    )
    .await;
    assert_eq!(results[0].status, "失败");
    assert_eq!(results[1].status, "完成");
    let summary = format_results(&results);
    assert!(summary.contains("成功结果"));
    assert!(!summary.contains("秘密文件内容"));
}

#[tokio::test]
async fn timeout_is_isolated_and_notified_immediately() {
    let notifications = Mutex::new(Vec::new());
    let results = collect_tasks(
        vec![task("超时任务"), task("成功任务")],
        Duration::from_millis(30),
        |index, _| async move {
            if index == 0 {
                std::future::pending::<()>().await;
            }
            Ok(completed("及时完成"))
        },
        |index, _, result| {
            notifications
                .lock()
                .unwrap()
                .push((index, result.status.clone()))
        },
    )
    .await;
    assert_eq!(results[0].status, "超时");
    assert_eq!(results[1].status, "完成");
    assert_eq!(notifications.lock().unwrap()[0], (1, "完成".into()));
}

#[tokio::test]
async fn concurrency_is_globally_bounded_across_batches() {
    let active = AtomicUsize::new(0);
    let peak = AtomicUsize::new(0);
    let run = |_, _: SubTask| async {
        let count = active.fetch_add(1, Ordering::SeqCst) + 1;
        peak.fetch_max(count, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(10)).await;
        active.fetch_sub(1, Ordering::SeqCst);
        Ok(completed("结果"))
    };
    let tasks = vec![task("任务"); 4];
    let (first, second) = tokio::join!(
        collect_tasks(tasks.clone(), Duration::from_secs(2), run, |_, _, _| {}),
        collect_tasks(tasks, Duration::from_secs(2), run, |_, _, _| {})
    );
    assert_eq!(first.len() + second.len(), 8);
    assert!(peak.load(Ordering::SeqCst) > 1);
    assert!(peak.load(Ordering::SeqCst) <= 4);
    assert_eq!(active.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn truncation_notice_is_kept_even_on_failure() {
    let mut task = task("长任务");
    task.truncated = true;
    let results = collect_tasks(
        vec![task],
        Duration::from_secs(1),
        |_, _| async { Err(AppError::InternalError("失败".into())) },
        |_, _, _| {},
    )
    .await;
    assert!(results[0].summary.contains("已截断"));
}

struct MockApi {
    runtime: TaskRuntime,
    requests: Arc<Mutex<Vec<Value>>>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for MockApi {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn mock_api(handler: impl Fn(&Value) -> (u16, Value) + Send + Sync + 'static) -> MockApi {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let server = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 4096];
            let (header_end, length) = loop {
                let read = socket.read(&mut buffer).await.unwrap();
                if read == 0 {
                    return;
                }
                bytes.extend_from_slice(&buffer[..read]);
                if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length:")
                                .and_then(|length| length.trim().parse::<usize>().ok())
                        })
                        .unwrap();
                    break (end + 4, length);
                }
            };
            while bytes.len() < header_end + length {
                let read = socket.read(&mut buffer).await.unwrap();
                if read == 0 {
                    return;
                }
                bytes.extend_from_slice(&buffer[..read]);
            }
            let request = serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap();
            let (status, response) = handler(&request);
            captured.lock().unwrap().push(request);
            let body = response.to_string();
            socket.write_all(format!("HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        }
    });
    let runtime = TaskRuntime {
        base: format!("http://{address}"),
        key: "test-private-key".into(),
        model: "mock-model".into(),
        depth: 2,
        cfg: crate::config::defaults::default_config(),
        perms: AgentPermissions::default(),
        dispatch: |_, _, _, _| {
            Box::pin(async {
                Err(AppError::ToolboxError("测试环境未提供工具执行器".into()))
            })
        },
    };
    MockApi {
        runtime,
        requests,
        server,
    }
}

fn final_reply(text: &str) -> Value {
    json!({"choices": [{"message": {"role": "assistant", "content": text}}]})
}

fn forbidden_call() -> Value {
    json!({"choices": [{"message": {"role": "assistant", "content": null, "tool_calls": [{"id": "call-denied", "type": "function", "function": {"name": "spawn_sub_agents", "arguments": "{}"}}]}}]})
}

#[tokio::test]
async fn real_shared_loop_dispatches_isolated_tasks_and_aggregates_results() {
    let api = mock_api(|request| {
        let goal = request["messages"][1]["content"].as_str().unwrap();
        (
            200,
            final_reply(if goal.contains("第一任务") {
                "第一结果"
            } else {
                "第二结果"
            }),
        )
    })
    .await;
    let args = serde_json::from_value(json!({"tasks": [{"goal": "第一任务", "context": "独立资料一"}, {"goal": "第二任务", "context": "独立资料二"}]})).unwrap();
    let output = spawn_sub_agents(
        None,
        &args,
        &parent_tools(&api.runtime.perms),
        &api.runtime,
        &StreamEmitter::silent(),
        "parent-test",
    )
    .await
    .unwrap();
    assert!(output.contains("[子任务 1] 状态：完成\n第一结果"));
    assert!(output.contains("[子任务 2] 状态：完成\n第二结果"));
    let requests = api.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for request in requests.iter() {
        assert_eq!(request["messages"].as_array().unwrap().len(), 2);
        assert!(!names(request["tools"].as_array().unwrap()).contains(&"spawn_sub_agents"));
        assert!(!request.to_string().contains("主对话历史中的密文"));
    }
}

#[tokio::test]
async fn real_shared_loop_failure_still_returns_successful_sibling() {
    let api = mock_api(|request| {
        if request["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("失败")
        {
            (500, json!({"error": "test-private-key 敏感文件内容"}))
        } else {
            (200, final_reply("成功结果"))
        }
    })
    .await;
    let args =
        serde_json::from_value(json!({"tasks": [{"goal": "失败任务"}, {"goal": "成功任务"}]}))
            .unwrap();
    let output = spawn_sub_agents(
        None,
        &args,
        &parent_tools(&api.runtime.perms),
        &api.runtime,
        &StreamEmitter::silent(),
        "parent-failure",
    )
    .await
    .unwrap();
    assert!(output.contains("状态：失败"));
    assert!(output.contains("状态：完成\n成功结果"));
    assert!(!output.contains("test-private-key"));
    assert!(!output.contains("敏感文件内容"));
}

#[tokio::test]
async fn real_shared_loop_rejects_hallucinated_nested_tools() {
    let api = mock_api(|request| {
        let messages = request["messages"].as_array().unwrap();
        if messages.iter().any(|message| message["role"] == "tool") {
            (200, final_reply("未执行未经授权的工具"))
        } else {
            (200, forbidden_call())
        }
    })
    .await;
    let args = serde_json::from_value(json!({"tasks": [{"goal": "执行任务"}]})).unwrap();
    let output = spawn_sub_agents(
        None,
        &args,
        &parent_tools(&api.runtime.perms),
        &api.runtime,
        &StreamEmitter::silent(),
        "parent-denied",
    )
    .await
    .unwrap();
    assert!(output.contains("状态：被拒"));
    let requests = api.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let denied = requests[1]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|message| message["role"] == "tool")
        .unwrap();
    assert!(denied["content"].as_str().unwrap().contains("拒绝"));
}

#[tokio::test]
async fn real_shared_loop_stops_at_fifteen_steps_and_returns_progress() {
    let api = mock_api(|request| {
        if request.get("tools").is_some() {
            (200, forbidden_call())
        } else {
            (200, final_reply("已尝试十五步，均未执行未经授权操作"))
        }
    })
    .await;
    let args = serde_json::from_value(json!({"tasks": [{"goal": "持续任务"}]})).unwrap();
    let output = spawn_sub_agents(
        None,
        &args,
        &parent_tools(&api.runtime.perms),
        &api.runtime,
        &StreamEmitter::silent(),
        "parent-budget",
    )
    .await
    .unwrap();
    assert!(output.contains("状态：步数用尽"));
    assert!(output.contains("已尝试十五步"));
    let requests = api.requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.get("tools").is_some())
            .count(),
        15
    );
    assert_eq!(requests.len(), 16);
}

#[tokio::test]
async fn four_full_summaries_and_plain_text_output_are_not_lost() {
    let api = mock_api(|_| {
        (
            200,
            final_reply(&format!("<script>不执行</script>{}", "结果".repeat(800))),
        )
    })
    .await;
    let args = serde_json::from_value(json!({"tasks": vec![json!({"goal": "任务"}); 4]})).unwrap();
    let output = spawn_sub_agents(
        None,
        &args,
        &parent_tools(&api.runtime.perms),
        &api.runtime,
        &StreamEmitter::silent(),
        "parent-four",
    )
    .await
    .unwrap();
    assert!(output.contains("[子任务 4] 状态：完成"));
    assert!(output.chars().count() > 3000);
    for result in output.split("\n\n") {
        let summary = result.split_once('\n').unwrap().1;
        assert!(summary.chars().count() <= 800);
        assert!(summary.starts_with("<script>不执行</script>"));
    }
}
