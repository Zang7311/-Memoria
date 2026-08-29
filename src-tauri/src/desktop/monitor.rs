// 《铃·记忆体》屏幕监测核心（AI-6 任务 3/4/5）
// 功能：
//  - 定时轮询前台窗口（winapi），检测应用/标题变化
//  - 规则匹配（支持通配符 *、精确匹配、冷却时间）
//  - 触发后通过事件推送气泡弹窗
//  - 三层兜底：API 失败降级 → 完全失败自动禁用 → 用户手动配置规则作为备选
use crate::error::AppError;
use crate::types::{MonitorTriggerEvent, ScreenMonitorRule, WindowInfo};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// 是否开启屏幕监测
static MONITORING: AtomicBool = AtomicBool::new(true);
/// 屏幕监测是否可用（三层兜底：完全失败时为 false）
static AVAILABLE: AtomicBool = AtomicBool::new(true);
/// 轮询间隔（秒，默认 3）
static INTERVAL: AtomicU32 = AtomicU32::new(3);
/// 规则缓存（进程内共享，持久化到 monitor_rules.json）
static RULES: Mutex<Vec<ScreenMonitorRule>> = Mutex::new(Vec::new());
/// 每条规则最近触发时间（用于冷却）
static LAST_TRIGGER: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ==================== 前台窗口检测（winapi，任务 3） ====================

/// 获取当前前台窗口信息（三层兜底第 1 层：API 失败自动降级）
///
/// 兜底设计：
///  - QueryFullProcessImageNameW 失败 → 应用名回退为 "unknown"，但仍返回窗口标题（不阻塞）
///  - GetForegroundWindow 本身失败 → 返回 Err，由调用方触发降级/禁用逻辑
pub fn detect_window() -> Result<WindowInfo, AppError> {
    #[cfg(target_os = "windows")]
    {
        use winapi::shared::minwindef::{DWORD, FALSE, MAX_PATH};
        use winapi::shared::windef::{HWND, RECT};
        use winapi::um::handleapi::CloseHandle;
        use winapi::um::processthreadsapi::OpenProcess;
        use winapi::um::winbase::QueryFullProcessImageNameW;
        use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;
        use winapi::um::winuser::{
            GetForegroundWindow, GetSystemMetrics, GetWindowRect, GetWindowTextW,
            GetWindowThreadProcessId, SM_CXSCREEN, SM_CYSCREEN,
        };

        unsafe {
            let hwnd: HWND = GetForegroundWindow();
            if hwnd.is_null() {
                return Err(AppError::WindowInfoError(
                    "无法获取前台窗口句柄（GetForegroundWindow 返回空）".to_string(),
                ));
            }

            // 1) 窗口标题（GetWindowTextW，失败不影响）
            let mut title_buf = [0u16; 512];
            GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
            let window_title = String::from_utf16_lossy(&title_buf)
                .trim_end_matches('\0')
                .to_string();

            // 2) 进程 ID
            let mut pid: DWORD = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);

            // 3) 可执行文件名（QueryFullProcessImageNameW，失败回退 unknown——兜底第 1 层）
            let mut app_name = String::from("unknown");
            let process_handle =
                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
            if !process_handle.is_null() {
                let mut path_buf = [0u16; MAX_PATH];
                let mut size: DWORD = path_buf.len() as DWORD;
                let ok = QueryFullProcessImageNameW(
                    process_handle,
                    0,
                    path_buf.as_mut_ptr(),
                    &mut size,
                );
                if ok != FALSE {
                    let path = String::from_utf16_lossy(&path_buf[..size as usize]);
                    if let Some(name) =
                        std::path::Path::new(&path).file_name()
                    {
                        app_name = name.to_string_lossy().to_string();
                    }
                } else {
                    log::warn!(
                        "[monitor] QueryFullProcessImageNameW 失败，降级为 unknown"
                    );
                }
                CloseHandle(process_handle);
            }

            // 4) 全屏判断：窗口矩形是否覆盖整个屏幕
            let mut rect: RECT = std::mem::zeroed();
            let mut is_fullscreen = false;
            if GetWindowRect(hwnd, &mut rect) != 0 {
                let sw = GetSystemMetrics(SM_CXSCREEN);
                let sh = GetSystemMetrics(SM_CYSCREEN);
                let w = rect.right - rect.left;
                let h = rect.bottom - rect.top;
                if w >= sw && h >= sh {
                    is_fullscreen = true;
                }
            }

            Ok(WindowInfo {
                app_name,
                window_title,
                is_fullscreen,
                is_foreground: true,
            })
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(AppError::WindowInfoError("非 Windows 平台暂不支持窗口检测".to_string()))
    }
}

// ==================== 规则管理（任务 5） ====================

