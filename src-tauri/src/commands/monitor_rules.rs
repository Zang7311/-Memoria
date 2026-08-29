// 《铃·记忆体》IPC：屏幕监测规则管理（AI-6 任务 10）
use crate::desktop::monitor;
use crate::types::{
    DeleteMonitorRuleRequest, GetMonitorRulesResponse, SetMonitoringRequest,
    UpdateMonitorRuleRequest,
};

/// 获取监测状态 + 规则列表
#[tauri::command]
pub fn get_monitor_rules() -> GetMonitorRulesResponse {
    GetMonitorRulesResponse {
        rules: monitor::get_rules(),
        enabled: monitor::is_monitoring(),
        interval_seconds: monitor::get_interval(),
        available: monitor::is_available(),
    }
}

/// 更新（或新增）单条规则
#[tauri::command]
pub fn update_monitor_rule(request: UpdateMonitorRuleRequest) {
    monitor::update_rule(request.rule);
}

/// 删除单条规则
#[tauri::command]
pub fn delete_monitor_rule(request: DeleteMonitorRuleRequest) {
    monitor::delete_rule(&request.rule_id);
}

/// 启用/禁用屏幕监测（可附带调整轮询频率）
/// 返回最终是否处于启用状态（不可用时强制关闭并返回 false）
#[tauri::command]
pub fn toggle_monitoring(request: SetMonitoringRequest) -> bool {
    monitor::set_monitoring(request.enabled, request.interval_seconds)
        && monitor::is_monitoring()
}
