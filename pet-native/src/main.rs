// 铃·桌宠 原生窗口验证程序（pet-native）
//
// 目的：**不经过 WebView2**，验证 Windows 10 + 150% 缩放下能否正确
//   1. 创建透明无边框分层窗口（WS_EX_LAYERED + UpdateLayeredWindow）
//   2. 按 DPI 正确换算「逻辑尺寸 → 物理尺寸」
//   3. 透明像素区域自动穿透鼠标，可见区域可拖动/点击
//
// 第一版只画一张程序生成的透明测试图（粉色圆 + 白色十字），不接 Live2D。
// 通过后再进入 Cubism Native 阶段。
//
// 运行：cargo run（会开一个控制台打印 DPI 诊断 + 一个透明圆窗口）
// 交互：圆内拖动 = 移动；圆内单击 = 标题变为 clicked；右键 = 退出

use std::mem::size_of;
use std::ptr::null_mut;

use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HBRUSH, HBITMAP, HDC, HWND, POINT, RECT, SIZE};
use winapi::um::libloaderapi::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use winapi::um::wingdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DeleteObject, DIB_RGB_COLORS, GetDeviceCaps, SelectObject,
    LOGPIXELSX,
};
use winapi::um::winuser::*;

/// 桌宠的**逻辑**尺寸（150% 缩放下物理应为 300）
const LOGIC_SIZE: i32 = 200;

struct Probe {
    bgra: Vec<u8>,
    phys_w: i32,
    phys_h: i32,
    hwnd: HWND,
    hdc_screen: HDC,
    mem_dc: HDC,
    dragging: bool,
    drag_start: (i32, i32),
    win_start: (i32, i32),
    moved: bool,
}

static mut PROBE: Option<Box<Probe>> = None;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

type SetCtxFn = unsafe extern "system" fn(isize) -> i32;

/// 启用 Per-Monitor V2 DPI 感知（失败则退回旧 API）
unsafe fn enable_dpi_awareness() -> bool {
    let user32 = LoadLibraryW(wide("user32.dll").as_ptr());
    if !user32.is_null() {
        let proc = GetProcAddress(user32, b"SetProcessDpiAwarenessContext\0".as_ptr() as *const i8);
        if !proc.is_null() {
            let f: SetCtxFn = std::mem::transmute(proc);
            // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 = -4
            if f(-4) != 0 {
                return true;
            }
        }
    }
    SetProcessDPIAware() != 0
}

/// 生成测试图：透明背景 + 粉色圆 + 白色十字（预乘 alpha 的 BGRA）
fn build_image(size: i32) -> Vec<u8> {
    let mut buf = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 / 2.0;
    let r = size as f32 * 0.46;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - c;
            let dy = y as f32 + 0.5 - c;
            let d = (dx * dx + dy * dy).sqrt();
            // 边缘 1.5px 柔和过渡（顺带验证 alpha 混合）
            let a = ((r - d) / 1.5).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            let cross = (dx.abs() < size as f32 * 0.015 && dy.abs() < r * 0.7)
                || (dy.abs() < size as f32 * 0.015 && dx.abs() < r * 0.7);
            let (cr, cg, cb) = if cross { (255u8, 255u8, 255u8) } else { (255, 122, 148) };
            let al = (a * 255.0) as u8;
            let i = ((y * size + x) * 4) as usize;
            // 预乘 alpha：BGRA
            buf[i] = (cb as u32 * al as u32 / 255) as u8;
            buf[i + 1] = (cg as u32 * al as u32 / 255) as u8;
            buf[i + 2] = (cr as u32 * al as u32 / 255) as u8;
            buf[i + 3] = al;
        }
    }
    buf
}

