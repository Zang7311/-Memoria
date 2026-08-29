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
