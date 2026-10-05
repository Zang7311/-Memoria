use super::*;
use serde_json::json;

struct Fixture {
    store: Arc<AuditStore>,
}
impl Fixture {
    fn new(max_entries: usize, max_file_bytes: u64) -> Self {
        let dir = std::env::temp_dir().join(format!("mem-audit-unit-{}", uuid::Uuid::new_v4()));
        Self {
            store: Arc::new(AuditStore::new(
                dir,
                AuditConfig {
                    max_entries,
                    max_file_bytes,
                },
            )),
        }
    }
    fn list(&self) -> Vec<AuditEntry> {
        self.store.list(&AuditQuery::default()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self.store.dir.is_dir() {
            let _ = fs::remove_dir_all(&self.store.dir);
        } else {
            let _ = fs::remove_file(&self.store.dir);
        }
    }
}
fn entry(id: usize) -> AuditEntry {
    AuditEntry {
        id: id.to_string(),
        at: id as i64,
        kind: "agent".into(),
        title: format!("整理中文资料{id}"),
        request_id: Some("父请求".into()),
        session_id: Some("会话".into()),
        goal_id: None,
        tools: Vec::new(),
        files_changed: vec!["资料/报告.txt".into()],
        result: "ok".into(),
        summary: "整理完成".into(),
        duration_ms: 12,
    }
}
fn args(value: Value) -> HashMap<String, Value> {
    value.as_object().unwrap().clone().into_iter().collect()
}
fn run(fixture: &Fixture, kind: &str) -> AuditRun {
    AuditRun::with_store(
        kind,
        "整理资料",
        Some("父请求"),
        Some("会话"),
        (kind == "goal").then_some("目标一"),
        fixture.store.clone(),
    )
}

#[test]
fn 中文字段写读往返且追加不改旧字节() {
    let fixture = Fixture::new(2000, 5 * 1024 * 1024);
    fixture.store.append(&entry(1)).unwrap();
    let before = fs::read(fixture.store.dir.join("audit.jsonl")).unwrap();
    fixture.store.append(&entry(2)).unwrap();
    let after = fs::read(fixture.store.dir.join("audit.jsonl")).unwrap();
    assert_eq!(
        fs::read(fixture.store.dir.join("audit-1.jsonl")).unwrap(),
        before
    );
    assert_eq!(String::from_utf8(after).unwrap().lines().count(), 1);
    let entries = fixture.list();
    assert_eq!(entries[0].title, "整理中文资料2");
    assert_eq!(entries[1].session_id.as_deref(), Some("会话"));
    assert_eq!(entries[1].files_changed, ["资料/报告.txt"]);
}

#[test]
fn 保留最新条目且重启后仍应用上限() {
    let fixture = Fixture::new(3, 5 * 1024 * 1024);
    for id in 0..7 {
        fixture.store.append(&entry(id)).unwrap();
    }
    assert_eq!(fixture.store.segments().unwrap().len(), 3);
    let physical_count: usize = fixture
        .store
        .segments()
        .unwrap()
        .iter()
        .map(|path| fs::read_to_string(path).unwrap().lines().count())
        .sum();
    assert_eq!(physical_count, 3);
    assert_eq!(
        fixture
            .list()
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        ["6", "5", "4"]
    );
    let reopened = AuditStore::new(fixture.store.dir.clone(), fixture.store.config.clone());
    assert_eq!(reopened.list(&AuditQuery::default()).unwrap().len(), 3);
    assert!(reopened
        .list(&AuditQuery {
            keyword: Some("资料0".into()),
            ..Default::default()
        })
        .unwrap()
        .is_empty());
}

#[test]
fn 超过大小即时归档且归档参与读取() {
    let fixture = Fixture::new(3, 1);
    fixture.store.append(&entry(1)).unwrap();
    assert!(fixture.store.dir.join("audit-1.jsonl").exists());
    let before = fs::read(fixture.store.dir.join("audit-1.jsonl")).unwrap();
    fixture.store.append(&entry(2)).unwrap();
    assert_eq!(
        fs::read(fixture.store.dir.join("audit-1.jsonl")).unwrap(),
        before
    );
    assert_eq!(fixture.list().len(), 2);
    for id in 3..6 {
        fixture.store.append(&entry(id)).unwrap();
    }
    assert_eq!(fixture.store.segments().unwrap().len(), 3);
    assert!(!fixture.store.dir.join("audit-1.jsonl").exists());
    assert_eq!(fixture.list()[0].id, "5");
}

#[test]
fn 检索匹配标题工具路径简报且组合筛选() {
    let fixture = Fixture::new(10, 10000);
    let mut first = entry(100);
    first.tools.push(ToolCallRecord {
        name: "toolbox_agent_read_file".into(),
        intent: Some("file.read".into()),
        ok: true,
        brief: "读取测试配置路径".into(),
        at: 100,
    });
    fixture.store.append(&first).unwrap();
    let mut second = entry(200);
    second.kind = "chat".into();
    second.result = "failed".into();
    fixture.store.append(&second).unwrap();
    for word in ["中文", "READ_FILE", "报告.txt", "测试配置"] {
        assert!(!fixture
            .store
            .list(&AuditQuery {
                keyword: Some(word.into()),
                ..Default::default()
            })
            .unwrap()
            .is_empty());
    }
    let query = AuditQuery {
        keyword: Some("资料".into()),
        kind: Some("agent".into()),
        result: Some("ok".into()),
        since: Some(100),
        until: Some(100),
    };
    assert_eq!(fixture.store.list(&query).unwrap().len(), 1);
    assert!(fixture
        .store
        .list(&AuditQuery {
            since: Some(201),
            ..Default::default()
        })
        .unwrap()
        .is_empty());
    assert!(fixture
        .store
        .list(&AuditQuery {
            result: Some("blocked".into()),
            ..Default::default()
        })
        .unwrap()
        .is_empty());
}

#[test]
fn 提取写删移动压缩解压及真实工具箱输入() {
    for name in [
        "file_write",
        "file_delete",
        "toolbox_agent_write_file",
        "toolbox_agent_delete_file",
    ] {
        assert_eq!(
            changed_files(
                name,
                &args(json!({"target":"资料.txt","content":"不得记录的正文"}))
            ),
            ["资料.txt"]
        );
    }
    assert_eq!(
        changed_files(
            "toolbox_agent_write_file",
            &args(json!({"input":"{\"path\":\"资料.txt\",\"content\":\"正文\"}"}))
        ),
        ["资料.txt"]
    );
    assert_eq!(
        changed_files(
            "toolbox_agent_delete_file",
            &args(json!({"input":"资料.txt"}))
        ),
        ["资料.txt"]
    );
    assert_eq!(
        changed_files(
            "file_move",
            &args(json!({"source":"旧.txt","target":"新.txt"}))
        )
        .len(),
        2
    );
    for name in [
        "zip",
        "unzip",
        "toolbox_agent_compress",
        "toolbox_agent_extract",
    ] {
        assert_eq!(
            changed_files(name, &args(json!({"target":"输出路径"}))),
            ["输出路径"]
        );
    }
    assert_eq!(
        changed_files(
            "toolbox_agent_compress",
            &args(json!({"source":"报告.txt"}))
        ),
        ["报告.zip"]
    );
    assert_eq!(
        changed_files(
            "toolbox_agent_extract",
            &args(json!({"archive":"报告.zip"}))
        ),
        ["报告"]
    );
    assert!(changed_files("toolbox_agent_read_file", &args(json!({"path":"报告.txt"}))).is_empty());
    assert!(changed_files("shell", &args(json!({"command":"写文件内容"}))).is_empty());
}

#[tokio::test]
async fn 多次工具调用聚合成一条且失败不误报改动() {
    let fixture = Fixture::new(10, 10000);
    let audit = run(&fixture, "agent");
    audit
        .scope(async {
            let tools = vec![json!({"function":{"name":"file_write"},"x-intent":"file.write"})];
            for (path, ok) in [
                ("资料.txt", true),
                ("资料.txt", true),
                ("未写入.txt", false),
            ] {
                let mut tool =
                    ToolAttempt::new("file_write", &args(json!({"target":path})), &tools);
                tool.finish(ok);
            }
        })
        .await;
    audit.finish("ok");
    drop(audit);
    let entries = fixture.list();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].tools.len(), 3);
    assert_eq!(entries[0].tools[0].intent.as_deref(), Some("file.write"));
    assert_eq!(entries[0].files_changed, ["资料.txt"]);
    assert_eq!(entries[0].result, "partial");
}

