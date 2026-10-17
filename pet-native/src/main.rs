// 铃·桌宠 原生实现 v0.2（Win32 分层窗口，不经过 WebView2）
//
// 已验证：150% 缩放下窗口定位、DPI 换算、透明、鼠标交互全部正确。
// 本版本加入：真实头像 PNG、待机浮动、单击说话气泡、长按摸头、拖动、
//            松手掉落（带弹跳）、右键菜单（穿透/重置位置/退出）、位置记忆。
//
// 素材：assets/avatar.png（从项目 src/assets/ball_avatar.png 复制）
// 运行：cargo run（控制台会打印诊断；右键菜单退出）

use std::mem::size_of;
use std::ptr::null_mut;
use std::time::Instant;

use image::imageops::FilterType;
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HBITMAP, HDC, HWND, POINT, RECT, SIZE};
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::libloaderapi::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

/// 角色区逻辑尺寸（150% 下物理 300）
const ROLE: i32 = 200;
/// 顶部气泡区逻辑高度（物理 90）
const BUBBLE_H: i32 = 60;
const WIN_LOGIC_H: i32 = ROLE + BUBBLE_H;

const FALL_GRAVITY: f32 = 34.0;
const SLEEP_AFTER_SECS: u64 = 300;
const LONG_PRESS_MS: u32 = 700;

#[derive(PartialEq, Clone, Copy)]
enum State {
    Idle,
    Talk,
    Sleep,
}

struct Pet {
    hwnd: HWND,
    hdc_screen: HDC,
    mem_dc: HDC,
    bitmap: HBITMAP,
    bits: *mut u8,
    phys_w: i32,
    phys_h: i32,
    role_phys: i32,
    bubble_phys_h: i32,
    scale: f32,
    avatar: Vec<u8>, // 预乘 BGRA，尺寸 role_phys x role_phys
    state: State,
    bubble: String,
    bubble_until: Option<Instant>,
    dragging: bool,
    moved: bool,
    long_press_fired: bool,
    drag_start: (i32, i32),
    win_start: (i32, i32),
    falling: bool,
    fall_v: f32,
    last_active: Instant,
    click_through: bool,
    zzz_phase: f32,
}

static mut PET: Option<Box<Pet>> = None;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

type SetCtxFn = unsafe extern "system" fn(isize) -> i32;

unsafe fn enable_dpi_awareness() -> bool {
    let user32 = LoadLibraryW(wide("user32.dll").as_ptr());
    if !user32.is_null() {
        let proc = GetProcAddress(user32, b"SetProcessDpiAwarenessContext\0".as_ptr() as *const i8);
        if !proc.is_null() {
            let f: SetCtxFn = std::mem::transmute(proc);
            if f(-4) != 0 {
                return true;
            }
        }
    }
    SetProcessDPIAware() != 0
}

/// 载入头像：缩放 + 预乘 alpha（BGRA）
fn load_avatar(path: &str, size: i32) -> Vec<u8> {
    match image::open(path) {
        Ok(img) => {
            let rgba = img.resize_exact(size as u32, size as u32, FilterType::Lanczos3).to_rgba8();
            let src = rgba.as_raw();
            let mut out = vec![0u8; src.len()];
            for i in (0..src.len()).step_by(4) {
                let a = src[i + 3] as u32;
                out[i] = (src[i + 2] as u32 * a / 255) as u8;
                out[i + 1] = (src[i + 1] as u32 * a / 255) as u8;
                out[i + 2] = (src[i] as u32 * a / 255) as u8;
                out[i + 3] = src[i + 3];
            }
            out
        }
        Err(e) => {
            println!("[pet] 头像加载失败 {path}: {e}（用空图继续）");
            vec![0u8; (size * size * 4) as usize]
        }
    }
}