/// 把整张位图贴到分层窗口
unsafe fn paint(p: &Probe) {
    let mut bi: BITMAPINFO = std::mem::zeroed();
    bi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    bi.bmiHeader.biWidth = p.phys_w;
    bi.bmiHeader.biHeight = -p.phys_h; // 负数 = top-down
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = 32;
    bi.bmiHeader.biCompression = BI_RGB;

    let mut bits: *mut winapi::ctypes::c_void = null_mut();
    let bmp: HBITMAP = CreateDIBSection(p.mem_dc, &bi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
    if bmp.is_null() {
        return;
    }
    std::ptr::copy_nonoverlapping(p.bgra.as_ptr(), bits as *mut u8, p.bgra.len());
    let old = SelectObject(p.mem_dc, bmp as *mut _);

    let mut wr: RECT = std::mem::zeroed();
    GetWindowRect(p.hwnd, &mut wr);
    let mut pos = POINT { x: wr.left, y: wr.top };
    let mut size = SIZE { cx: p.phys_w, cy: p.phys_h };
    let mut src = POINT { x: 0, y: 0 };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    let mut blend = blend;
    UpdateLayeredWindow(
        p.hwnd,
        p.hdc_screen,
        &mut pos,
        &mut size,
        p.mem_dc,
        &mut src,
        0,
        &mut blend,
        ULW_ALPHA,
    );

    SelectObject(p.mem_dc, old);
    DeleteObject(bmp as *mut _);
}

unsafe fn print_diagnostics(p: &Probe, tag: &str) {
    let mut wr: RECT = std::mem::zeroed();
    let mut cr: RECT = std::mem::zeroed();
    GetWindowRect(p.hwnd, &mut wr);
    GetClientRect(p.hwnd, &mut cr);
    let dpi = GetDpiForWindow(p.hwnd);
    let scale = if dpi > 0 { dpi as f64 / 96.0 } else { 1.0 };
    println!(
        "[{tag}] hwnd dpi={dpi} scale={scale:.2} phys={}x{} window_rect=({},{})-({},{}) client={}x{} bitmap={}x{}",
        p.phys_w, p.phys_h,
        wr.left, wr.top, wr.right, wr.bottom,
        cr.right - cr.left, cr.bottom - cr.top,
        p.phys_w, p.phys_h
    );
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_LBUTTONDOWN => {
            if let Some(p) = &mut PROBE {
                let mut pt = POINT { x: 0, y: 0 };
                GetCursorPos(&mut pt);
                let mut wr: RECT = std::mem::zeroed();
                GetWindowRect(hwnd, &mut wr);
                p.dragging = true;
                p.moved = false;
                p.drag_start = (pt.x, pt.y);
                p.win_start = (wr.left, wr.top);
                SetCapture(hwnd);
                println!("[input] left down at ({},{})", pt.x, pt.y);
            }
            0
        }
        WM_MOUSEMOVE => {
            if let Some(p) = &mut PROBE {
                if p.dragging {
                    let mut pt = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut pt);
                    let dx = pt.x - p.drag_start.0;
                    let dy = pt.y - p.drag_start.1;
                    if !p.moved && (dx.abs() > 3 || dy.abs() > 3) {
                        p.moved = true;
                    }
                    if p.moved {
                        SetWindowPos(
                            hwnd,
                            HWND_TOPMOST,
                            p.win_start.0 + dx,
                            p.win_start.1 + dy,
                            p.phys_w,
                            p.phys_h,
                            SWP_NOACTIVATE,
                        );
                    }
                }
            }
            0
        }
        WM_LBUTTONUP => {
            if let Some(p) = &mut PROBE {
                if p.dragging {
                    ReleaseCapture();
                    p.dragging = false;
                    let mut wr: RECT = std::mem::zeroed();
                    GetWindowRect(hwnd, &mut wr);
                    if p.moved {
                        println!("[input] drag end -> window at ({},{})", wr.left, wr.top);
                    } else {
                        println!("[input] click (no drag)");
                        SetWindowTextW(hwnd, wide("铃·桌宠 原生验证 — clicked").as_ptr());
                    }
                }
            }
            0
        }
        WM_RBUTTONUP => {
            println!("[input] right click -> quit");
            PostQuitMessage(0);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn main() {
    unsafe {
        let aware = enable_dpi_awareness();
        println!("[pet-native] per-monitor v2 = {aware}");

        // 屏幕与缩放
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        let hdc = GetDC(null_mut());
        let dpi = GetDeviceCaps(hdc, LOGPIXELSX);
        let scale = dpi as f64 / 96.0;
        println!("[pet-native] screen physical = {sw}x{sh}, LOGPIXELSX dpi = {dpi}, scale = {scale:.2}");

        // 逻辑 → 物理
        let phys = (LOGIC_SIZE as f64 * scale).round() as i32;
        let x = (100.0 * scale).round() as i32;
        let y = (100.0 * scale).round() as i32;
        println!("[pet-native] target: logic {LOGIC_SIZE}x{LOGIC_SIZE} -> physical {phys}x{phys} at physical ({x},{y})");

        let bgra = build_image(phys);

        let hinstance = GetModuleHandleW(std::ptr::null());
        let class_name = wide("PetNativeProbe");
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_HAND),
            hbrBackground: null_mut() as HBRUSH,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: null_mut(),
        };
        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            wide("铃·桌宠 原生验证").as_ptr(),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            phys,
            phys,
            null_mut(),
            null_mut(),
            hinstance,
            null_mut(),
        );
        if hwnd.is_null() {
            println!("[pet-native] CreateWindowExW failed");
            return;
        }

        let mem_dc = CreateCompatibleDC(hdc);
        let probe = Box::new(Probe {
            bgra,
            phys_w: phys,
            phys_h: phys,
            hwnd,
            hdc_screen: hdc,
            mem_dc,
            dragging: false,
            drag_start: (0, 0),
            win_start: (0, 0),
            moved: false,
        });
        PROBE = Some(probe);
        paint(PROBE.as_ref().unwrap());
        print_diagnostics(PROBE.as_ref().unwrap(), "created");

        println!("[pet-native] 操作：圆内拖动=移动；圆内单击=标题变 clicked；右键=退出");

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        println!("[pet-native] exit");
    }
}