#[tokio::test]
async fn 参数含密钥密码文件网页正文均不进入记录() {
    let fixture = Fixture::new(10, 10000);
    let audit = run(&fixture, "agent");
    audit.scope(async {
        let mut tool = ToolAttempt::new("toolbox_agent_write_file", &args(json!({"input":json!({"path":"资料.txt","content":"文件原文绝不可外泄","api_key":"unknown-short-key","password":"不能记录的密码","web_body":"网页抓取原文"}).to_string()})), &[]);
        tool.finish(true);
        let mut shell = ToolAttempt::new("toolbox_agent_shell", &args(json!({"command":"curl -H 'Authorization: secret-command-key'"})), &[]);
        shell.finish(true);
    }).await;
    audit.finish("ok");
    drop(audit);
    let text = fs::read_to_string(fixture.store.dir.join("audit.jsonl")).unwrap();
    for secret in [
        "unknown-short-key",
        "不能记录的密码",
        "文件原文绝不可外泄",
        "网页抓取原文",
        "secret-command-key",
        "curl -H",
    ] {
        assert!(!text.contains(secret));
    }
    assert!(text.contains("资料.txt"));
    for tool in &fixture.list()[0].tools {
        assert!(tool.brief.chars().count() <= 100);
    }
}

#[tokio::test]
async fn 调用密钥即使混入标题路径也会隐藏() {
    let fixture = Fixture::new(10, 10000);
    let secret = "short-unknown-key";
    let audit = AuditRun::with_store(
        "agent",
        &format!("处理 {secret}"),
        None,
        None,
        None,
        fixture.store.clone(),
    );
    audit
        .scope(async {
            let mut attempt = ToolAttempt::new(
                "file_write",
                &args(json!({"target":format!("目录/{secret}.txt"),"api_key":secret})),
                &[],
            );
            attempt.finish(true);
        })
        .await;
    audit.finish("ok");
    drop(audit);
    assert!(!fs::read_to_string(fixture.store.dir.join("audit.jsonl"))
        .unwrap()
        .contains(secret));
}

