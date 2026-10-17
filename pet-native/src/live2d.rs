// Live2D 渲染 DLL（PetL2D.dll）的运行时加载与取帧
//
// 设计：
//   - 运行时 LoadLibrary，不产生链接期依赖：DLL 缺失 / 加载失败时桌宠自动退回 PNG 头像
//   - DLL 固定输出 960x540（16:9）。取景参数（Scale 2.6 / Translate y -1.5）是照着这个
//     尺寸调好的，换成方形画布会走 DLL 里另一条投影分支，画面完全不同 —— 所以不改尺寸，
//     而是在这一层把画面裁成正方形再缩放到窗口的角色区。
//   - 裁剪框 (240,60,480) 是按实测 alpha 包围盒定的：20 秒采样 96 帧，模型活动范围
//     x 293..666、y 86..539，480 的正方形框刚好留出四周余量（头发不会被切）。
//   - 双线性缩放在本进程做（预乘 RGBA → 预乘 BGRA，直接给 UpdateLayeredWindow 用）
//
// 路径：DLL 读着色器/模型用的是相对路径（"FrameworkShaders/..."、"Resources/Haru/"），
// 所以调用 load 时把当前目录临时切到 <exe目录>/live2d，加载完再切回来。

use std::path::Path;
use std::time::Instant;

use winapi::shared::minwindef::HMODULE;
use winapi::um::libloaderapi::{GetProcAddress, LoadLibraryW};

/// DLL 渲染分辨率（必须保持 16:9，否则 DLL 内部投影分支会变）
pub const SRC_W: i32 = 960;
pub const SRC_H: i32 = 540;
/// 从 960x540 里裁出的正方形（左上角 + 边长）
const CROP_X: i32 = 240;
const CROP_Y: i32 = 60;
const CROP: i32 = 480;

/// 模型名与资源目录（相对 <exe>/live2d，全 ASCII 以免 CRT 窄字符路径出问题）
const MODEL_NAME: &[u8] = b"Haru\0";
const RESOURCES_DIR: &[u8] = b"Resources/\0";

type LoadFn = unsafe extern "C" fn(*const i8, *const i8, i32, i32) -> i32;
type RenderFn = unsafe extern "C" fn(f32, *mut u8, i32, i32) -> i32;
type DestroyFn = unsafe extern "C" fn();

pub struct Live2D {
    render: RenderFn,
    destroy: Option<DestroyFn>,
    /// DLL 输出的整帧 RGBA（预乘 alpha），SRC_W * SRC_H * 4
    frame: Vec<u8>,
    /// 每个目标像素在 x / y 方向的双线性采样点 (i0, i1, frac)
    xs: Vec<(u32, u32, f32)>,
    ys: Vec<(u32, u32, f32)>,
    /// 目标边长（= 窗口角色区物理边长）
    dest: i32,
    started: Instant,
    fail_count: u32,
}

