// 《铃·记忆体》原生桌宠托管（Live2D 独立进程 pet-native）
//
// 为什么是一个独立进程：应用内那套悬浮球走 WebView2，高缩放下透明小窗的画面会错位；
// 原生桌宠是 Win32 分层窗口 + 自己的 Live2D 渲染 DLL，不经过 WebView。
//
// 本模块管三件事：
//   1. 找到随安装包分发进来的 pet\pet-native.exe
//   2. 按配置开关拉起 / 关掉它（配置键 native_pet_enabled）
//   3. 应用退出时收尸，不留孤儿进程（桌宠自己也盯着父进程，双保险）
use crate::config;
use crate::error::AppError;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// 托管的桌宠子进程（None = 我们没在管它）。
/// 用户自己双击快捷方式启动的那只不归这里管 —— 桌宠自带单实例互斥量，不会出现两只。
static PET_CHILD: Mutex<Option<Child>> = Mutex::new(None);

/// 设置页用的一小份状态（前端据此显示「已运行 / 未启动 / 找不到程序」）
#[derive(Debug, Clone, Serialize)]
pub struct NativePetStatus {
    /// 托管的桌宠是否在跑
    pub running: bool,
    /// 进程 ID
    pub pid: Option<u32>,
    /// 有没有找到桌宠程序（找不到 = 开发态没构建，或安装包缺资源）
    pub found: bool,
    /// 桌宠程序路径
    pub exe_path: String,
    /// 给用户看的一句话
    pub message: String,
}

impl NativePetStatus {
    fn not_found() -> Self {
        Self {
            running: false,
            pid: None,
            found: false,
            exe_path: String::new(),
            message: "没找到桌宠程序（安装包里应该有 pet\\pet-native.exe）".to_string(),
        }
    }

    fn idle(exe: &Path) -> Self {
        Self {
            running: false,
            pid: None,
            found: true,
            exe_path: exe.display().to_string(),
            message: "桌宠没在运行".to_string(),
        }
    }

    fn running(exe: &Path, pid: u32) -> Self {
        Self {
            running: true,
            pid: Some(pid),
            found: true,
            exe_path: exe.display().to_string(),
            message: format!("桌宠运行中（进程 {pid}）"),
        }
    }
}

/// 桌宠程序候选路径：先找安装包资源目录，再找开发态 target\release
fn pet_exe_candidates(app: &AppHandle) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(dir) = app.path().resource_dir() {
        out.push(dir.join("pet").join("pet-native.exe"));
        out.push(dir.join("pet-native.exe"));
    }
    // 开发态（pnpm tauri dev）资源目录里没有 pet\，直接用刚编译出来的那个
    if cfg!(debug_assertions) {
        out.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("pet-native")
                .join("target")
                .join("release")
                .join("pet-native.exe"),
        );
    }
    out
}

fn locate_pet_exe(app: &AppHandle) -> Option<PathBuf> {
    pet_exe_candidates(app).into_iter().find(|p| p.is_file())
}

