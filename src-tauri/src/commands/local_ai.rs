// 《铃·记忆体》一键本地部署 AI（收尾工程师）
// 引导式部署：检测 Ollama 是否安装 → 未装则引导下载 → 已装则一键拉取模型
use crate::error::AppError;
use serde::Serialize;

/// 检测结果：是否已安装 Ollama + 已安装的模型列表
#[derive(Debug, Serialize)]
pub struct DetectOllamaResponse {
    pub installed: bool,
    pub models: Vec<String>,
}

/// 检测 Ollama 服务是否可用，并列出已安装模型（GET /api/tags）
#[tauri::command]
pub async fn detect_ollama() -> Result<DetectOllamaResponse, AppError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| AppError::NetworkError(e.to_string()))?;
    let resp = match client.get("http://localhost:11434/api/tags").send().await {
        Ok(r) if r.status().is_success() => r,
        _ => {
            return Ok(DetectOllamaResponse {
                installed: false,
                models: Vec::new(),
            })
        }
    };
    let json: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => {
            return Ok(DetectOllamaResponse {
                installed: true,
                models: Vec::new(),
            })
        }
    };
    let models = json
        .get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    Ok(DetectOllamaResponse {
        installed: true,
        models,
    })
}

/// 一键拉取本地模型（ollama pull <model>），阻塞等待完成
/// 常用推荐：qwen2.5:3b（轻量）/ qwen2.5:7b（均衡）等
#[tauri::command]
pub async fn pull_model(model: String) -> Result<String, AppError> {
    let name = model.trim();
    if name.is_empty() {
        return Err(AppError::ConfigError("模型名不能为空".into()));
    }
    log::info!("[local-ai] 开始拉取模型：{name}");
    let output = tokio::process::Command::new("ollama")
        .args(["pull", name])
        .output()
        .await
        .map_err(|e| {
            AppError::ModelError(format!("启动 ollama 失败（请确认已安装并加入 PATH）：{e}"))
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        log::info!("[local-ai] 模型 {name} 拉取成功");
        Ok(if !stdout.is_empty() { stdout } else { format!("模型 {name} 拉取完成") })
    } else {
        Err(AppError::ModelError(format!(
            "拉取失败：{}",
            if !stderr.is_empty() { stderr } else { "未知错误".to_string() }
        )))
    }
}

/// 显卡显存信息
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
    let output = cmd.output();
    if let Ok(o) = output {
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

/// 设置 Ollama 模型存储路径（用户级环境变量 OLLAMA_MODELS，setx 持久化）
#[tauri::command]
pub fn set_ollama_models_path(path: String) -> Result<String, AppError> {
    let p = path.trim();
    if p.is_empty() {
        return Err(AppError::ConfigError("路径不能为空".into()));
    }
    let mut cmd = std::process::Command::new("setx");
    cmd.args(["OLLAMA_MODELS", p]);
    // 隐藏控制台窗口
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let out = cmd.output()
        .map_err(|e| AppError::ConfigError(format!("设置环境变量失败：{e}")))?;
    if out.status.success() {
        log::info!("[local-ai] 已设置 OLLAMA_MODELS = {p}");
        Ok(format!("✅ 已设置模型存储路径（重启 Ollama 后生效）：{p}"))
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(AppError::ConfigError(format!("设置失败：{stderr}")))
    }
}
