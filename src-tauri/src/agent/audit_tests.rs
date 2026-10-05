use super::loop_::{execute_task_tool, TaskRuntime};
use super::stream_progress::StreamEmitter;
use super::tools::AgentPermissions;
use crate::audit::{AuditConfig, AuditQuery, AuditRun, AuditStore};
use crate::error::AppError;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

fn runtime() -> TaskRuntime {
    TaskRuntime {
        base: String::new(),
        key: String::new(),
        model: String::new(),
        depth: 1,
        cfg: crate::config::defaults::default_config(),
        perms: AgentPermissions::default(),
        dispatch: |_, name, _, _| {
            Box::pin(async move {
                if name == "file_delete" {
                    Err(AppError::ToolboxError("不得记录的错误原文".into()))
                } else {
                    Ok("不得记录的文件和网页正文".into())
                }
            })
        },
    }
}

#[tokio::test]
async fn audit_real_tool_dispatch_aggregates_without_recording_outputs() {
    let dir = std::env::temp_dir().join(format!("mem-audit-dispatch-{}", uuid::Uuid::new_v4()));
    let store = Arc::new(AuditStore::new(dir.clone(), AuditConfig::default()));
    let audit = AuditRun::with_store(
        "agent",
        "真实工具聚合",
        Some("父请求"),
        None,
        None,
        store.clone(),
    );
    let runtime = runtime();
    let emitter = StreamEmitter::silent();
    audit
        .scope(async {
            for name in ["file_write", "file_delete"] {
                let args = HashMap::from([
                    ("target".into(), json!("报告.txt")),
                    ("api_key".into(), json!("private-dispatch-key")),
                ]);
                let tools = vec![json!({"function":{"name":name},"x-intent":"file.write"})];
                let result = execute_task_tool(
                    None,
                    name,
                    &args,
                    &tools,
                    &runtime,
                    &emitter,
                    "父请求",
                    false,
                    None,
                )
                .await;
                assert_eq!(result.is_ok(), name == "file_write");
            }
        })
        .await;
    audit.finish("ok");
    drop(audit);
    let entries = store.list(&AuditQuery::default()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].tools.len(), 2);
    assert_eq!(entries[0].result, "partial");
    assert_eq!(entries[0].files_changed, ["报告.txt"]);
    let text = std::fs::read_to_string(dir.join("audit.jsonl")).unwrap();
    for value in [
        "private-dispatch-key",
        "不得记录的错误原文",
        "不得记录的文件和网页正文",
    ] {
        assert!(!text.contains(value));
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn audit_real_dispatch_succeeds_when_audit_directory_is_unwritable() {
    let dir = std::env::temp_dir().join(format!("mem-audit-failure-{}", uuid::Uuid::new_v4()));
    std::fs::write(&dir, "目录被文件阻挡").unwrap();
    let store = Arc::new(AuditStore::new(dir.clone(), AuditConfig::default()));
    let audit = AuditRun::with_store("agent", "失败隔离", None, None, None, store);
    let result = audit
        .scope(execute_task_tool(
            None,
            "file_write",
            &HashMap::new(),
            &[],
            &runtime(),
            &StreamEmitter::silent(),
            "请求",
            false,
            None,
        ))
        .await;
    audit.finish("ok");
    drop(audit);
    assert!(result.is_ok());
    std::fs::remove_file(dir).unwrap();
}