/// 桌宠的配置/位置文件放哪：跟应用的配置同目录（exe 所在目录可能只读）
fn pet_data_dir() -> PathBuf {
    let dir = config::data_dir().join("pet");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// 查当前托管的桌宠状态（顺手清理已经退出的进程）
pub fn status(app: &AppHandle) -> NativePetStatus {
    let exe = locate_pet_exe(app);
    let Some(exe) = exe else {
        return NativePetStatus::not_found();
    };
    let mut guard = PET_CHILD.lock().unwrap_or_else(|e| e.into_inner());
    match guard.as_mut() {
        Some(child) => match child.try_wait() {
            Ok(None) => NativePetStatus::running(&exe, child.id()),
            _ => {
                // 自己退了（用户右键退出 / 崩溃），把句柄清掉
                *guard = None;
                NativePetStatus::idle(&exe)
            }
        },
        None => NativePetStatus::idle(&exe),
    }
}

/// 拉起桌宠（已在运行则原样返回状态）
pub fn start(app: &AppHandle) -> Result<NativePetStatus, AppError> {
    let Some(exe) = locate_pet_exe(app) else {
        return Ok(NativePetStatus::not_found());
    };
    {
        let mut guard = PET_CHILD.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(child) = guard.as_mut() {
            if matches!(child.try_wait(), Ok(None)) {
                return Ok(NativePetStatus::running(&exe, child.id()));
            }
        }
        *guard = None;
    }

    let data_dir = pet_data_dir();
    // 桌宠的诊断输出写文件：应用托管时不带控制台，出问题只能靠这个看
    let log_path = data_dir.join("pet.log");
    let mut cmd = Command::new(&exe);
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    cmd.arg("--data-dir")
        .arg(&data_dir)
        .arg("--parent-pid")
        .arg(std::process::id().to_string())
        .stdin(Stdio::null());
    match std::fs::File::create(&log_path) {
        Ok(file) => {
            let cloned = file.try_clone();
            cmd.stdout(Stdio::from(file));
            cmd.stderr(match cloned {
                Ok(f) => Stdio::from(f),
                Err(_) => Stdio::null(),
            });
        }
        Err(_) => {
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
        }
    }
    // 桌宠是控制台程序，托管启动时别让它闪一下黑框
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::InternalError(format!("拉起桌宠失败：{e}")))?;
    let pid = child.id();
    // 抢不到单实例（桌面上已经有一只）或缺 DLL 时它会立刻退出，等一下才能如实报告
    std::thread::sleep(Duration::from_millis(300));
    match child.try_wait() {
        Ok(Some(code)) => {
            let mut status = NativePetStatus::idle(&exe);
            status.message = match code.code() {
                // 桌宠约定：抢不到单实例时退出码 2
                Some(2) => "桌面上已经有一只桌宠在运行，这次没有重复启动".to_string(),
                Some(c) => format!(
                    "桌宠启动后立刻退出了（退出码 {c}），日志见 {}",
                    log_path.display()
                ),
                None => "桌宠启动后立刻退出了，日志见 pet.log".to_string(),
            };
            Ok(status)
        }
        _ => {
            let mut guard = PET_CHILD.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(child);
            log::info!("[native-pet] 已启动：{} (pid {pid})", exe.display());
            Ok(NativePetStatus::running(&exe, pid))
        }
    }
}

/// 关掉托管的桌宠
pub fn stop() -> NativePetStatus {
    let mut guard = PET_CHILD.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut child) = guard.take() {
        let pid = child.id();
        let _ = child.kill();
        let _ = child.wait();
        log::info!("[native-pet] 已关闭 pid {pid}");
    }
    NativePetStatus {
        running: false,
        pid: None,
        found: true,
        exe_path: String::new(),
        message: "桌宠已关闭".to_string(),
    }
}

/// 按配置开关对齐：开着就确保在跑，关着就确保停掉（应用启动 / 设置页切换都走它）
pub fn sync_with_config(app: &AppHandle) -> Result<NativePetStatus, AppError> {
    if config::store::get_config().native_pet_enabled {
        start(app)
    } else {
        stop();
        Ok(status(app))
    }
}

/// 应用退出时收尸（被强杀时靠桌宠自己的父进程看门狗）
pub fn shutdown() {
    let mut guard = PET_CHILD.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

// —— IPC：设置页用 ——

/// 查询状态
#[tauri::command]
pub fn native_pet_status(app: AppHandle) -> NativePetStatus {
    status(&app)
}

/// 按配置开关启动/停止（设置页开关改完配置后调用）
#[tauri::command]
pub fn native_pet_sync(app: AppHandle) -> Result<NativePetStatus, AppError> {
    sync_with_config(&app)
}

/// 重启桌宠（换模型 / 卡住时用）
#[tauri::command]
pub fn native_pet_restart(app: AppHandle) -> Result<NativePetStatus, AppError> {
    stop();
    start(&app)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 配置目录拼出来的桌宠状态目录应该在配置目录下面（不是 exe 目录）
    #[test]
    fn pet_data_dir_is_under_config_dir() {
        let dir = pet_data_dir();
        assert!(dir.starts_with(config::data_dir()));
        assert!(dir.exists());
    }

    /// 找不到程序时状态要明确说「没找到」，不能让前端以为只是没启动
    #[test]
    fn not_found_status_is_explicit() {
        let st = NativePetStatus::not_found();
        assert!(!st.found);
        assert!(!st.running);
        assert!(st.message.contains("没找到"));
    }
}
