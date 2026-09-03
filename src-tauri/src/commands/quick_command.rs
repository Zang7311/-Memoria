// 《铃·记忆体》快捷指令系统（AI-9）
//  - 系统工具三件套：电源模式切换 / 音量设置 / 启动音乐（独立命令，设置页或快捷指令均可调用）
//  - 快捷指令数据模型：QuickCommand { id, name, steps:[{tool,input}], say }，持久化于 config.json
//  - 管理 IPC：list_quick_commands / save_quick_command / delete_quick_command
//  - 执行 IPC：execute_quick_command —— 按顺序执行 steps（系统工具走本模块，其余复用工具箱 execute）
use crate::config;
use crate::desktop::toolbox;
use crate::error::AppError;
use crate::types::{
    DeleteQuickCommandRequest, ExecuteQuickCommandRequest, ExecuteQuickCommandResponse,
    ListQuickCommandsResponse, QuickCommandStep, SaveQuickCommandRequest,
};
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Manager};

// ==================== 通用执行辅助 ====================

/// 获取资源目录（开发模式 = src-tauri，打包模式 = 安装目录 resources；供工具箱预设加载使用）
fn resource_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .resource_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// 把 PowerShell 脚本编码为 -EncodedCommand 参数（UTF-16LE + base64，规避中文/引号/GBK 坑）
fn ps_encoded_command(script: &str) -> String {
    use base64::Engine;
    let bytes: Vec<u8> = script
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// 异步执行一段 PowerShell 脚本（隐藏控制台，15 秒超时），返回 stdout 文本
async fn run_powershell(script: &str, vol: Option<&str>) -> Result<String, String> {
    let enc = ps_encoded_command(script);
    let mut cmd = tokio::process::Command::new("powershell");
    cmd.arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-EncodedCommand")
        .arg(&enc);
    if let Some(v) = vol {
        cmd.env("VOL", v);
    }
    cmd.kill_on_drop(true);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW 隐藏控制台
    }
    match tokio::time::timeout(Duration::from_secs(15), cmd.output()).await {
        Err(_) => Err("命令执行超时（15 秒）".into()),
        Ok(Err(e)) => Err(e.to_string()),
        Ok(Ok(out)) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if out.status.success() {
                Ok(stdout)
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                Err(if stderr.is_empty() { stdout } else { stderr })
            }
        }
    }
}

/// 异步执行 powercfg 命令（隐藏控制台，10 秒超时），返回 stdout 文本
async fn run_powercfg(args: &[&str]) -> Result<String, String> {
    let mut cmd = tokio::process::Command::new("powercfg");
    cmd.args(args);
    cmd.kill_on_drop(true);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000);
    }
    match tokio::time::timeout(Duration::from_secs(10), cmd.output()).await {
        Err(_) => Err("命令执行超时（10 秒）".into()),
        Ok(Err(e)) => Err(e.to_string()),
        Ok(Ok(out)) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if out.status.success() {
                Ok(stdout)
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                Err(if stderr.is_empty() { stdout } else { stderr })
            }
        }
    }
}

// ==================== 系统工具三件套 ====================

/// 电源模式 → powercfg 方案别名 + 中文名
fn power_scheme(mode: &str) -> Option<(&'static str, &'static str)> {
    match mode {
        "balanced" | "power-balanced" => Some(("SCHEME_BALANCED", "平衡")),
        "high" | "power-high" => Some(("SCHEME_MIN", "高性能")),
        "power-saver" | "saver" => Some(("SCHEME_MAX", "节能")),
        _ => None,
    }
}

/// 切换系统电源计划（内部实现，返回友好中文说明）
async fn set_power_inner(mode: &str) -> Result<String, AppError> {
    let (scheme, name) = power_scheme(mode)
        .ok_or_else(|| AppError::ToolboxError(format!("未知电源模式：{mode}")))?;
    run_powercfg(&["/setactive", scheme])
        .await
        .map_err(AppError::ToolboxError)?;
    Ok(format!("已切换到「{name}」电源计划"))
}

/// 设置系统主音量（内部实现，0-100；经 waveOutSetVolume 调系统主音量）
async fn set_volume_inner(level: &str) -> Result<String, AppError> {
    let vol: i32 = level
        .trim()
        .parse()
        .map_err(|_| AppError::ToolboxError(format!("音量必须是 0-100 的数字，收到：{level}")))?;
    if !(0..=100).contains(&vol) {
        return Err(AppError::ToolboxError(format!("音量必须在 0-100 之间，收到：{vol}")));
    }
    // 纯 ASCII 脚本（走 UTF-16LE base64，避免 GBK 乱码）：waveOutSetVolume 同时设左右声道
    let script = r#"
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class Vol{[DllImport("winmm.dll")]public static extern int waveOutSetVolume(IntPtr h,uint v);}'
$v=[int]$env:VOL
if($v -lt 0){$v=0}; if($v -gt 100){$v=100}
$scalar=[int][math]::Round($v*65535/100)
$both=[uint32](([int64]$scalar * 65536) + $scalar)
[Vol]::waveOutSetVolume([IntPtr]::Zero,$both)|Out-Null
Write-Output ("volume="+$v)
"#;
    let out = run_powershell(script, Some(level))
        .await
        .map_err(AppError::ToolboxError)?;
    Ok(format!("系统主音量已设为 {vol}%（{}）", out.trim()))
}