/// 从文件加载规则（文件不存在时返回空列表）
pub fn load_rules() -> Vec<ScreenMonitorRule> {
    let path = crate::desktop::monitor_rules_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str::<Vec<ScreenMonitorRule>>(&s)
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// 保存规则到文件（失败仅记日志，不阻塞）
pub fn save_rules(rules: &[ScreenMonitorRule]) {
    let path = crate::desktop::monitor_rules_path();
    match serde_json::to_string_pretty(rules) {
        Ok(s) => {
            if let Err(e) = std::fs::write(&path, s) {
                log::warn!("[monitor] 保存规则失败：{e}");
            }
        }
        Err(e) => log::warn!("[monitor] 序列化规则失败：{e}"),
    }
}

/// 初始化规则缓存（应用启动时调用）
pub fn init() {
    let rules = load_rules();
    *RULES.lock().unwrap() = rules;
}

/// 获取规则列表（克隆，避免锁竞争）
pub fn get_rules() -> Vec<ScreenMonitorRule> {
    RULES.lock().unwrap().clone()
}

/// 更新或新增单条规则（id 相同则覆盖）
pub fn update_rule(rule: ScreenMonitorRule) {
    let mut guard = RULES.lock().unwrap();
    if let Some(existing) = guard.iter_mut().find(|r| r.id == rule.id) {
        *existing = rule;
    } else {
        guard.push(rule);
    }
    save_rules(&guard);
}

/// 删除单条规则
pub fn delete_rule(rule_id: &str) {
    let mut guard = RULES.lock().unwrap();
    guard.retain(|r| r.id != rule_id);
    save_rules(&guard);
}

// ==================== 监测状态控制 ====================

pub fn is_monitoring() -> bool {
    MONITORING.load(Ordering::Relaxed)
}

pub fn is_available() -> bool {
    AVAILABLE.load(Ordering::Relaxed)
}

/// 当前轮询间隔（秒）
pub fn get_interval() -> u32 {
    INTERVAL.load(Ordering::Relaxed)
}

/// 设置监测开关与频率；返回是否成功启用
pub fn set_monitoring(enabled: bool, interval_seconds: Option<u32>) -> bool {
    if enabled {
        // 若已知不可用（三层兜底第 2 层已禁用），拒绝重新开启
        if !AVAILABLE.load(Ordering::Relaxed) {
            log::warn!("[monitor] 屏幕监测不可用，无法开启（请检查系统权限）");
            return false;
        }
        MONITORING.store(true, Ordering::Relaxed);
    } else {
        MONITORING.store(false, Ordering::Relaxed);
    }
    if let Some(iv) = interval_seconds {
        INTERVAL.store(iv.clamp(1, 60), Ordering::Relaxed);
    }
    true
}

// ==================== 规则匹配 ====================

/// 通配符匹配：支持 `*`（前缀/后缀/包含）、精确匹配
fn app_matches(pattern: &str, app: &str) -> bool {
    let p = pattern.trim().to_lowercase();
    let a = app.trim().to_lowercase();
    if p == "*" || p.is_empty() {
        return true;
    }
    if p.contains('*') {
        if p.starts_with('*') && p.ends_with('*') {
            let mid = &p[1..p.len() - 1];
            mid.is_empty() || a.contains(mid)
        } else if p.starts_with('*') {
            a.ends_with(&p[1..])
        } else if p.ends_with('*') {
            a.starts_with(&p[..p.len() - 1])
        } else {
            a == p
        }
    } else {
        a == p
    }
}

/// 检查规则是否处于冷却期
fn in_cooldown(rule_id: &str, cooldown_seconds: u32) -> bool {
    if cooldown_seconds == 0 {
        return false;
    }
    let map = LAST_TRIGGER.lock().unwrap();
    if let Some(last) = map.get(rule_id) {
        if last.elapsed() < Duration::from_secs(cooldown_seconds as u64) {
            return true;
        }
    }
    false
}

/// 记录触发时间
fn record_trigger(rule_id: &str) {
    LAST_TRIGGER.lock().unwrap().insert(rule_id.to_string(), Instant::now());
}

/// 匹配前台窗口并触发规则
fn check_and_trigger(app: &AppHandle, info: &WindowInfo) {
    let rules = get_rules();
    for rule in rules {
        if !rule.enabled {
            continue;
        }
        if !app_matches(&rule.app_name, &info.app_name) {
            continue;
        }
        if in_cooldown(&rule.id, rule.cooldown_seconds) {
            continue;
        }
        record_trigger(&rule.id);

        log::info!(
            "[monitor] 触发规则 {}（app={}）reply={}",
            rule.id,
            info.app_name,
            rule.trigger_reply
        );

        // 气泡内容：规则预设回复（避免直接调用流式 send_message 污染对话/记忆）
        let payload = MonitorTriggerEvent {
            app_name: info.app_name.clone(),
            window_title: info.window_title.clone(),
            reply: if rule.trigger_reply.is_empty() {
                "主人在忙什么呢？".to_string()
            } else {
                rule.trigger_reply.clone()
            },
            rule_id: rule.id.clone(),
        };
        let _ = app.emit("monitor-trigger", payload);
    }
}

// ==================== 后台轮询主循环（任务 3/4） ====================

/// 启动屏幕监测后台任务（应用 setup 时调用一次）
pub fn start_monitor(app: AppHandle) {
    init();
    tauri::async_runtime::spawn(async move {
        let mut last_app = String::new();
        let mut consecutive_failures = 0u32;
        loop {
            // 未开启时休眠等待
            if !MONITORING.load(Ordering::Relaxed) {
                consecutive_failures = 0;
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }

            match detect_window() {
                Ok(info) => {
                    consecutive_failures = 0;
                    // 仅前台应用变化时重新匹配（减少触发）
                    if info.app_name != last_app {
                        last_app = info.app_name.clone();
                        check_and_trigger(&app, &info);
                    }
                }
                Err(e) => {
                    consecutive_failures += 1;
                    log::error!("[monitor] 窗口检测失败（第 {consecutive_failures} 次）：{e}");
                    // 三层兜底第 2 层：连续失败达到阈值 → 判定不可用，自动禁用并提示
                    if consecutive_failures >= 5 {
                        AVAILABLE.store(false, Ordering::Relaxed);
                        MONITORING.store(false, Ordering::Relaxed);
                        log::error!("[monitor] 屏幕监测不可用，已自动禁用（请检查系统权限）");
                        let _ = app.emit(
                            "monitor-unavailable",
                            "屏幕监测不可用，请检查系统权限，已自动禁用该功能",
                        );
                    }
                }
            }

            let iv = INTERVAL.load(Ordering::Relaxed).clamp(1, 60) as u64;
            tokio::time::sleep(Duration::from_secs(iv)).await;
        }
    });
}
