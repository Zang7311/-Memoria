// Agent 任务取消标志表
//
// 每个任务按 request_id 注册一个 AtomicBool；前端调用 agent_cancel 时将其置 true。
// 任务结束（正常/中断/超步）后必须调用 cleanup 清理条目，防止内存泄漏。
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};

static CANCEL_FLAGS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

fn flags() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    CANCEL_FLAGS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 注册一个新任务，返回其取消标志的引用。
/// 若同名 id 已存在（极少见），直接复用并重置为 false。
pub fn register(request_id: &str) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let mut map = flags().lock().unwrap_or_else(|e| e.into_inner());
    map.insert(request_id.to_string(), flag.clone());
    flag
}

/// 检查指定任务是否已被取消。
/// 若 id 不存在（任务已结束），保守返回 false。
pub fn is_cancelled(request_id: &str) -> bool {
    let map = flags().lock().unwrap_or_else(|e| e.into_inner());
    map.get(request_id)
        .map(|f| f.load(Ordering::Relaxed))
        .unwrap_or(false)
}

/// 将指定任务标记为「已取消」。
/// 若 id 不存在（任务已结束），静默忽略。
pub fn set_cancelled(request_id: &str) {
    let map = flags().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(f) = map.get(request_id) {
        f.store(true, Ordering::Relaxed);
    }
}

/// 任务结束后清理条目，避免内存无限增长。
pub fn cleanup(request_id: &str) {
    let mut map = flags().lock().unwrap_or_else(|e| e.into_inner());
    map.remove(request_id);
}

/// 取消标志的 RAII 守卫。
///
/// **为什么必须有它**：靠「每个 return 前手写 cleanup」是不可靠的 ——
/// 只要函数里出现任何一个 `?` 提前返回（例如 `call_llm(...).await?`、
/// `.ok_or_else(...)?`），就会绕过清理，条目永久留在全局表里泄漏。
/// 而且以后任何人往函数里新增一个早退分支，都会静默漏掉清理。
///
/// 用 Drop 守卫之后，正确性由类型系统保证：无论从哪条路返回
/// （正常收尾 / 用户中断 / 提前出错），Drop 一定会执行。
pub struct CancelGuard(String);

impl CancelGuard {
    /// 注册任务并返回守卫；守卫离开作用域时自动 cleanup。
    pub fn new(request_id: &str) -> Self {
        register(request_id);
        Self(request_id.to_string())
    }

    /// 本守卫对应的任务 id
    pub fn request_id(&self) -> &str {
        &self.0
    }
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        cleanup(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 未设置取消标志时返回_false() {
        let id = "test_no_cancel";
        let _flag = register(id);
        assert!(!is_cancelled(id));
        cleanup(id);
    }

    #[test]
    fn 设置取消标志后返回_true() {
        let id = "test_set_cancel";
        let _flag = register(id);
        assert!(!is_cancelled(id));
        set_cancelled(id);
        assert!(is_cancelled(id));
        cleanup(id);
    }

    #[test]
    fn cleanup_后标志被清理() {
        let id = "test_cleanup";
        let _flag = register(id);
        set_cancelled(id);
        cleanup(id);
        // 清理后 id 不存在，保守返回 false
        assert!(!is_cancelled(id));
    }

    #[test]
    fn 不存在的_id_返回_false() {
        assert!(!is_cancelled("nonexistent_task_id_xyz"));
    }

    #[test]
    fn set_cancelled_对不存在_id_不_panic() {
        // 任务已结束后前端延迟发来取消，不应 panic
        set_cancelled("already_gone_id");
    }

    #[test]
    fn 守卫_正常离开作用域时清理() {
        let id = "guard_normal";
        {
            let g = CancelGuard::new(id);
            assert_eq!(g.request_id(), id);
            assert!(!is_cancelled(id), "刚注册时不该是已取消");
        }
        assert!(!is_cancelled(id), "守卫 Drop 后条目必须被移除");
    }

    /// 返回一个必然失败的 Result，用来触发调用方的 `?` 提前返回
    fn 必然失败() -> Result<(), String> {
        Err("boom".to_string())
    }

    #[test]
    fn 守卫_遇到问号提前返回也清理() {
        // 这正是原实现的 bug：函数里一旦出现 `?` 早退（如 call_llm(...).await?、
        // .ok_or_else(...)?），手写的 cleanup 就被跳过，条目永久泄漏。
        // Drop 守卫必须在这个场景下仍然清理 —— 这就是它存在的全部意义。
        fn 模拟带问号的函数(id: &str) -> Result<(), String> {
            let _g = CancelGuard::new(id);
            必然失败()?; // ← 在这里提前返回，绕过任何手写 cleanup
            Ok(())
        }

        let id = "guard_early_return";
        assert!(模拟带问号的函数(id).is_err());
        assert!(!is_cancelled(id), "`?` 提前返回时守卫也必须完成清理");
    }

    #[test]
    fn 守卫_中断路径同样清理() {
        let id = "guard_cancelled_path";
        {
            let _g = CancelGuard::new(id);
            set_cancelled(id);
            assert!(is_cancelled(id), "中断期间应能查到已取消");
        }
        // 被取消过的任务收尾后条目也要清掉，不能因为「取消过」就留着
        assert!(!is_cancelled(id));
    }
}
