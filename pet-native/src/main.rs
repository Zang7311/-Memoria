// 铃·桌宠 原生实现 v0.4（Win32 分层窗口，不经过 WebView2）
//
// 已验证：150% 缩放下窗口定位、DPI 换算、透明、鼠标交互全部正确。
// v0.2：真实头像 PNG、待机浮动、单击说话气泡、长按摸头、拖动、
//      松手掉落（带弹跳）、右键菜单（穿透/重置位置/退出）、位置记忆。
// v0.3：接上 Live2D —— 每帧问 PetL2D.dll 要一帧 RGBA，裁剪缩放后贴进角色区，
//      替掉 PNG 头像；窗口/拖动/掉落/弹跳/气泡/菜单全部不变。
//      DLL 缺失或出帧失败时自动退回 PNG 头像，不会白屏。
// v0.4：可以由《铃·记忆体》托管启动 ——
//      --data-dir 把配置/位置存到用户数据目录（安装目录可能只读），
//      --parent-pid 让应用退出时桌宠跟着关掉（不留孤儿进程），
//      再加单实例互斥量，双击快捷方式和应用自动拉起不会出现两只。
//
// 素材：assets/avatar.png（PNG 兜底）、live2d/（PetL2D.dll + 着色器缓存 + Haru 模型）
// 运行：cargo run（控制台会打印诊断；右键菜单退出）

mod live2d;

use std::mem::size_of;
use std::ptr::null_mut;
use std::time::Instant;

use image::imageops::FilterType;
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HBITMAP, HDC, HWND, POINT, RECT, SIZE};
use winapi::shared::winerror::ERROR_ALREADY_EXISTS;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::handleapi::CloseHandle;
use winapi::um::libloaderapi::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use winapi::um::processthreadsapi::OpenProcess;
use winapi::um::synchapi::{CreateMutexW, WaitForSingleObject};
use winapi::um::wingdi::*;
use winapi::um::winnt::SYNCHRONIZE;
use winapi::um::winuser::*;

/// 应用推消息过来的本机端口（改这里要同步改 ling-mem 的 desktop/pet_talk.rs）
const PET_PORT: u16 = 46180;
/// 自定义消息：收到一条聊天气泡（lparam = Box<ChatBubble> 裸指针）
const WM_APP_PET_SAY: UINT = WM_APP + 21;

/// 应用推过来的一条气泡（主线程接管所有权，处理完 drop）
struct ChatBubble {
    text: String,
    ttl: f32,
    mood: u8,
}

/// 角色区逻辑尺寸（150% 下物理 225）
const ROLE: i32 = 150;
/// 顶部气泡区逻辑高度：够放三行 12px 文字（物理 93 → 93）
const BUBBLE_H: i32 = 70;

const FALL_GRAVITY: f32 = 2.5;
const SLEEP_AFTER_SECS: u64 = 300;
const LONG_PRESS_MS: u32 = 700;

/// 启动参数（由《铃·记忆体》托管启动时传入；手动双击时全为空，行为和以前一样）
#[derive(Default)]
struct RunOpts {
    /// 配置/位置写到哪：安装目录可能是只读的（Program Files），应用会指到用户数据目录
    data_dir: Option<std::path::PathBuf>,
    /// 父进程 PID：应用一退出桌宠就跟着关掉，不留一个没人管的孤儿进程
    parent_pid: Option<u32>,
}

static OPTS: std::sync::OnceLock<RunOpts> = std::sync::OnceLock::new();

/// 冒不冒气泡（应用通过 cfg 帧同步；关掉后连点击/摸头的本地反应也不说话）
static SPEECH_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// 解析 `--data-dir <目录>` / `--parent-pid <pid>`（`--key=value` 写法也认）
fn parse_args() -> RunOpts {
    fn take(inline: Option<String>, args: &mut std::iter::Skip<std::env::Args>) -> Option<String> {
        inline.or_else(|| args.next())
    }
    let mut opts = RunOpts::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let (key, inline) = match arg.split_once('=') {
            Some((k, v)) => (k.to_string(), Some(v.to_string())),
            None => (arg, None),
        };
        match key.as_str() {
            "--data-dir" => {
                if let Some(v) = take(inline, &mut args) {
                    opts.data_dir = Some(std::path::PathBuf::from(v));
                }
            }
            "--parent-pid" => {
                if let Some(v) = take(inline, &mut args) {
                    opts.parent_pid = v.trim().parse().ok();
                }
            }
            other => println!("[pet] 忽略不认识的参数：{other}"),
        }
    }
    opts
}

/// 配置/位置文件的落点：给了 --data-dir 就用它，否则还是 exe 同目录
fn state_path(name: &str) -> Option<std::path::PathBuf> {
    let base = OPTS.get().and_then(|o| o.data_dir.clone()).or_else(|| {
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.to_path_buf()))
    })?;
    let _ = std::fs::create_dir_all(&base);
    Some(base.join(name))
}

