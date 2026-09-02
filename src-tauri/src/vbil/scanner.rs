// 《铃·记忆体》VBIL 模块 —— 形象扫描器
//
// 用 winapi 枚举所有可见顶层窗口（EnumWindows），提取标题/类名/进程名，
// 按关键词过滤出疑似虚拟形象，供前端展示与用户确认。

use serde::Serialize;

/// 疑似虚拟形象窗口
#[derive(Debug, Clone, Serialize)]
pub struct SuspectedAvatar {
    /// 窗口句柄（十进制）
    pub hwnd: usize,
    /// 窗口标题
    pub title: String,
    /// 窗口类名
    pub class: String,
    /// 进程名
    pub process: String,
}

/// 关键词：标题 / 类名 / 进程名命中即视为疑似虚拟形象
const KEYWORDS: &[&str] = &[
    "SSP", "Live2D", "live2d", "Live2DViewerEX", "天选姬", "桌宠", "桌面精灵", "虚拟形象",
    "DesktopMate", "VPet", "Desktop Pet", "Shimeji", "sakura", "春菜", "桌宠",
];

/// 扫描所有顶层窗口，返回疑似虚拟形象列表
pub fn scan_windows() -> Vec<SuspectedAvatar> {
    #[cfg(target_os = "windows")]
    {
        use winapi::shared::minwindef::{BOOL, LPARAM, TRUE};
        use winapi::shared::windef::HWND;
        use winapi::um::winuser::EnumWindows;

        // EnumWindows 回调：把命中的窗口收集进 lparam 指向的 Vec
        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let entries = &mut *(lparam as *mut Vec<SuspectedAvatar>);
            if let Some(entry) = inspect_window(hwnd) {
                entries.push(entry);
            }
            TRUE // 继续枚举
        }

        let mut entries: Vec<SuspectedAvatar> = Vec::new();
        unsafe {
            EnumWindows(
                Some(enum_proc),
                &mut entries as *mut Vec<SuspectedAvatar> as LPARAM,
            );
        }
        entries
    }
    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}

/// 检查单个窗口，命中关键词则返回疑似虚拟形象
#[cfg(target_os = "windows")]
fn inspect_window(hwnd: winapi::shared::windef::HWND) -> Option<SuspectedAvatar> {
    use winapi::shared::minwindef::{DWORD, FALSE, MAX_PATH};
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::winbase::QueryFullProcessImageNameW;
    use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;
    use winapi::um::winuser::{
        GetClassNameW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };

    unsafe {
        // 只枚举可见窗口，排除后台/隐藏窗口
        if IsWindowVisible(hwnd) == 0 {
            return None;
        }

        // 1) 窗口标题
        let mut title_buf = [0u16; 512];
        GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        let title = String::from_utf16_lossy(&title_buf)
            .trim_end_matches('\0')
            .to_string();

        // 2) 窗口类名
        let mut class_buf = [0u16; 256];
        GetClassNameW(hwnd, class_buf.as_mut_ptr(), class_buf.len() as i32);
        let class = String::from_utf16_lossy(&class_buf)
            .trim_end_matches('\0')
            .to_string();

        // 3) 进程名
        let mut pid: DWORD = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let mut process = String::from("unknown");
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
        if !handle.is_null() {
            let mut path_buf = [0u16; MAX_PATH];
            let mut size: DWORD = path_buf.len() as DWORD;
            if QueryFullProcessImageNameW(handle, 0, path_buf.as_mut_ptr(), &mut size) != FALSE {
                let path = String::from_utf16_lossy(&path_buf[..size as usize]);
                if let Some(name) = std::path::Path::new(&path).file_name() {
                    process = name.to_string_lossy().to_string();
                }
            }
            CloseHandle(handle);
        }

        // 过滤：标题 / 类名 / 进程名命中关键词
        let haystack = format!("{} {} {}", title, class, process);
        let lower = haystack.to_lowercase();
        if KEYWORDS
            .iter()
            .any(|k| lower.contains(&k.to_lowercase()))
        {
            return Some(SuspectedAvatar {
                hwnd: hwnd as usize,
                title,
                class,
                process,
            });
        }
        None
    }
}
