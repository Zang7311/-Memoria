//! 多步计划确认闸门（plan_gate.rs）——「先规划后执行」的用户确认环节
//!
//! 规划器拆出多步计划后，先把计划推给前端（`agent-plan-request` 事件），
//! 用户可以在卡片上改步骤、确认或取消；确认（或超时）后才真正开始执行。
//!
//! 设计要点：
//!   - 等待状态只存一个 oneshot sender，plan_id 直接用 request_id
//!     （一次任务同一时刻最多一个待确认的计划）。
//!   - 超时（WAIT_SECS）后按原计划继续 —— 用户没盯着屏幕时任务不能卡死。
//!   - `agent_cancel` 会把等待中的计划一并取消（PlanReply::Cancel），
//!     用户点「停止」时不用等满 120 秒。
//!   - 这里不碰 UI：事件推送与超时兜底由 loop_.rs 负责，方便单测。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::oneshot;

/// 计划卡里展示的一步（字段与 `planner::SubTask` 对齐）
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlanStep {
    pub step: usize,
    pub goal: String,
    pub tool_hint: Option<String>,
}

/// 用户对计划的回应
#[derive(Debug, Clone, PartialEq)]
pub enum PlanReply {
    /// 确认执行；字符串是（可能被编辑过的）步骤文本
    Approve(Vec<String>),
    /// 取消整次任务
    Cancel,
}

/// 用户等待确认的最长时间（秒）。超时按原计划继续，不能把任务卡死。
pub const WAIT_SECS: u64 = 120;

/// request_id → 等待回应的发送端
static PENDING: Mutex<Option<HashMap<String, oneshot::Sender<PlanReply>>>> = Mutex::new(None);

fn with_pending<R>(f: impl FnOnce(&mut HashMap<String, oneshot::Sender<PlanReply>>) -> R) -> R {
    let mut guard = PENDING.lock().unwrap_or_else(|poison| poison.into_inner());
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    f(guard.as_mut().expect("刚刚已初始化"))
}

/// 登记一个等待确认的计划，返回接收端。
pub fn register(plan_id: &str) -> oneshot::Receiver<PlanReply> {
    let (tx, rx) = oneshot::channel();
    with_pending(|pending| {
        // 同一 request 理论上不会重复登记；真重复就替换旧的（旧任务会自然超时收场）
        pending.insert(plan_id.to_string(), tx);
    });
    rx
}

/// 提交用户回应；返回是否真的有一个在等的人（超时/取消后为 false）。
pub fn settle(plan_id: &str, reply: PlanReply) -> bool {
    let sender = with_pending(|pending| pending.remove(plan_id));
    match sender {
        Some(tx) => tx.send(reply).is_ok(),
        None => false,
    }
}

/// 放弃等待（超时等场景），清掉登记避免泄漏。
pub fn discard(plan_id: &str) {
    with_pending(|pending| {
        pending.remove(plan_id);
    });
}

/// 阻塞等待用户回应，超时返回 None。
pub async fn wait(rx: oneshot::Receiver<PlanReply>, timeout: Duration) -> Option<PlanReply> {
    match tokio::time::timeout(timeout, rx).await {
        Ok(Ok(reply)) => Some(reply),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn 确认后能把编辑过的步骤带回来() {
        let rx = register("req-confirm");
        assert!(settle("req-confirm", PlanReply::Approve(vec!["第一步".into(), "第二步".into()])));
        let reply = wait(rx, Duration::from_millis(200)).await;
        assert_eq!(reply, Some(PlanReply::Approve(vec!["第一步".into(), "第二步".into()])));
    }

    #[tokio::test]
    async fn 取消能把等待中的任务叫醒() {
        let rx = register("req-cancel");
        assert!(settle("req-cancel", PlanReply::Cancel));
        assert_eq!(wait(rx, Duration::from_millis(200)).await, Some(PlanReply::Cancel));
    }

    #[tokio::test]
    async fn 超时后返回空而不是卡死() {
        let rx = register("req-timeout");
        assert_eq!(wait(rx, Duration::from_millis(30)).await, None);
        // 超时后调用方会 discard；此时再 settle 应返回 false
        discard("req-timeout");
        assert!(!settle("req-timeout", PlanReply::Cancel));
    }

    #[test]
    fn 没有等待者时settle返回false() {
        assert!(!settle("req-nobody", PlanReply::Approve(vec![])));
    }

    #[test]
    fn 重复登记不会泄漏旧等待者() {
        let _old = register("req-dup");
        let _new = register("req-dup");
        // 重复登记时旧的发送端被丢弃，settle 只认最新那个
        assert!(settle("req-dup", PlanReply::Cancel));
    }
}