/// 单实例保护：应用自动拉起 + 用户又双击了快捷方式时，桌面上只留一只
unsafe fn already_running() -> bool {
    let name = wide("LingPetNative.SingleInstance");
    let handle = CreateMutexW(null_mut(), 0, name.as_ptr());
    if handle.is_null() {
        return false;
    }
    if GetLastError() == ERROR_ALREADY_EXISTS {
        return true;
    }
    // 故意不 CloseHandle：进程活着就一直占着这个互斥量，退出时由系统回收
    false
}

/// 盯着父进程：应用被关掉（哪怕是被强杀）时桌宠自己退出，别在桌面上留下孤儿
unsafe fn spawn_parent_watchdog(parent_pid: u32, hwnd: HWND) {
    // 裸指针不是 Send，先转成整数再进线程
    let hwnd_addr = hwnd as isize;
    std::thread::spawn(move || {
        let handle = OpenProcess(SYNCHRONIZE, 0, parent_pid);
        if handle.is_null() {
            println!("[pet] 监视不了父进程 {parent_pid}，忽略");
            return;
        }
        loop {
            let r = WaitForSingleObject(handle, 1000);
            if r == 0 {
                // WAIT_OBJECT_0：父进程没了。走 WM_CLOSE 这条正常退出路，位置能存下来
                println!("[pet] 应用已退出，桌宠跟着关闭");
                PostMessageW(hwnd_addr as HWND, WM_CLOSE, 0, 0);
                CloseHandle(handle);
                return;
            }
        }
    });
}

/// 起一个后台线程收应用推来的气泡。
/// 应用没起来、端口被占都只是「没有联动」，不影响桌宠本体，所以失败只打日志。
unsafe fn spawn_talk_listener(hwnd: HWND) {
    let hwnd_addr = hwnd as usize;
    std::thread::spawn(move || {
        let sock = match std::net::UdpSocket::bind(("127.0.0.1", PET_PORT)) {
            Ok(s) => s,
            Err(e) => {
                println!("[pet] 联动端口 {PET_PORT} 绑定失败（{e}），本次不会显示聊天气泡");
                return;
            }
        };
        println!("[pet] 联动通道就绪：127.0.0.1:{PET_PORT}");
        let mut buf = [0u8; 4096];
        loop {
            let (n, _from) = match sock.recv_from(&mut buf) {
                Ok(v) => v,
                Err(_) => break,
            };
            let text = String::from_utf8_lossy(&buf[..n]).to_string();
            // 协议：v1\t<mood>\t<ttl>\t<正文>
            let mut it = text.splitn(4, '\t');
            if it.next() != Some("v1") {
                continue;
            }
            // 配置帧：v1 \t cfg \t speech=0/1
            let second = it.next().unwrap_or("calm");
            if second == "cfg" {
                let payload = it.next().unwrap_or("");
                if let Some(v) = payload.trim().strip_prefix("speech=") {
                    let on = v.trim() != "0";
                    SPEECH_ON.store(on, std::sync::atomic::Ordering::Relaxed);
                    println!("[pet] 冒气泡开关 → {}", if on { "开" } else { "关" });
                    // 关掉的瞬间把正在显示的气泡清掉
                    if !on {
                        if let Some(p) = &mut PET {
                            p.bubble.clear();
                            p.bubble_until = None;
                            p.state = State::Idle;
                            draw_frame(p);
                        }
                    }
                }
                continue;
            }
            let mood = match second {
                "happy" => 1,
                "sad" => 2,
                "angry" => 3,
                "curious" => 4,
                _ => 0,
            };
            let ttl = it.next().and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(6.0);
            let body = it.next().unwrap_or("").trim().to_string();
            if body.is_empty() {
                continue;
            }
            let boxed = Box::new(ChatBubble { text: body, ttl, mood });
            // 窗口先没了就没人接这个 Box，就地回收，别泄漏
            if PostMessageW(hwnd_addr as HWND, WM_APP_PET_SAY, 0, Box::into_raw(boxed) as LPARAM) == 0 {
                break;
            }
        }
        println!("[pet] 联动通道已关闭");
    });
}

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
    logic_size: i32,
    avatar_path: std::path::PathBuf,
    avatar: Vec<u8>, // 预乘 BGRA，尺寸 role_phys x role_phys
    /// Live2D 渲染器；None = 用 PNG 头像
    l2d: Option<live2d::Live2D>,
    /// 配置里是否要求用 Live2D（false 时即使 DLL 在也用 PNG）
    l2d_wanted: bool,
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
    bounce_count: u8,
    bounce_enabled: bool,
    last_active: Instant,
    click_through: bool,
    /// 气泡底色情绪：0 平静 1 开心 2 难过 3 生气 4 好奇
    bubble_mood: u8,
    zzz_phase: f32,
    /// 多帧形象：待机循环帧（assets/frames/idle/*.png，预乘 BGRA，role_phys²）
    idle_frames: Vec<Vec<u8>>,
    /// 说话循环帧（assets/frames/talk/*.png）；没有就用待机帧
    talk_frames: Vec<Vec<u8>>,
    /// 帧循环起点
    frame_clock: Instant,
    /// 底边淡出当前值 / 目标值（0 = 硬切，1 = 下摆化掉）
    fade: f32,
    fade_target: f32,
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