unsafe fn create_backbuffer(p: &mut Pet) {
    let mut bi: BITMAPINFO = std::mem::zeroed();
    bi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    bi.bmiHeader.biWidth = p.phys_w;
    bi.bmiHeader.biHeight = -p.phys_h;
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = 32;
    bi.bmiHeader.biCompression = BI_RGB;
    let mut bits: *mut winapi::ctypes::c_void = null_mut();
    p.bitmap = CreateDIBSection(p.mem_dc, &bi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
    p.bits = bits as *mut u8;
    SelectObject(p.mem_dc, p.bitmap as *mut _);
}

unsafe fn blit(p: &Pet) {
    let mut pos = POINT { x: 0, y: 0 };
    let mut size = SIZE { cx: p.phys_w, cy: p.phys_h };
    let mut src = POINT { x: 0, y: 0 };
    let mut blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    let mut wr: RECT = std::mem::zeroed();
    GetWindowRect(p.hwnd, &mut wr);
    pos.x = wr.left;
    pos.y = wr.top;

    // 与已验证的探针程序相同的贴图方式：每帧新建 DIB 再 UpdateLayeredWindow。
    // （常驻 DIB 的那版实测屏幕上看不到，虽然内存里数据是好的。）
    let mut bi: BITMAPINFO = std::mem::zeroed();
    bi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    bi.bmiHeader.biWidth = p.phys_w;
    bi.bmiHeader.biHeight = -p.phys_h;
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = 32;
    bi.bmiHeader.biCompression = BI_RGB;
    let mut bits: *mut winapi::ctypes::c_void = null_mut();
    let bmp = CreateDIBSection(p.mem_dc, &bi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
    if bmp.is_null() {
        println!("[pet] CreateDIBSection 失败");
        return;
    }
    let len = (p.phys_w * p.phys_h * 4) as usize;
    std::ptr::copy_nonoverlapping(p.bits, bits as *mut u8, len);
    let old = SelectObject(p.mem_dc, bmp as *mut _);

    let ok = UpdateLayeredWindow(
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
    if ok == 0 {
        println!("[pet] UpdateLayeredWindow FAILED err={}", GetLastError());
    }

    SelectObject(p.mem_dc, old);
    DeleteObject(bmp as *mut _);
}

unsafe fn clear(buf: *mut u8, len: usize) {
    std::ptr::write_bytes(buf, 0, len);
}

/// 画圆角矩形（预乘不透明）
unsafe fn fill_round_rect(buf: *mut u8, w: i32, x0: i32, y0: i32, rw: i32, rh: i32, radius: i32, color: (u8, u8, u8)) {
    for y in y0..(y0 + rh) {
        for x in x0..(x0 + rw) {
            let dx = if x < x0 + radius { x0 + radius - x } else if x > x0 + rw - radius { x - (x0 + rw - radius) } else { 0 };
            let dy = if y < y0 + radius { y0 + radius - y } else if y > y0 + rh - radius { y - (y0 + rh - radius) } else { 0 };
            if dx * dx + dy * dy > radius * radius {
                continue;
            }
            let i = ((y * w + x) * 4) as usize;
            *buf.add(i) = color.2;
            *buf.add(i + 1) = color.1;
            *buf.add(i + 2) = color.0;
            *buf.add(i + 3) = 255;
        }
    }
}

unsafe fn draw_avatar(buf: *mut u8, w: i32, h: i32, avatar: &[u8], role: i32, y_off: i32) {
    for y in 0..role {
        let dy = y + y_off;
        if dy < 0 || dy >= h {
            continue;
        }
        for x in 0..role {
            if x >= w {
                break;
            }
            let si = ((y * role + x) * 4) as usize;
            let di = ((dy * w + x) * 4) as usize;
            // 预乘源，直接覆盖（背景已清零）
            *buf.add(di) = avatar[si];
            *buf.add(di + 1) = avatar[si + 1];
            *buf.add(di + 2) = avatar[si + 2];
            *buf.add(di + 3) = avatar[si + 3];
        }
    }
}

unsafe fn draw_frame(p: &mut Pet) {
    let w = p.phys_w;
    let h = p.phys_h;
    clear(p.bits, (w * h * 4) as usize);

    // 待机浮动：正弦上下
    let t = Instant::now();
    let phase = (t.elapsed().as_millis() % 3000) as f32 / 3000.0 * std::f32::consts::TAU;
    let float_off = ((phase.sin()) * 3.0 * p.scale) as i32;

    // 角色区在窗口底部
    let role_top = p.bubble_phys_h;
    let avatar = &p.avatar;
    let mut tmp = vec![0u8; (w * h * 4) as usize];
    // 角色绘制在 (0, role_top + float_off)，宽度 = role_phys
    for y in 0..p.role_phys {
        let dy = role_top + y + float_off;
        if dy < 0 || dy >= h {
            continue;
        }
        for x in 0..p.role_phys.min(w) {
            let si = ((y * p.role_phys + x) * 4) as usize;
            let di = ((dy * w + x) * 4) as usize;
            if si + 3 >= avatar.len() || di + 3 >= tmp.len() {
                continue;
            }
            tmp[di] = avatar[si];
            tmp[di + 1] = avatar[si + 1];
            tmp[di + 2] = avatar[si + 2];
            tmp[di + 3] = avatar[si + 3];
        }
    }
    std::ptr::copy_nonoverlapping(tmp.as_ptr(), p.bits, tmp.len());

    // 气泡
    if !p.bubble.is_empty() {
        let bx = (10.0 * p.scale) as i32;
        let by = (6.0 * p.scale) as i32;
        let bw = w - bx * 2;
        let bh = p.bubble_phys_h - by - (8.0 * p.scale) as i32;
        fill_round_rect(p.bits, w, bx, by, bw, bh, (14.0 * p.scale) as i32, (34, 32, 36));
        // 文字用 GDI 画，再把气泡区 alpha 补齐
        let font = CreateFontW(
            -((15.0 * p.scale) as i32), 0, 0, 0, FW_NORMAL as i32, 0, 0, 0, DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_PITCH, wide("Microsoft YaHei").as_ptr(),
        );
        let old = SelectObject(p.mem_dc, font as *mut _);
        SetBkMode(p.mem_dc, TRANSPARENT as i32);
        SetTextColor(p.mem_dc, RGB(238, 230, 231));
        let mut rect = RECT { left: bx + 8, top: by + 4, right: bx + bw - 8, bottom: by + bh - 4 };
        DrawTextW(p.mem_dc, wide(&p.bubble).as_ptr(), -1, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
        SelectObject(p.mem_dc, old);
        DeleteObject(font as *mut _);
        // GDI 写字会把 alpha 写 0：气泡矩形内 alpha=0 的像素补成 255
        for y in by..(by + bh) {
            for x in bx..(bx + bw) {
                let i = ((y * w + x) * 4) as usize;
                if *p.bits.add(i + 3) == 0 {
                    *p.bits.add(i + 3) = 255;
                }
            }
        }
    }

    // 打瞌睡的 zzz
    if p.state == State::Sleep {
        p.zzz_phase += 0.15;
        let zoff = (p.zzz_phase.sin() * 4.0 * p.scale) as i32;
        let zx = w - (70.0 * p.scale) as i32;
        let zy = role_top + (40.0 * p.scale) as i32 - zoff;
        fill_round_rect(p.bits, w, zx, zy, (46.0 * p.scale) as i32, (26.0 * p.scale) as i32, (10.0 * p.scale) as i32, (34, 32, 36));
        for y in zy..(zy + (26.0 * p.scale) as i32) {
            for x in zx..(zx + (46.0 * p.scale) as i32) {
                let i = ((y * w + x) * 4) as usize;
                if *p.bits.add(i + 3) == 0 {
                    *p.bits.add(i + 3) = 255;
                }
            }
        }
    }

    blit(p);

    // 首帧诊断：确认位图里真的有东西、窗口是可见的
    static mut FRAME_LOGGED: bool = false;
    if !FRAME_LOGGED {
        FRAME_LOGGED = true;
        let mut nonzero = 0usize;
        for i in (0..(w * h * 4) as usize).step_by(4) {
            if *p.bits.add(i + 3) > 0 {
                nonzero += 1;
            }
        }
        println!(
            "[pet] 首帧位图非透明像素={nonzero} 窗口visible={} hwnd={:?}",
            IsWindowVisible(p.hwnd),
            p.hwnd
        );
    }
}

unsafe fn say(text: &str, secs: f32) {
    if let Some(p) = &mut PET {
        p.bubble = text.to_string();
        p.bubble_until = Some(Instant::now() + std::time::Duration::from_secs_f32(secs));
        p.state = State::Talk;
        draw_frame(p);
    }
}

unsafe fn save_position(p: &Pet) {
    let mut wr: RECT = std::mem::zeroed();
    GetWindowRect(p.hwnd, &mut wr);
    let lx = (wr.left as f32 / p.scale).round() as i32;
    let ly = (wr.top as f32 / p.scale).round() as i32;
    let path = std::env::current_exe().ok().map(|e| e.with_file_name("pet-position.txt"));
    if let Some(path) = path {
        let _ = std::fs::write(path, format!("{lx},{ly}"));
        println!("[pet] 位置已保存: {lx},{ly}");
    }
}

unsafe fn load_position() -> Option<(i32, i32)> {
    let path = std::env::current_exe().ok()?.with_file_name("pet-position.txt");
    let text = std::fs::read_to_string(path).ok()?;
    let mut it = text.split(',');
    let x = it.next()?.trim().parse().ok()?;
    let y = it.next()?.trim().parse().ok()?;
    Some((x, y))
}

unsafe fn start_fall() {
    if let Some(p) = &mut PET {
        let mut wr: RECT = std::mem::zeroed();
        GetWindowRect(p.hwnd, &mut wr);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        if screen_h - wr.bottom > 30 {
            p.falling = true;
            p.fall_v = 0.0;
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_LBUTTONDOWN => {
            if let Some(p) = &mut PET {
                p.dragging = true;
                p.moved = false;
                p.long_press_fired = false;
                let mut pt = POINT { x: 0, y: 0 };
                GetCursorPos(&mut pt);
                let mut wr: RECT = std::mem::zeroed();
                GetWindowRect(hwnd, &mut wr);
                p.drag_start = (pt.x, pt.y);
                p.win_start = (wr.left, wr.top);
                SetCapture(hwnd);
                SetTimer(hwnd, 2, LONG_PRESS_MS, None);
            }
            0
        }
        WM_MOUSEMOVE => {
            if let Some(p) = &mut PET {
                if p.dragging {
                    let mut pt = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut pt);
                    let dx = pt.x - p.drag_start.0;
                    let dy = pt.y - p.drag_start.1;
                    if !p.moved && (dx.abs() > 4 || dy.abs() > 4) {
                        p.moved = true;
                        KillTimer(hwnd, 2);
                    }
                    if p.moved {
                        SetWindowPos(hwnd, HWND_TOPMOST, p.win_start.0 + dx, p.win_start.1 + dy, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
                    }
                }
            }
            0
        }
        WM_LBUTTONUP => {
            if let Some(p) = &mut PET {
                if p.dragging {
                    ReleaseCapture();
                    KillTimer(hwnd, 2);
                    p.dragging = false;
                    if p.moved {
                        println!("[pet] 拖动结束");
                        start_fall();
                    } else if p.long_press_fired {
                        // 长按已在定时器里处理
                    } else {
                        say("在呢在呢～", 2.2);
                        println!("[pet] 单击说话");
                    }
                }
            }
            0
        }
        WM_TIMER => {
            if wparam == 2 {
                // 长按摸头
                if let Some(p) = &mut PET {
                    if p.dragging && !p.moved {
                        p.long_press_fired = true;
                        KillTimer(hwnd, 2);
                        say("唔…好舒服", 2.5);
                        println!("[pet] 摸头");
                    }
                }
                return 0;
            }
            if wparam == 1 {
                if let Some(p) = &mut PET {
                    // 气泡超时
                    if let Some(until) = p.bubble_until {
                        if Instant::now() >= until {
                            p.bubble.clear();
                            p.bubble_until = None;
                            p.state = State::Idle;
                        }
                    }
                    // 掉落
                    if p.falling {
                        let mut wr: RECT = std::mem::zeroed();
                        GetWindowRect(hwnd, &mut wr);
                        let screen_h = GetSystemMetrics(SM_CYSCREEN);
                        let mut y = wr.top as f32;
                        p.fall_v += FALL_GRAVITY;
                        y += p.fall_v;
                        let floor = (screen_h - p.phys_h) as f32;
                        if y >= floor {
                            y = floor;
                            if p.fall_v > 6.0 {
                                p.fall_v = -p.fall_v * 0.42;
                            } else {
                                p.falling = false;
                                p.fall_v = 0.0;
                                save_position(p);
                            }
                        }
                        SetWindowPos(hwnd, HWND_TOPMOST, wr.left, y as i32, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
                    }
                    // 长时间无操作 → 睡
                    if p.state == State::Idle && Instant::now().duration_since(p.last_active).as_secs() > SLEEP_AFTER_SECS {
                        p.state = State::Sleep;
                        say("有点困了…", 2.5);
                        p.state = State::Sleep;
                        println!("[pet] 打瞌睡");
                    }
                    draw_frame(p);
                }
                return 0;
            }
            0
        }
        WM_RBUTTONUP => {
            if let Some(p) = &mut PET {
                let menu = CreatePopupMenu();
                let mut checked = if p.click_through { MF_CHECKED } else { MF_UNCHECKED };
                AppendMenuW(menu, MF_STRING | checked, 1, wide("鼠标穿透").as_ptr());
                checked = MF_UNCHECKED;
                AppendMenuW(menu, MF_STRING | checked, 2, wide("重置位置").as_ptr());
                AppendMenuW(menu, MF_SEPARATOR, 0, null_mut());
                AppendMenuW(menu, MF_STRING, 3, wide("退出").as_ptr());
                let mut pt = POINT { x: 0, y: 0 };
                GetCursorPos(&mut pt);
                let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, null_mut());
                DestroyMenu(menu);
                match cmd {
                    1 => {
                        p.click_through = !p.click_through;
                        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
                        let ex = if p.click_through { ex | WS_EX_TRANSPARENT } else { ex & !WS_EX_TRANSPARENT };
                        SetWindowLongW(hwnd, GWL_EXSTYLE, ex as i32);
                        println!("[pet] 穿透 = {}", p.click_through);
                    }
                    2 => {
                        let (sw, sh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
                        SetWindowPos(hwnd, HWND_TOPMOST, sw - p.phys_w - (24.0 * p.scale) as i32, sh - p.phys_h - (48.0 * p.scale) as i32, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
                        save_position(p);
                    }
                    3 => {
                        save_position(p);
                        PostQuitMessage(0);
                    }
                    _ => {}
                }
            }
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
        let hdc = GetDC(null_mut());
        let dpi = GetDeviceCaps(hdc, LOGPIXELSX);
        let scale = dpi as f32 / 96.0;
        let phys_w = (ROLE as f32 * scale).round() as i32;
        let phys_h = (WIN_LOGIC_H as f32 * scale).round() as i32;
        let role_phys = phys_w;
        let bubble_phys_h = phys_h - role_phys;
        println!("[pet] dpi aware v2 = {aware}, dpi = {dpi}, scale = {scale:.2}");
        println!("[pet] 窗口物理 {phys_w}x{phys_h}（逻辑 {ROLE}x{WIN_LOGIC_H}）");

        let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let avatar_path = exe_dir
            .as_ref()
            .map(|d| d.join("assets").join("avatar.png"))
            .filter(|p| p.exists())
            .or_else(|| Some(std::path::PathBuf::from("assets/avatar.png")))
            .unwrap();
        let avatar = load_avatar(avatar_path.to_str().unwrap_or("assets/avatar.png"), role_phys);
        let opaque = avatar.chunks(4).filter(|c| c[3] > 0).count();
        println!("[pet] 头像: {} ({} 字节, 非透明像素 {})", avatar_path.display(), avatar.len(), opaque);

        let mut x = (100.0 * scale) as i32;
        let mut y = (100.0 * scale) as i32;
        if let Some((lx, ly)) = load_position() {
            x = (lx as f32 * scale) as i32;
            y = (ly as f32 * scale) as i32;
            println!("[pet] 读取位置: {lx},{ly} 逻辑");
        } else {
            let (sw, sh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
            x = sw - phys_w - (24.0 * scale) as i32;
            y = sh - phys_h - (48.0 * scale) as i32;
        }

        let hinstance = GetModuleHandleW(std::ptr::null());
        let class_name = wide("LingPetNative");
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_HAND),
            hbrBackground: null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: null_mut(),
        };
        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            wide("铃").as_ptr(),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            phys_w,
            phys_h,
            null_mut(),
            null_mut(),
            hinstance,
            null_mut(),
        );
        if hwnd.is_null() {
            println!("[pet] 创建窗口失败");
            return;
        }

        let mem_dc = CreateCompatibleDC(hdc);
        let mut pet = Box::new(Pet {
            hwnd,
            hdc_screen: hdc,
            mem_dc,
            bitmap: null_mut(),
            bits: null_mut(),
            phys_w,
            phys_h,
            role_phys,
            bubble_phys_h,
            scale,
            avatar,
            state: State::Idle,
            bubble: String::new(),
            bubble_until: None,
            dragging: false,
            moved: false,
            long_press_fired: false,
            drag_start: (0, 0),
            win_start: (0, 0),
            falling: false,
            fall_v: 0.0,
            last_active: Instant::now(),
            click_through: false,
            zzz_phase: 0.0,
        });
        create_backbuffer(&mut pet);
        PET = Some(pet);
        {
            let p = PET.as_mut().unwrap();
            draw_frame(p);
        }
        SetTimer(hwnd, 1, 33, None);
        println!("[pet] 就绪：单击说话，长按摸头，拖动后松手会掉到屏幕底，右键菜单");

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if let Some(p) = &PET {
            save_position(p);
        }
        println!("[pet] 退出");
    }
}