#[test]
fn 标题按字符截断且隐藏密钥人格和多行正文() {
    assert_eq!(safe_text(&"中".repeat(80), 60).chars().count(), 60);
    for text in [
        "设置 api_key=unknown-secret",
        "password: secret-value",
        "使用 sk-abcdef",
        "修改人格：不可记录的设定",
    ] {
        let result = safe_text(text, 60);
        assert!(!result.contains("unknown-secret"));
        assert!(!result.contains("secret-value"));
        assert!(!result.contains("sk-abcdef"));
        assert!(!result.contains("不可记录的设定"));
    }
    assert_eq!(safe_text("任务标题\n文件正文", 60), "任务标题");
    assert!(changed_files("file_write", &args(json!({"target":"api_key=secret"}))).is_empty());
}

#[tokio::test]
async fn 写入失败仍然返回实际工具成功() {
    let fixture = Fixture::new(10, 10000);
    fs::write(&fixture.store.dir, "阻止创建目录").unwrap();
    let audit = run(&fixture, "agent");
    let result: Result<String, AppError> = audit
        .scope(async {
            let mut attempt = ToolAttempt::new("test_tool", &HashMap::new(), &[]);
            attempt.finish(true);
            Ok("主任务成功".into())
        })
        .await;
    audit.finish("ok");
    drop(audit);
    assert_eq!(result.unwrap(), "主任务成功");
    assert!(fixture.store.append(&entry(1)).is_err());
}

#[tokio::test]
async fn 并行子任务隔离且保留父请求关联() {
    let fixture = Fixture::new(10, 10000);
    let parent = run(&fixture, "agent");
    parent
        .scope(async {
            futures_util::future::join_all((0..4).map(|index| {
                let store = fixture.store.clone();
                async move {
                    let child = AuditRun::with_store(
                        "sub_agent",
                        &format!("子任务{index}"),
                        Some("父请求"),
                        None,
                        None,
                        store,
                    );
                    child
                        .scope(async {
                            let mut tool = ToolAttempt::new(
                                "file_write",
                                &args(json!({"target":format!("{index}.txt")})),
                                &[],
                            );
                            tokio::task::yield_now().await;
                            tool.finish(true);
                        })
                        .await;
                    child.finish("ok");
                }
            }))
            .await;
        })
        .await;
    parent.finish("ok");
    drop(parent);
    let entries = fixture.list();
    assert_eq!(entries.len(), 5);
    assert!(entries
        .iter()
        .find(|entry| entry.kind == "agent")
        .unwrap()
        .tools
        .is_empty());
    for child in entries.iter().filter(|entry| entry.kind == "sub_agent") {
        assert_eq!(child.request_id.as_deref(), Some("父请求"));
        assert_eq!(child.tools.len(), 1);
        assert_eq!(child.files_changed.len(), 1);
    }
}

