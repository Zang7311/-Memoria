// 《铃·记忆体》诊断模块（AI-7 任务 8/9）
// 系统信息采集（sysinfo）供诊断面板与诊断包共用。
pub mod export;

use crate::error::AppError;
use crate::types::{DiskInfo, SystemInfo};
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};

/// 采集系统信息（CPU/内存/磁盘/OS/应用版本）
/// 由 commands/system_info.rs 与 diagnostic/export.rs 共用
pub fn collect_system_info() -> Result<SystemInfo, AppError> {
    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );
    // CPU 使用率需两次采样间隔计算
    sys.refresh_cpu_usage();
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu_usage();

    let cpus = sys.cpus();
    let cpu_name = cpus
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "未知 CPU".to_string());
    let cpu_cores = cpus.len();
    let cpu_usage = sys.global_cpu_info().cpu_usage();

    let memory_total_mb = sys.total_memory() / 1024 / 1024;
    let memory_used_mb = sys.used_memory() / 1024 / 1024;

    // sysinfo 0.30：name()/os_version() 为关联函数（无 self）
    let os_name = sysinfo::System::name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "未知".to_string());
    let os_version = sysinfo::System::os_version()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "未知".to_string());

    let mut disks = Vec::new();
    for d in Disks::new_with_refreshed_list().list() {
        disks.push(DiskInfo {
            name: d.name().to_string_lossy().to_string(),
            total_gb: d.total_space() / 1024 / 1024 / 1024,
            available_gb: d.available_space() / 1024 / 1024 / 1024,
        });
    }

    Ok(SystemInfo {
        cpu_name,
        cpu_cores,
        cpu_usage,
        memory_total_mb,
        memory_used_mb,
        os_name,
        os_version,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        disks,
    })
}

/// 将系统信息格式化为人类可读文本（写入诊断包）
pub fn format_system_info(info: &SystemInfo) -> String {
    let mut s = String::new();
    s.push_str(&format!("《铃·记忆体》系统诊断信息\n"));
    s.push_str(&format!("================================\n"));
    s.push_str(&format!("应用版本：{}\n", info.app_version));
    s.push_str(&format!("操作系统：{} {}\n", info.os_name, info.os_version));
    s.push_str(&format!("CPU：{}（{} 核，使用率 {:.1}%）\n", info.cpu_name, info.cpu_cores, info.cpu_usage));
    s.push_str(&format!("内存：已用 {:.1} GB / 共 {:.1} GB\n", info.memory_used_mb as f64 / 1024.0, info.memory_total_mb as f64 / 1024.0));
    s.push_str("磁盘分区：\n");
    for d in &info.disks {
        s.push_str(&format!(
            "  {}：可用 {:.1} GB / 共 {:.1} GB\n",
            d.name, d.available_gb as f64, d.total_gb as f64
        ));
    }
    s
}

// ==================== P3 救援模式 ====================

/// 救援检测：检查各项关键资源是否完好（恢复窗口用）
/// 返回每项 { name, ok, detail }
#[tauri::command]
pub fn recovery_check() -> Result<Vec<serde_json::Value>, AppError> {
    let mut results = Vec::new();

    // 1. 配置文件
    let cfg_path = crate::config::config_path();
    let cfg_ok = cfg_path.exists();
    let cfg_detail = if cfg_ok {
        cfg_path.to_string_lossy().to_string()
    } else {
        "缺失（将自动重建默认配置）".to_string()
    };
    results.push(serde_json::json!({
        "name": "配置文件",
        "ok": cfg_ok,
        "detail": cfg_detail,
    }));

    // 2. 数据目录（记忆）
    let data_path = crate::config::default_data_path();
    let data_ok = std::path::Path::new(&data_path).exists();
    let data_detail = if data_ok {
        data_path.clone()
    } else {
        "尚未创建（首次使用属正常）".to_string()
    };
    results.push(serde_json::json!({
        "name": "记忆数据目录",
        "ok": data_ok,
        "detail": data_detail,
    }));

    // 3. 记忆索引文件
    let idx_path = std::path::Path::new(&data_path).join("index.json");
    let idx_ok = idx_path.exists();
    let idx_detail = if idx_ok {
        let count = crate::memory::storage::read_all(&idx_path).map(|v| v.len()).unwrap_or(0);
        format!("{} 条记忆", count)
    } else {
        "无索引（首次使用属正常）".to_string()
    };
    results.push(serde_json::json!({
        "name": "记忆索引",
        "ok": idx_ok,
        "detail": idx_detail,
    }));

    // 4. 插件目录
    let plugin_dir = std::env::var("APPDATA")
        .map(|a| std::path::PathBuf::from(a).join("ling-memoria/plugins"))
        .unwrap_or_default();
    let plugin_ok = plugin_dir.exists();
    let plugin_detail = if plugin_ok {
        plugin_dir.to_string_lossy().to_string()
    } else {
        "无插件（正常）".to_string()
    };
    results.push(serde_json::json!({
        "name": "插件目录",
        "ok": plugin_ok,
        "detail": plugin_detail,
    }));

    // 5. 日志目录
    let log_dir = std::env::var("APPDATA")
        .map(|a| std::path::PathBuf::from(a).join("ling-memoria/logs"))
        .unwrap_or_default();
    let log_ok = log_dir.exists();
    let log_detail = if log_ok {
        log_dir.to_string_lossy().to_string()
    } else {
        "无日志（正常）".to_string()
    };
    results.push(serde_json::json!({
        "name": "日志目录",
        "ok": log_ok,
        "detail": log_detail,
    }));

    Ok(results)
}

/// 重置配置：备份损坏配置后重建默认配置（恢复窗口用）
#[tauri::command]
pub fn recovery_reset_config() -> Result<String, AppError> {
    let cfg_path = crate::config::config_path();
    if cfg_path.exists() {
        // 备份为 config.json.bak-<时间戳>
        let ts = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
        let bak = cfg_path.with_file_name(format!("config.json.bak-{}", ts));
        std::fs::copy(&cfg_path, &bak).map_err(|e| AppError::ConfigError(e.to_string()))?;
        let _ = std::fs::remove_file(&cfg_path);
        log::warn!("[recovery] 已备份并重置配置：{}", bak.to_string_lossy());
        Ok(format!("配置已备份到 {}，并重建默认配置", bak.to_string_lossy()))
    } else {
        Ok("配置文件不存在，无需重置".to_string())
    }
}