/// 载入头像：缩放 + 近黑背景透明化 + 圆形裁剪 + 预乘 alpha（BGRA）
fn load_avatar(path: &str, size: i32) -> Vec<u8> {
    match image::open(path) {
        Ok(img) => {
            let mut rgba = img.resize_exact(size as u32, size as u32, FilterType::Lanczos3).to_rgba8().into_raw();
            // 原图四角是纯黑不透明背景（实测 RGB(1,0,0)、(0,0,0)），
            // 圆形裁剪后会在圆边露出黑底 —— 先把近黑像素透明化。
            // 铃的头发/衣服是紫色调（125,126,203 等），阈值压在 24 以内不会误伤。
            for i in (0..rgba.len()).step_by(4) {
                let sum = rgba[i] as u32 + rgba[i + 1] as u32 + rgba[i + 2] as u32;
                if sum < 24 {
                    rgba[i + 3] = 0;
                }
            }
            // 圆形遮罩：圆外直接透明，圆边缘 1.5px 过渡；保留原图自身的 alpha
            let c = size as f32 / 2.0;
            let r = c - 1.0;
            for y in 0..size {
                for x in 0..size {
                    let dx = x as f32 + 0.5 - c;
                    let dy = y as f32 + 0.5 - c;
                    let d = (dx * dx + dy * dy).sqrt();
                    let mask = ((r - d) / 1.5).clamp(0.0, 1.0);
                    let i = ((y * size + x) * 4) as usize;
                    let orig = rgba[i + 3] as f32 / 255.0;
                    rgba[i + 3] = (mask * orig * 255.0) as u8;
                }
            }
            let mut out = vec![0u8; rgba.len()];
            for i in (0..rgba.len()).step_by(4) {
                let a = rgba[i + 3] as u32;
                out[i] = (rgba[i + 2] as u32 * a / 255) as u8;
                out[i + 1] = (rgba[i + 1] as u32 * a / 255) as u8;
                out[i + 2] = (rgba[i] as u32 * a / 255) as u8;
                out[i + 3] = rgba[i + 3];
            }
            out
        }
        Err(e) => {
            println!("[pet] 头像加载失败 {path}: {e}（用空图继续）");
            vec![0u8; (size * size * 4) as usize]
        }
    }
}

/// 载入一张形象帧：等比铺满 + 预乘 alpha（BGRA）。
/// 和头像不同：**不做**圆形裁剪、也不把深色抠掉——帧是端好背景的透明 PNG。
fn load_frame(path: &str, size: i32) -> Vec<u8> {
    match image::open(path) {
        Ok(img) => {
            let rgba = img
                .resize_exact(size as u32, size as u32, FilterType::Lanczos3)
                .to_rgba8()
                .into_raw();
            let mut out = vec![0u8; rgba.len()];
            for i in (0..rgba.len()).step_by(4) {
                let a = rgba[i + 3] as u32;
                out[i] = (rgba[i + 2] as u32 * a / 255) as u8;
                out[i + 1] = (rgba[i + 1] as u32 * a / 255) as u8;
                out[i + 2] = (rgba[i] as u32 * a / 255) as u8;
                out[i + 3] = rgba[i + 3];
            }
            out
        }
        Err(e) => {
            println!("[pet] 帧加载失败 {path}: {e}");
            Vec::new()
        }
    }
}

/// 扫某个目录下的 png 帧并按文件名排序（idle_00.png、idle_01.png…）
fn load_frames_dir(dir: &std::path::Path, size: i32) -> Vec<Vec<u8>> {
    let mut files: Vec<std::path::PathBuf> = match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().map(|x| x.eq_ignore_ascii_case("png")).unwrap_or(false))
            .collect(),
        Err(_) => return Vec::new(),
    };
    files.sort();
    files
        .iter()
        .filter_map(|p| {
            let f = load_frame(p.to_str()?, size);
            if f.is_empty() {
                None
            } else {
                Some(f)
            }
        })
        .collect()
}