#[tokio::test]
async fn 超时丢弃仍记录取消和未完成工具() {
    let fixture = Fixture::new(10, 10000);
    let result = tokio::time::timeout(std::time::Duration::from_millis(10), async {
        let audit = run(&fixture, "sub_agent");
        audit
            .scope(async {
                let _tool =
                    ToolAttempt::new("file_write", &args(json!({"target":"未完成.txt"})), &[]);
                std::future::pending::<()>().await;
            })
            .await;
    })
    .await;
    assert!(result.is_err());
    let entries = fixture.list();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].result, "cancelled");
    assert!(!entries[0].tools[0].ok);
    assert!(entries[0].files_changed.is_empty());
}

#[test]
fn 五种记录类型及目标关联和自造结果() {
    let fixture = Fixture::new(10, 10000);
    for kind in ["chat", "agent", "sub_agent", "goal", "forge"] {
        let audit = run(&fixture, kind);
        audit.finish("ok");
    }
    let entries = fixture.list();
    assert_eq!(entries.len(), 5);
    assert_eq!(
        entries
            .iter()
            .find(|entry| entry.kind == "goal")
            .unwrap()
            .goal_id
            .as_deref(),
        Some("目标一")
    );
    assert_eq!(
        entries
            .iter()
            .find(|entry| entry.kind == "forge")
            .unwrap()
            .summary,
        "工具注册成功；成功"
    );
}

#[test]
fn 目标进展摘要只记步骤变化不复制模型正文() {
    let fixture = Fixture::new(10, 10000);
    let audit = run(&fixture, "goal");
    audit.goal_progress(1, 3, 5);
    audit.finish("partial");
    drop(audit);
    assert!(fixture.list()[0]
        .summary
        .contains("本轮新增完成 2 个步骤，累计完成 3/5 个步骤"));
    assert!(fixture.list()[0].summary.chars().count() <= 200);
}

#[tokio::test]
async fn 清空只删除审计且必须明确确认() {
    let fixture = Fixture::new(10, 1);
    fixture.store.append(&entry(1)).unwrap();
    fs::write(fixture.store.dir.join("config.json"), "保留配置").unwrap();
    fs::write(fixture.store.dir.join("audit-config.json"), "保留审计配置").unwrap();
    assert!(crate::commands::audit::clear_audit(false).await.is_err());
    assert_eq!(fixture.list().len(), 1);
    fixture.store.clear().unwrap();
    assert!(fixture.list().is_empty());
    assert!(fixture.store.dir.join("config.json").exists());
    assert!(fixture.store.dir.join("audit-config.json").exists());
}

#[test]
fn 损坏尾行后仍可追加且不覆盖原字节() {
    let fixture = Fixture::new(10, 10000);
    fixture.store.append(&entry(1)).unwrap();
    let path = fixture.store.dir.join("audit.jsonl");
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"{broken")
        .unwrap();
    let before = fs::read(&path).unwrap();
    fixture.store.append(&entry(2)).unwrap();
    assert_eq!(
        fs::read(fixture.store.dir.join("audit-1.jsonl")).unwrap(),
        before
    );
    assert!(fs::read(path).unwrap().ends_with(b"\n"));
    assert_eq!(fixture.list().len(), 2);
}

#[test]
fn 多线程追加不会交错成损坏行() {
    let fixture = Fixture::new(100, 100000);
    let threads: Vec<_> = (0..8)
        .map(|index| {
            let store = fixture.store.clone();
            std::thread::spawn(move || {
                for offset in 0..5 {
                    store.append(&entry(index * 5 + offset)).unwrap();
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(fixture.list().len(), 40);
    assert_eq!(fixture.store.segments().unwrap().len(), 40);
}