fn wide(s: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

unsafe fn symbol<T>(lib: HMODULE, name: &[u8]) -> Option<T> {
    let p = GetProcAddress(lib, name.as_ptr() as *const i8);
    if p.is_null() {
        return None;
    }
    Some(std::mem::transmute_copy(&p))
}

/// 把目标轴的每个像素映射到裁剪框内的源坐标（双线性：两个采样点 + 权重）
fn axis_map(dest: i32, crop_off: i32) -> Vec<(u32, u32, f32)> {
    let mut v = Vec::with_capacity(dest as usize);
    if dest <= 0 {
        return v;
    }
    let step = CROP as f32 / dest as f32;
    let last = (crop_off + CROP - 1) as u32;
    for d in 0..dest {
        let s = ((d as f32 + 0.5) * step - 0.5).max(0.0);
        let base = s.floor();
        let frac = s - base;
        let i0 = (crop_off as u32 + base as u32).min(last);
        let i1 = (i0 + 1).min(last);
        v.push((i0, i1, frac));
    }
    v
}

impl Live2D {
    /// 尝试加载 <exe目录>/live2d/PetL2D.dll 并载入模型。任一步失败返回 None（调用方退回 PNG）。
    /// dest = 角色区物理边长（窗口宽度），决定裁剪缩放后的输出尺寸。
    pub unsafe fn init(exe_dir: &Path, dest: i32) -> Option<Live2D> {
        let dir = exe_dir.join("live2d");
        let dll_path = dir.join("PetL2D.dll");
        if !dll_path.exists() {
            println!("[pet] 未找到 {}（用 PNG 头像）", dll_path.display());
            return None;
        }
        let wpath = wide(&dll_path);
        let lib = LoadLibraryW(wpath.as_ptr());
        if lib.is_null() {
            println!("[pet] LoadLibrary 失败：{}", dll_path.display());
            return None;
        }
        let load: LoadFn = match symbol(lib, b"pet_l2d_load\0") {
            Some(f) => f,
            None => {
                println!("[pet] DLL 里没有 pet_l2d_load");
                return None;
            }
        };
        let render: RenderFn = match symbol(lib, b"pet_l2d_render\0") {
            Some(f) => f,
            None => {
                println!("[pet] DLL 里没有 pet_l2d_render");
                return None;
            }
        };
        let destroy: Option<DestroyFn> = symbol(lib, b"pet_l2d_destroy\0");

        // DLL 用相对路径读着色器和模型，先把工作目录切到 live2d/
        let old_cwd = std::env::current_dir().ok();
        if std::env::set_current_dir(&dir).is_err() {
            println!("[pet] 无法切换到 {}", dir.display());
            return None;
        }
        let t0 = Instant::now();
        let ok = load(MODEL_NAME.as_ptr() as *const i8, RESOURCES_DIR.as_ptr() as *const i8, SRC_W, SRC_H);
        let ms = t0.elapsed().as_millis();
        if let Some(old) = old_cwd {
            let _ = std::env::set_current_dir(old);
        }
        if ok == 0 {
            println!("[pet] pet_l2d_load 失败（用时 {ms} ms）");
            return None;
        }
        println!("[pet] Live2D 模型就绪：Haru {SRC_W}x{SRC_H}，加载 {ms} ms");

        let mut l2d = Live2D {
            render,
            destroy,
            frame: vec![0u8; (SRC_W * SRC_H * 4) as usize],
            xs: Vec::new(),
            ys: Vec::new(),
            dest: 0,
            started: Instant::now(),
            fail_count: 0,
        };
        l2d.set_dest(dest);
        Some(l2d)
    }

    /// 目标边长变化（切换大小）时重建采样表；不重建 DLL 侧资源
    pub fn set_dest(&mut self, dest: i32) {
        if dest == self.dest || dest <= 0 {
            return;
        }
        self.dest = dest;
        self.xs = axis_map(dest, CROP_X);
        self.ys = axis_map(dest, CROP_Y);
    }

    /// 让 DLL 画一帧。成功返回 true。
    pub unsafe fn render(&mut self) -> bool {
        let t = self.started.elapsed().as_secs_f32();
        let r = (self.render)(t, self.frame.as_mut_ptr(), SRC_W, SRC_H);
        if r == 1 {
            self.fail_count = 0;
            true
        } else {
            self.fail_count += 1;
            if self.fail_count == 1 || self.fail_count % 300 == 0 {
                println!("[pet] Live2D 出帧失败（第 {} 次）", self.fail_count);
            }
            false
        }
    }

    /// 连续失败太多次：认为 DLL 已不可用，调用方可以退回 PNG
    pub fn broken(&self) -> bool {
        self.fail_count > 120
    }

    /// 把当前帧裁剪缩放后写进分层窗口的 DIB（预乘 BGRA）
    /// dx, dy = 角色区左上角在窗口内的位置
    pub unsafe fn composite(&self, dst: *mut u8, win_w: i32, win_h: i32, dx: i32, dy: i32) {
        let dest = self.dest;
        let src = self.frame.as_ptr();
        let stride = (SRC_W * 4) as usize;
        for y in 0..dest {
            let ty = dy + y;
            if ty < 0 || ty >= win_h {
                continue;
            }
            let (sy0, sy1, wy) = self.ys[y as usize];
            let row0 = sy0 as usize * stride;
            let row1 = sy1 as usize * stride;
            let iwy = 1.0 - wy;
            for x in 0..dest {
                let tx = dx + x;
                if tx < 0 || tx >= win_w {
                    continue;
                }
                let (sx0, sx1, wx) = self.xs[x as usize];
                let col0 = sx0 as usize * 4;
                let col1 = sx1 as usize * 4;
                let ixw = 1.0 - wx;
                let w00 = ixw * iwy;
                let w10 = wx * iwy;
                let w01 = ixw * wy;
                let w11 = wx * wy;
                let p00 = src.add(row0 + col0);
                let p10 = src.add(row0 + col1);
                let p01 = src.add(row1 + col0);
                let p11 = src.add(row1 + col1);
                let mut c = [0u8; 4];
                for k in 0..4 {
                    let v = *p00.add(k) as f32 * w00
                        + *p10.add(k) as f32 * w10
                        + *p01.add(k) as f32 * w01
                        + *p11.add(k) as f32 * w11;
                    c[k] = v.round().clamp(0.0, 255.0) as u8;
                }
                let di = ((ty * win_w + tx) * 4) as usize;
                *dst.add(di) = c[2];
                *dst.add(di + 1) = c[1];
                *dst.add(di + 2) = c[0];
                *dst.add(di + 3) = c[3];
            }
        }
    }

    /// 退出时调用：只让 DLL 停掉渲染，不做重量级清理（DLL 侧清理会崩，见已知问题）
    pub unsafe fn shutdown(&self) {
        if let Some(f) = self.destroy {
            f();
        }
    }
}