/// 点是否在圆角矩形内（与 fill_round_rect 同一套判定，用来补 alpha 时避让圆角外）
fn inside_rounded(x: i32, y: i32, x0: i32, y0: i32, rw: i32, rh: i32, radius: i32) -> bool {
    if x < x0 || x >= x0 + rw || y < y0 || y >= y0 + rh {
        return false;
    }
    let dx = if x < x0 + radius {
        x0 + radius - x
    } else if x > x0 + rw - radius {
        x - (x0 + rw - radius)
    } else {
        0
    };
    let dy = if y < y0 + radius {
        y0 + radius - y
    } else if y > y0 + rh - radius {
        y - (y0 + rh - radius)
    } else {
        0
    };
    dx * dx + dy * dy <= radius * radius
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
    // 常驻 DIB（draw_frame 直接写 p.bits，mem_dc 一直选中它）：
    // 每帧只做 UpdateLayeredWindow，不再重建位图 —— 之前每帧 CreateDIBSection
    // 是掉落动画掉帧的主因。
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
        static mut FAIL_LOGGED: bool = false;
        if !FAIL_LOGGED {
            FAIL_LOGGED = true;
            println!("[pet] UpdateLayeredWindow FAILED err={}", GetLastError());
        }
    }
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

/// 打瞌睡的「z z z」：三个 z 在头顶气泡区里上下飘。
/// v0.2 画的是个纯黑圆角框（框里从来没画过字，补 alpha 时连圆角外也补成黑的），
/// 还压在角色脸上上下飘 —— 这里改成真的画字，位置挪到头顶。
unsafe fn draw_sleep_z(p: &mut Pet, w: i32) {
    let scale = p.scale;
    let px = (16.0 * scale) as i32;
    let font = CreateFontW(
        -px, 0, 0, 0, FW_BOLD as i32, 0, 0, 0, DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, ANTIALIASED_QUALITY, DEFAULT_PITCH,
        wide("Microsoft YaHei").as_ptr(),
    );
    let old = SelectObject(p.mem_dc, font as *mut _);
    SetBkMode(p.mem_dc, TRANSPARENT as i32);
    SetTextColor(p.mem_dc, RGB(214, 208, 226));
    for i in 0..3 {
        let t = (p.zzz_phase + i as f32 * 0.8).sin() * 0.5 + 0.5; // 0..1 上下飘
        let x = (w as f32) * 0.14 + (i as f32 * 10.0 + t * 3.0) * scale;
        // 竖直方向要保证三个 z 都留在气泡区里（字号 px，第三个不能压到角色头发上）
        let y = (p.bubble_phys_h as f32) * 0.06
            + (i as f32 * 9.0 + t * 5.0) * scale;
        let mut rect = RECT {
            left: x as i32,
            top: y as i32,
            right: ((x as i32) + px * 2),
            bottom: ((y as i32) + px * 2),
        };
        DrawTextW(p.mem_dc, wide("z").as_ptr(), -1, &mut rect, DT_LEFT | DT_TOP | DT_SINGLELINE | DT_NOCLIP);
    }
    SelectObject(p.mem_dc, old);
    DeleteObject(font as *mut _);
    // GDI 写字会把 alpha 写成 0：气泡区（含一点点溢出）里「有颜色但还是透明」的像素补成不透明
    //（气泡本体已经是不透明的，会被下面的判断跳过；角色像素 alpha 非 0，也不会被误伤）
    let y1 = (p.bubble_phys_h + px * 2).min(p.phys_h);
    for y in 0..y1 {
        for x in 0..p.phys_w {
            let i = ((y * p.phys_w + x) * 4) as usize;
            if *p.bits.add(i + 3) != 0 {
                continue;
            }
            let sum = *p.bits.add(i) as u32 + *p.bits.add(i + 1) as u32 + *p.bits.add(i + 2) as u32;
            if sum > 0 {
                *p.bits.add(i + 3) = 255;
            }
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
    // 底边淡出强度：被拖起来时淡掉下摆（不然悬空时就是「齐腰切断」），落地后再淡回来
    if (p.fade - p.fade_target).abs() < 0.01 {
        p.fade = p.fade_target;
    } else {
        p.fade += (p.fade_target - p.fade) * 0.25;
    }
    // 多帧形象优先：有 frames/ 就用帧循环（铃自己的立绘），没有才走 Live2D / PNG 头像
    let mut drew = false;
    {
        let using_talk = p.state == State::Talk && !p.talk_frames.is_empty();
        let frames: &Vec<Vec<u8>> = if using_talk { &p.talk_frames } else { &p.idle_frames };
        if let Some(frame) = frames.get({
            let fps = if using_talk { 8.0 } else { 3.0 };
            let elapsed = Instant::now().duration_since(p.frame_clock).as_secs_f32();
            (elapsed * fps) as usize % frames.len().max(1)
        }) {
            for y in 0..p.role_phys {
                let dy = role_top + y + float_off;
                if dy < 0 || dy >= h {
                    continue;
                }
                for x in 0..p.role_phys.min(w) {
                    let si = ((y * p.role_phys + x) * 4) as usize;
                    let di = ((dy * w + x) * 4) as usize;
                    if si + 3 >= frame.len() || di + 3 >= (w * h * 4) as usize {
                        continue;
                    }
                    *p.bits.add(di) = frame[si];
                    *p.bits.add(di + 1) = frame[si + 1];
                    *p.bits.add(di + 2) = frame[si + 2];
                    *p.bits.add(di + 3) = frame[si + 3];
                }
            }
            drew = true;
        }
    }
    // 其次 Live2D：让 DLL 画一帧，裁剪缩放后直接写进 DIB（预乘 BGRA）
    // 只有没画帧时才走 Live2D（否则 Haru 会把帧盖掉）
    if !drew {
        if let Some(l2d) = p.l2d.as_mut() {
            if l2d.render() {
                l2d.composite(p.bits, w, h, 0, role_top + float_off, p.fade);
                drew = true;
            }
        }
    }
    if p.l2d.as_ref().map_or(false, |l| l.broken()) {
        println!("[pet] Live2D 连续出帧失败，改回 PNG 头像");
        p.l2d = None;
    }
    // PNG 头像兜底（DLL 缺失 / 刚失败时走这条路）
    if !drew {
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
    }

    // 气泡
    if !p.bubble.is_empty() {
        let bx = (10.0 * p.scale) as i32;
        let by = (6.0 * p.scale) as i32;
        let bw = w - bx * 2;
        let bh = p.bubble_phys_h - by - (8.0 * p.scale) as i32;
        // 情绪底色：只调气泡底色，不动角色画面（0 平静 / 1 开心 / 2 难过 / 3 生气 / 4 好奇）
        let bubble_color = match p.bubble_mood {
            1 => (44, 32, 38),
            2 => (32, 34, 44),
            3 => (46, 30, 32),
            4 => (30, 40, 38),
            _ => (34, 32, 36),
        };
        fill_round_rect(p.bits, w, bx, by, bw, bh, (14.0 * p.scale) as i32, bubble_color);
        // 文字用 GDI 画，再把气泡区 alpha 补齐。
        // 字多就换小一号并用自动换行：150px 宽的窗口一行只放得下七八个汉字
        let font_px = if p.bubble.chars().count() > 12 { 12.0 } else { 15.0 };
        let font = CreateFontW(
            -((font_px * p.scale) as i32), 0, 0, 0, FW_NORMAL as i32, 0, 0, 0, DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_PITCH, wide("Microsoft YaHei").as_ptr(),
        );
        let old = SelectObject(p.mem_dc, font as *mut _);
        SetBkMode(p.mem_dc, TRANSPARENT as i32);
        SetTextColor(p.mem_dc, RGB(238, 230, 231));
        let mut rect = RECT { left: bx + 6, top: by + 3, right: bx + bw - 6, bottom: by + bh - 3 };
        DrawTextW(p.mem_dc, wide(&p.bubble).as_ptr(), -1, &mut rect, DT_CENTER | DT_WORDBREAK | DT_END_ELLIPSIS);
        SelectObject(p.mem_dc, old);
        DeleteObject(font as *mut _);
        // GDI 写字会把 alpha 写 0：只把「圆角矩形内部」的 alpha 补成 255；
        // 圆角外保持透明，否则四角会变成一块黑色（这就是之前语音框的黑边）
        let radius = (14.0 * p.scale) as i32;
        for y in by..(by + bh) {
            for x in bx..(bx + bw) {
                if !inside_rounded(x, y, bx, by, bw, bh, radius) {
                    continue;
                }
                let i = ((y * w + x) * 4) as usize;
                if *p.bits.add(i + 3) == 0 {
                    *p.bits.add(i + 3) = 255;
                }
            }
        }
    }

    // 打瞌睡：头顶飘 Zzz
    if p.state == State::Sleep {
        p.zzz_phase += 0.12;
        draw_sleep_z(p, w);
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
    // 本地反应（点一下、摸头、打瞌睡）：开关关掉就完全不出气泡
    if !SPEECH_ON.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    say_with_mood(text, secs, 0);
}

/// 冒一条气泡，并按情绪给气泡上色（0 平静 / 1 开心 / 2 难过 / 3 生气 / 4 好奇）
unsafe fn say_with_mood(text: &str, secs: f32, mood: u8) {
    if let Some(p) = &mut PET {
        p.bubble = text.to_string();
        p.bubble_mood = mood;
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
    let path = state_path("pet-position.txt");
    if let Some(path) = path {
        let _ = std::fs::write(path, format!("{lx},{ly}"));
        println!("[pet] 位置已保存: {lx},{ly}");
    }
}

/// 读配置文本并去掉 UTF-8 BOM（用记事本/PowerShell 存的文件常带 BOM，
/// 带 BOM 时第一行 "size=..." 会匹配不上，之前就是这么静默失效的）
fn config_text() -> Option<String> {
    let path = state_path("pet-config.txt")?;
    let text = std::fs::read_to_string(path).ok()?;
    Some(text.trim_start_matches('\u{feff}').to_string())
}

fn load_config_size() -> i32 {
    let Some(text) = config_text() else { return ROLE };
    for line in text.lines() {
        if let Some(v) = line.trim().strip_prefix("size=") {
            if let Ok(n) = v.trim().parse::<i32>() {
                return n.clamp(80, 320);
            }
        }
    }
    ROLE
}

fn load_config_bounce() -> bool {
    let Some(text) = config_text() else { return true };
    for line in text.lines() {
        if let Some(v) = line.trim().strip_prefix("bounce=") {
            return v.trim() != "0";
        }
    }
    true
}

/// Live2D 开关：pet-config.txt 里 `live2d=0` 时强制用 PNG 头像（默认开）
/// 模型目录名（live2d/Resources 下的子目录）；没配置就用官方样例 Haru
fn load_config_model() -> String {
    let Some(text) = config_text() else { return "Haru".to_string() };
    for line in text.lines() {
        if let Some(v) = line.trim().strip_prefix("model=") {
            let name = v.trim();
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }
    "Haru".to_string()
}

fn load_config_live2d() -> bool {
    let Some(text) = config_text() else { return true };
    for line in text.lines() {
        if let Some(v) = line.trim().strip_prefix("live2d=") {
            return v.trim() != "0";
        }
    }
    true
}

/// 是否留着控制台黑框：只有 pet-config.txt 写了 `console=1` 才留（默认藏起来，桌宠不该带个黑窗口）
fn keep_console() -> bool {
    let Some(text) = config_text() else { return false };
    for line in text.lines() {
        if let Some(v) = line.trim().strip_prefix("console=") {
            return v.trim() == "1";
        }
    }
    false
}

fn save_config(p: &Pet) {
    let console = keep_console();
    if let Some(path) = state_path("pet-config.txt") {
        let _ = std::fs::write(
            path,
            format!(
                "size={}\nbounce={}\nlive2d={}\nconsole={}\n",
                p.logic_size,
                if p.bounce_enabled { 1 } else { 0 },
                if p.l2d_wanted { 1 } else { 0 },
                if console { 1 } else { 0 }
            ),
        );
        println!(
            "[pet] 配置已保存: size={} bounce={} live2d={} console={}",
            p.logic_size, p.bounce_enabled, p.l2d_wanted, console
        );
    }
}

/// 换个尺寸：重载头像、重建位图、调整窗口（保持底边位置不变）
unsafe fn apply_size(p: &mut Pet, logic: i32) {
    if logic == p.logic_size {
        return;
    }
    let scale = p.scale;
    let new_role = (logic as f32 * scale).round() as i32;
    let new_phys_h = ((logic + BUBBLE_H) as f32 * scale).round() as i32;

    let mut wr: RECT = std::mem::zeroed();
    GetWindowRect(p.hwnd, &mut wr);
    let bottom = wr.bottom;
    let left = wr.left;

    p.avatar = load_avatar(p.avatar_path.to_str().unwrap_or("assets/avatar.png"), new_role);
    if !p.bitmap.is_null() {
        DeleteObject(p.bitmap as *mut _);
    }
    p.logic_size = logic;
    p.phys_w = new_role;
    p.phys_h = new_phys_h;
    p.role_phys = new_role;
    p.bubble_phys_h = new_phys_h - new_role;
    p.fade = 0.0;
    p.fade_target = 0.0;
    create_backbuffer(p);
    if let Some(l2d) = p.l2d.as_mut() {
        l2d.set_dest(new_role);
    }

    let new_top = bottom - new_phys_h;
    // 换大尺寸后别跑到屏幕外（keep 住底边，右边/上边超了就拉回来）
    let (sw, sh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
    let new_left = if left + new_role > sw { (sw - new_role).max(0) } else { left };
    let new_top = new_top.max(0).min((sh - new_phys_h).max(0));
    SetWindowPos(p.hwnd, HWND_TOPMOST, new_left, new_top, new_role, new_phys_h, SWP_NOACTIVATE);
    draw_frame(p);
    save_config(p);
    println!("[pet] 尺寸切换为逻辑 {logic}（物理 {new_role}x{new_phys_h}）");
}

unsafe fn load_position() -> Option<(i32, i32)> {
    let path = state_path("pet-position.txt")?;
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
            p.bounce_count = 0;
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
                // 有交互就不该算「长时间无操作」（v0.2 忘了刷新这个时间戳，
                // 导致开满 5 分钟后每次点它都会立刻打瞌睡）
                p.last_active = Instant::now();
                if p.state == State::Sleep {
                    p.state = State::Idle;
                }
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
                        // 一拖起来就先把下摆淡掉（这里只重画一次；拖动过程仍然只挪窗口不重画，
                        // 所以直接把 fade 拉满，不能靠每帧插值）
                        p.fade = 1.0;
                        p.fade_target = 1.0;
                        draw_frame(p);
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
                        // 松手后把下摆淡回来（掉落途中不重画，落地后逐帧淡入）
                        p.fade_target = 0.0;
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
        WM_APP_PET_SAY => {
            // 应用推来的聊天气泡：接管 lparam 上的 Box，处理完自动释放
            let msg = Box::from_raw(lparam as *mut ChatBubble);
            if let Some(p) = &mut PET {
                p.last_active = Instant::now();
            }
            // 兜底截断：应用那边会先压到 26 字，这里再保一道，免得别的发送方塞进来一长串把气泡撑破
            const MAX_SHOW: usize = 24;
            let shown = if msg.text.chars().count() > MAX_SHOW {
                let mut s: String = msg.text.chars().take(MAX_SHOW).collect();
                s.push('…');
                s
            } else {
                msg.text.clone()
            };
            say_with_mood(&shown, msg.ttl, msg.mood);
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
                    // 拖动中：位置由鼠标驱动，不重绘（重绘会与移动打架，表现为颤动）
                    if p.dragging {
                        return 0;
                    }
                    // 掉落中：只动位置、不重绘（重绘整窗是掉帧主因），弹跳到停稳为止
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
                            // 弹跳开关打开时最多弹两下：第一下大、第二下小；关掉就直接落地
                            if p.bounce_enabled && p.bounce_count < 2 && p.fall_v > 8.0 {
                                p.bounce_count += 1;
                                p.fall_v = -p.fall_v * 0.38;
                                println!("[pet] 触地反弹 #{} v={:.1}", p.bounce_count, p.fall_v);
                            } else {
                                p.falling = false;
                                p.fall_v = 0.0;
                                save_position(p);
                                println!("[pet] 落地停稳");
                            }
                        }
                        SetWindowPos(hwnd, HWND_TOPMOST, wr.left, y as i32, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
                        return 0;
                    }
                    // 气泡超时
                    if let Some(until) = p.bubble_until {
                        if Instant::now() >= until {
                            p.bubble.clear();
                            p.bubble_until = None;
                            // 气泡消失了要回到「该有的状态」：还是长时间没交互就继续睡。
                            // v0.2 这里一律设成 Idle，结果睡着之后每 2.5 秒又被判定
                            // 「长时间无操作」→ 反复说「有点困了…」+ 反复画瞌睡框。
                            p.state = if Instant::now().duration_since(p.last_active).as_secs() > SLEEP_AFTER_SECS {
                                State::Sleep
                            } else {
                                State::Idle
                            };
                        }
                    }
                    // 长时间无操作 → 睡（只有从 Idle 进来才说一次话，睡着之后不再重复）
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
                p.last_active = Instant::now();
                let menu = CreatePopupMenu();
                let ct_check = if p.click_through { MF_CHECKED } else { MF_UNCHECKED };
                AppendMenuW(menu, MF_STRING | ct_check, 1, wide("鼠标穿透").as_ptr());
                AppendMenuW(menu, MF_STRING, 2, wide("重置位置").as_ptr());
                let bounce_check = if p.bounce_enabled { MF_CHECKED } else { MF_UNCHECKED };
                AppendMenuW(menu, MF_STRING | bounce_check, 7, wide("掉落时弹跳").as_ptr());
                AppendMenuW(menu, MF_SEPARATOR, 0, null_mut());
                let s125 = if p.logic_size == 100 { MF_CHECKED } else { MF_UNCHECKED };
                let s150 = if p.logic_size == 150 { MF_CHECKED } else { MF_UNCHECKED };
                let s200 = if p.logic_size == 200 { MF_CHECKED } else { MF_UNCHECKED };
                AppendMenuW(menu, MF_STRING | s125, 3, wide("大小：小 100").as_ptr());
                AppendMenuW(menu, MF_STRING | s150, 4, wide("大小：中 150").as_ptr());
                AppendMenuW(menu, MF_STRING | s200, 5, wide("大小：大 200").as_ptr());
                AppendMenuW(menu, MF_SEPARATOR, 0, null_mut());
                AppendMenuW(menu, MF_STRING, 6, wide("退出").as_ptr());
                let mut pt = POINT { x: 0, y: 0 };
                GetCursorPos(&mut pt);
                // 菜单弹出期间把桌宠移到屏幕外，否则 topmost 的它会盖住菜单
                let mut wr: RECT = std::mem::zeroed();
                GetWindowRect(hwnd, &mut wr);
                let saved = (wr.left, wr.top);
                let off_x = GetSystemMetrics(SM_CXSCREEN) + 200;
                SetWindowPos(hwnd, HWND_TOPMOST, off_x, saved.1, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
                SetForegroundWindow(hwnd);
                let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, null_mut());
                SetWindowPos(hwnd, HWND_TOPMOST, saved.0, saved.1, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
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
                    3 => apply_size(p, 100),
                    4 => apply_size(p, 150),
                    5 => apply_size(p, 200),
                    7 => {
                        p.bounce_enabled = !p.bounce_enabled;
                        save_config(p);
                        println!("[pet] 弹跳 = {}", p.bounce_enabled);
                    }
                    6 => {
                        save_position(p);
                        PostQuitMessage(0);
                    }
                    _ => {}
                }
            }
            0
        }
        WM_CLOSE => {
            // Alt+F4 / 任务栏关闭：窗口销毁前先把位置存下来
            if let Some(p) = &PET {
                save_position(p);
            }
            DestroyWindow(hwnd);
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
    let _ = OPTS.set(parse_args());
    unsafe {
        // 应用会自动拉起桌宠，用户又双击了快捷方式的话，留一只就够了
        if already_running() {
            println!("[pet] 已经有一只桌宠在跑了，本次退出");
            std::process::exit(2);
        }
        // 桌宠是双击启动的，别把黑色控制台窗口摆在桌面上（要排查就 console=1）
        if !keep_console() {
            let cw = winapi::um::wincon::GetConsoleWindow();
            if !cw.is_null() {
                ShowWindow(cw, SW_HIDE);
            }
        }
        let aware = enable_dpi_awareness();
        let hdc = GetDC(null_mut());
        let dpi = GetDeviceCaps(hdc, LOGPIXELSX);
        let scale = dpi as f32 / 96.0;
        let logic_size = load_config_size();
        let bounce_enabled_init = load_config_bounce();
        let phys_w = (logic_size as f32 * scale).round() as i32;
        let phys_h = ((logic_size + BUBBLE_H) as f32 * scale).round() as i32;
        let role_phys = phys_w;
        let bubble_phys_h = phys_h - role_phys;
        println!("[pet] dpi aware v2 = {aware}, dpi = {dpi}, scale = {scale:.2}");
        println!("[pet] 窗口物理 {phys_w}x{phys_h}（逻辑 {logic_size}x{}）", logic_size + BUBBLE_H);

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

        // Live2D：DLL 缺失/加载失败就静默退回 PNG
        // 多帧形象：assets/frames/idle 与 assets/frames/talk（有就优先于 Live2D）
        let frames_root = exe_dir
            .as_deref()
            .unwrap_or(std::path::Path::new("."))
            .join("assets")
            .join("frames");
        let idle_frames = load_frames_dir(&frames_root.join("idle"), role_phys);
        let talk_frames = load_frames_dir(&frames_root.join("talk"), role_phys);
        if !idle_frames.is_empty() {
            println!(
                "[pet] 多帧形象：待机 {} 帧 / 说话 {} 帧（优先于 Live2D）",
                idle_frames.len(),
                talk_frames.len()
            );
        }

        let l2d_wanted = load_config_live2d();
        let l2d = if l2d_wanted {
            live2d::Live2D::init(
                exe_dir.as_deref().unwrap_or(std::path::Path::new(".")),
                role_phys,
                &load_config_model(),
            )
        } else {
            println!("[pet] 配置里关了 Live2D（live2d=0），用 PNG 头像");
            None
        };

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
        if let Some(pid) = OPTS.get().and_then(|o| o.parent_pid) {
            println!("[pet] 由应用托管（父进程 {pid}），它退出时我会跟着关");
            spawn_parent_watchdog(pid, hwnd);
        }
        // 应用 → 桌宠 的单向通道：铃回话时把内容推过来，这里冒气泡
        spawn_talk_listener(hwnd);

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
            logic_size,
            avatar_path: avatar_path.clone(),
            avatar,
            l2d,
            l2d_wanted,
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
            bounce_count: 0,
            bounce_enabled: bounce_enabled_init,
            last_active: Instant::now(),
            click_through: false,
            bubble_mood: 0,
            zzz_phase: 0.0,
            idle_frames,
            talk_frames,
            frame_clock: Instant::now(),
            fade: 0.0,
            fade_target: 0.0,
        });
        create_backbuffer(&mut pet);
        PET = Some(pet);
        {
            let p = PET.as_mut().unwrap();
            draw_frame(p);
        }
        SetTimer(hwnd, 1, 33, None);
        let mode = match PET.as_ref() {
            Some(p) if !p.idle_frames.is_empty() => "多帧形象",
            Some(p) if p.l2d.is_some() => "Live2D",
            _ => "PNG 头像",
        };
        println!("[pet] 就绪（{mode}）：单击说话，长按摸头，拖动后松手会掉到屏幕底，右键菜单");

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if let Some(p) = &PET {
            // 窗口还活着才存位置（WM_CLOSE 那条路已经在销毁前存过了）
            if IsWindow(p.hwnd) != 0 {
                save_position(p);
            }
            if let Some(l2d) = p.l2d.as_ref() {
                l2d.shutdown();
            }
        }
        println!("[pet] 退出");
    }
}
