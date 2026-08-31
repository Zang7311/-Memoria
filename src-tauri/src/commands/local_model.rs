// 《铃·记忆体》内置本地模型状态查询（v1.0 离线智能版）
//
// 替代 v0.5.x 的 local_ai.rs（Ollama 检测 / ollama pull / OLLAMA_MODELS 环境变量）。
// 内置模型不需要外部进程，前端只需要知道：两档 GGUF 在不在、多大、放哪、内存够不够。
use crate::engine::local_llm::{self, ModelSize};
use serde::Serialize;

/// 单档内置模型的状态
#[derive(Debug, Serialize)]
pub struct LocalModelInfo {
    /// "0.5b" | "1.5b"
    pub size: String,
    /// 人话名（内置 0.5B / 内置 1.5B）
    pub label: String,
    /// GGUF 文件名
    pub file_name: String,
    /// 是否已就位（可直接对话）
    pub available: bool,
    /// 已就位时的绝对路径
    pub path: Option<String>,
    /// 文件大小（MB），未就位为 0
    pub size_mb: f64,
    /// 未就位时的引导文案
    pub hint: Option<String>,
}

/// 两档模型的整体状态（含内存提示与模型目录）
#[derive(Debug, Serialize)]
pub struct LocalModelStatus {
    pub models: Vec<LocalModelInfo>,
    /// 推荐放置目录（~/.铃记忆体-v10/models/qwen）
    pub models_dir: String,
    /// 物理内存（MB），0 表示采集失败
    pub memory_total_mb: u64,
    /// 内存是否够跑 1.5B（建议 ≥4GB 物理内存）
    pub can_run_1b: bool,
}

fn info_of(size: ModelSize) -> LocalModelInfo {
    match local_llm::find_model(size) {
        Some(p) => {
            let size_mb = std::fs::metadata(&p)
                .map(|m| m.len() as f64 / 1024.0 / 1024.0)
                .unwrap_or(0.0);
            LocalModelInfo {
                size: size.as_str().to_string(),
                label: size.label().to_string(),
                file_name: size.file_name().to_string(),
                available: true,
                path: Some(p.display().to_string()),
                size_mb,
                hint: None,
            }
        }
        None => LocalModelInfo {
            size: size.as_str().to_string(),
            label: size.label().to_string(),
            file_name: size.file_name().to_string(),
            available: false,
            path: None,
            size_mb: 0.0,
            hint: Some(local_llm::missing_model_hint(size)),
        },
    }
}

/// 检测两档内置模型是否就位（设置页「运行模式」三档卡片用它判断能不能选）
#[tauri::command]
pub fn detect_local_models() -> LocalModelStatus {
    let memory_total_mb = {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_memory();
        sys.total_memory() / 1024 / 1024
    };
    LocalModelStatus {
        models: vec![info_of(ModelSize::B05), info_of(ModelSize::B15)],
        models_dir: crate::config::data_dir()
            .join("models")
            .join("qwen")
            .display()
            .to_string(),
        memory_total_mb,
        // 1.5B q4_k_m 常驻约 1.6GB，留出系统与 WebView 开销 → 建议 4GB 以上
        can_run_1b: memory_total_mb == 0 || memory_total_mb >= 4000,
    }
}

/// 显卡显存信息（沿用 v0.5.x：设置页展示用，纯信息，不影响 CPU 推理）
#[derive(Debug, Serialize)]
pub struct GpuVram {
    pub name: String,
    pub vram_mb: u64,
}

/// 检测显卡显存（PowerShell 查 Win32_VideoController.AdapterRAM，单位转为 MB）
#[tauri::command]
pub fn detect_gpu_vram() -> Vec<GpuVram> {
    let mut result = Vec::new();
    let mut cmd = std::process::Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-Command",
        "Get-CimInstance Win32_VideoController | ForEach-Object { \"$($_.Name)|$($_.AdapterRAM)\" }",
    ]);
    // GUI 应用隐藏控制台窗口，避免弹出蓝色终端
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    if let Ok(o) = cmd.output() {
        if o.status.success() {
            let text = String::from_utf8_lossy(&o.stdout);
            for line in text.lines() {
                let mut parts = line.split('|');
                if let (Some(name), Some(ram)) = (parts.next(), parts.next()) {
                    if let Ok(bytes) = ram.trim().parse::<u64>() {
                        result.push(GpuVram {
                            name: name.trim().to_string(),
                            vram_mb: bytes / (1024 * 1024),
                        });
                    }
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_reports_both_sizes_consistently() {
        let s = detect_local_models();
        assert_eq!(s.models.len(), 2);
        assert_eq!(s.models[0].size, "0.5b");
        assert_eq!(s.models[1].size, "1.5b");
        // 无论模型在不在，字段要自洽：available 时给 path，否则给 hint
        for m in &s.models {
            if m.available {
                assert!(m.path.is_some());
                assert!(m.hint.is_none());
            } else {
                assert!(m.path.is_none());
                assert!(m.hint.is_some());
            }
        }
        assert!(s.models_dir.contains("qwen"));
    }
}