/// 启动音乐（内部实现）：指定文件路径用默认程序打开，空路径打开用户「音乐」目录
async fn play_music_inner(path: Option<&str>) -> Result<String, AppError> {
    let target = match path {
        Some(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => std::env::var("USERPROFILE")
            .map(|u| format!("{u}\\Music"))
            .unwrap_or_default(),
    };
    let mut cmd = tokio::process::Command::new("cmd");
    cmd.arg("/C").arg("start").arg("").arg(&target);
    cmd.kill_on_drop(true);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000);
    }
    match tokio::time::timeout(Duration::from_secs(15), cmd.output()).await {
        Err(_) => Err(AppError::ToolboxError("启动音乐超时".into())),
        Ok(Err(e)) => Err(AppError::ToolboxError(e.to_string())),
        Ok(Ok(_)) => Ok(format!("已启动播放：{target}")),
    }
}

// ==================== 快捷指令：数据管理 ====================

/// 列出所有快捷指令
#[tauri::command]
pub fn list_quick_commands() -> ListQuickCommandsResponse {
    let cfg = config::store::get_config();
    ListQuickCommandsResponse {
        commands: cfg.quick_commands,
    }
}

/// 新增或更新一条快捷指令
#[tauri::command]
pub fn save_quick_command(request: SaveQuickCommandRequest) -> Result<(), AppError> {
    let mut cfg = config::store::get_config();
    if let Some(existing) = cfg
        .quick_commands
        .iter_mut()
        .find(|c| c.id == request.command.id)
    {
        *existing = request.command;
    } else {
        cfg.quick_commands.push(request.command);
    }
    config::store::set_config(cfg)
}

/// 删除一条快捷指令
#[tauri::command]
pub fn delete_quick_command(request: DeleteQuickCommandRequest) -> Result<(), AppError> {
    let mut cfg = config::store::get_config();
    cfg.quick_commands
        .retain(|c| c.id != request.command_id);
    config::store::set_config(cfg)
}

// ==================== 快捷指令：执行 ====================

/// 执行单个动作，返回可展示的结果说明（pub(crate)：工具箱「组合工具」亦复用）
pub(crate) async fn execute_step(app: &AppHandle, step: &QuickCommandStep) -> String {
    let tool = step.tool.as_str();
    match tool {
        // 系统工具：音量
        "volume" => match step.input.as_deref() {
            Some(v) => match set_volume_inner(v).await {
                Ok(m) => m,
                Err(e) => format!("设置音量失败：{e}"),
            },
            None => "设置音量：未提供数值（0-100）".to_string(),
        },
        // 系统工具：音乐
        "music" => match play_music_inner(step.input.as_deref()).await {
            Ok(m) => m,
            Err(e) => format!("启动音乐失败：{e}"),
        },
        // 系统工具：电源（power-balanced / power-high / power-saver）
        t if t.starts_with("power-") => {
            let mode = &t["power-".len()..];
            match set_power_inner(mode).await {
                Ok(m) => m,
                Err(e) => format!("切换电源失败：{e}"),
            }
        }
        // 其余：复用工具箱工具执行
        other => {
            let dir = resource_dir(app);
            match toolbox::find_item(&dir, other) {
                Some(item) => match toolbox::execute(&item, step.input.clone()).await {
                    Ok(resp) => {
                        if resp.success {
                            resp.output
                                .unwrap_or_else(|| format!("已执行「{}」", item.name))
                        } else {
                            format!(
                                "执行「{}」失败：{}",
                                item.name,
                                resp.error.unwrap_or_default()
                            )
                        }
                    }
                    Err(e) => format!("执行「{}」出错：{e}", item.name),
                },
                None => format!("未知动作：{other}"),
            }
        }
    }
}

/// 按顺序执行一条快捷指令的所有动作
#[tauri::command]
pub async fn execute_quick_command(
    app: AppHandle,
    request: ExecuteQuickCommandRequest,
) -> Result<ExecuteQuickCommandResponse, AppError> {
    let cmd = {
        let cfg = config::store::get_config();
        cfg.quick_commands
            .iter()
            .find(|c| c.id == request.command_id)
            .cloned()
            .ok_or_else(|| {
                AppError::ConfigError(format!("快捷指令不存在：{}", request.command_id))
            })?
    };

    let mut results = Vec::with_capacity(cmd.steps.len());
    for step in &cmd.steps {
        results.push(execute_step(&app, step).await);
    }

    Ok(ExecuteQuickCommandResponse {
        success: true,
        results,
        say: cmd.say.clone(),
        error: None,
    })
}

// ==================== 系统工具：独立命令 ====================

/// 切换系统电源计划（mode: balanced | high | power-saver）
#[tauri::command]
pub async fn set_power_mode(mode: String) -> Result<String, AppError> {
    set_power_inner(&mode).await
}

/// 设置系统主音量（0-100）
#[tauri::command]
pub async fn set_volume(level: u8) -> Result<String, AppError> {
    set_volume_inner(&level.to_string()).await
}

/// 启动音乐（path 为空则打开用户音乐目录）
#[tauri::command]
pub async fn play_music(path: Option<String>) -> Result<String, AppError> {
    play_music_inner(path.as_deref()).await
}
