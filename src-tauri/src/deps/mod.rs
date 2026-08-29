// 《铃·记忆体》统一依赖管理器 DependencyManager
// 设计：工具只声明依赖（如 ffmpeg/ollama/qemu + required/optional），
//       由本模块统一 check() / getInstallGuide() / openDownloadPage() / recheck()。
//       新增依赖只需在这里加一个检查函数，前端预设工具声明 dependencies 字段即可，
//       不用为每个工具重复写"缺依赖提示"。
use std::process::Stdio;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct DepStatus {
    pub id: String,
    pub installed: bool,
    pub name: String,
    pub required: bool,
    pub install: String,
    pub url: Option<String>,
}

/// 检查二进制是否在 PATH 中
fn bin_exists(name: &str) -> bool {
    std::process::Command::new("where")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn check_ffmpeg() -> DepStatus {
    DepStatus {
        id: "ffmpeg".into(),
        installed: bin_exists("ffmpeg"),
        name: "FFmpeg（视频转码）".into(),
        required: false,
        install: "PowerShell 管理员运行：winget install ffmpeg".into(),
        url: Some("https://ffmpeg.org/download.html".into()),
    }
}

fn check_ollama() -> DepStatus {
    let installed = bin_exists("ollama")
        || {
            let p = format!(
                "{}\\Programs\\Ollama\\ollama.exe",
                std::env::var("LOCALAPPDATA").unwrap_or_default()
            );
            std::path::Path::new(&p).exists()
        };
    DepStatus {
        id: "ollama".into(),
        installed,
        name: "Ollama（本地 AI）".into(),
        required: false,
        install: "PowerShell 管理员运行：winget install Ollama.Ollama".into(),
        url: Some("https://ollama.com/download".into()),
    }
}

fn check_qemu() -> DepStatus {
    DepStatus {
        id: "qemu".into(),
        installed: bin_exists("qemu-system-x86_64"),
        name: "QEMU（虚拟机）".into(),
        required: false,
        install: "PowerShell 管理员运行：winget install QEMU.QEMU".into(),
        url: Some("https://www.qemu.org/download/".into()),
    }
}

/// 统一检查入口：返回依赖状态（是否安装 / 安装方法 / 下载页）
/// 前端工具只声明依赖 id，由这里决定怎么检测、怎么引导。
#[tauri::command]
pub fn check_dependency(id: String) -> Result<DepStatus, String> {
    match id.as_str() {
        "ffmpeg" => Ok(check_ffmpeg()),
        "ollama" => Ok(check_ollama()),
        "qemu" => Ok(check_qemu()),
        other => Err(format!("未知依赖：{other}")),
    }
}
