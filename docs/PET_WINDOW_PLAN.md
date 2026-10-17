# 桌宠窗口渲染问题：分析与后续方案

> 2026-10-17 记录。现象、实测数据、已排除的假设、候选方案都在这里，免得下次重查。

## 现象

桌宠（`floating-ball` 窗口：无边框、透明、置顶、skip_taskbar）在高缩放（本机 150%）屏幕上：

- 画面（头像 / Live2D / 气泡）向右下偏移，大约 `(74, 159)`，并被窗口边界裁掉一部分；
- 画面比窗口大约 1.3~1.5 倍；
- **主窗口（普通、有边框、非透明）完全正常**；
- 去掉透明（`transparent(false)`）后依然偏移，只是能看到一块白色窗口背景；

## 实测数据（同一时刻）

| 指标 | 值 | 期望 |
|---|---|---|
| 窗口逻辑尺寸 | 200×264 | 200×264 |
| 窗口物理尺寸 | 300×396 | 300×396 |
| WebView 视口（`innerWidth×innerHeight`） | **300×396** | 200×264 |
| `window.devicePixelRatio` | **1.00** | 1.50 |
| Tauri `scaleFactor()` | 1.50 | 1.50 |
| 一个 `position:fixed; inset:0` 探针 | 300×396 @ 0,0（但屏幕上的实际位置比窗口左上角偏 36,32） | 与窗口客户区重合 |

结论：**WebView2 在这个窗口上把物理像素当成了 CSS 像素**（视口 300×396、DPR 1），
内容按 300×396 布局，再被合成到一个位置偏掉的位置上。

## 已排除的假设（全部实机验证无效）

| 假设 | 做法 | 结果 |
|---|---|---|
| 启动时机太早 | 创建时 `.position()` / 前端 setPosition / 延迟 1.5s 定位 | ❌ |
| 程序移动窗口导致 | 完全不移动窗口，只在窗口创建时给尺寸 | ❌ |
| 尺寸不同步（窗口创建后立即 setSize） | 创建 200×200 → 前端 setSize → 或反过来 | ❌ |
| bounds 没下发 | 「先缩 1px 再还原」尺寸抖动 | ❌ |
| 透明窗口合成问题 | `transparent(false)` | ❌ |
| GPU 合成问题 | `--disable-gpu` 强制软件渲染 | ❌ |
| 视口单位布局问题 | 布局改 `%`/`vh` | ❌ |
| 页面缩放 | CSS `zoom`（0.667/0.75/0.9） | 大小变、偏移不变 |
| webview zoom | 加 `core:webview:allow-set-webview-zoom` 后 `setZoom(1/1.5)` | 部分缩小，仍偏 |
| 多显示器 DPI 混淆 | 枚举显示器：本机只有 1 个活动显示器 | ❌（排除） |
| 坐标原点问题 | 把窗口挪到 (0,0) 后测 | 仍偏 (+25,+140) ❌ |

## 根因判断

Tauri 在 150% 缩放下创建这个无边框透明窗口时，WebView2 的 **RasterizationScale / DPI 上下文没被正确设置**：

- 窗口本身按逻辑尺寸 200×264 → 物理 300×396 创建（正确）；
- 但 WebView2 以 DPR=1 计算，把 300×396 物理当成 300×396 CSS；
- 同时 WebView2 合成层的位置与窗口客户区错位（探针偏移 36,32）；
- 普通窗口（主窗口）不受影响 → 与窗口样式（无边框/透明/置顶）相关。

## 候选方案

### 方案 A：Rust 分层窗口自绘（根治，工作量大）

放弃 WebView 渲染桌宠，改用 `windows` crate：

- 用 `WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST` 创建原生窗口；
- 用 `UpdateLayeredWindow` 贴 ARGB 位图，自己控制每帧；
- 形象：PNG 序列帧（待机/摸头/被戳/睡觉）或 tiny-skia 矢量绘制；
- 交互：鼠标事件在原生窗口里处理，点击命中按 alpha 判断（透明区域穿透）；
- 与主窗口通信：本地 IPC（Tauri 命令 → channel / 共享状态）。

优点：完全可控、不依赖 WebView2；缺点：Live2D 用不了（要么换成逐帧立绘，要么自己接 Cubism SDK），工程 1~2 天。

### 方案 B：等 Tauri/WebView2 修复后重测

把问题复现步骤写成 issue 里的最小样例，升级 Tauri 后重测。零成本，但不可控。

### 方案 C：改变产品形态（绕开）

桌宠不再是独立置顶窗口，而是主窗口内的一块区域 + 托盘常驻。失去「跨应用陪伴」的核心卖点，不推荐。

### 方案 D：给 WebView2 单独设置缩放（未验证完）

进程级 `--force-device-scale-factor=1` 能让 WebView 视口=物理，但会影响主窗口（主窗口也变小）。若 Tauri 支持按 webview 传 args（目前不暴露），可以只对桌宠窗口生效。属于「等上游」。

## 当前处置

- 桌宠默认关闭（`floating_ball_enabled = false`），设置页有提示；
- Live2D 标为实验；
- 交互、台词、气泡、右键菜单等功能代码全部保留，问题只卡在「画面位置」。

---

# 2026-10-17 原生验证结果（pet-native）

按 GPT 建议做的第一步：不经过 WebView2，用 Rust + Win32 分层窗口验证。

程序在 `pet-native/`（独立子项目，不进主程序）。

## 窗口 / DPI 实测（全部正确）

```
[pet-native] per-monitor v2 = true
[pet-native] screen physical = 1920x1080, LOGPIXELSX dpi = 144, scale = 1.50
[pet-native] target: logic 200x200 -> physical 300x300 at physical (150,150)
[created] hwnd dpi=144 scale=1.50 phys=300x300
          window_rect=(150,150)-(450,450) client=300x300 bitmap=300x300
```

| 指标 | 原生分层窗口 | WebView2 桌宠 |
|---|---|---|
| DPI 感知 | Per-Monitor V2 | 同 |
| 窗口物理尺寸 | 300×300（逻辑 200） | 300×396 |
| 客户区 | 300×300 | 对不上 |
| 渲染位图 | 300×300 | 视口报 300×396 |
| 屏幕位置 | (150,150) 精确 | 偏 (74,159) |
| 形状 | 正圆，宽高均 274 像素 | 放大 1.35 倍 |

## 交互实测（全部通过）

- 单击圆内：收到 `left down` + `click (no drag)` ✅
- 拖动：窗口从 (150,150) 移到 (294,246)，位移与鼠标位移（144,96）完全相等 ✅
- 透明区域点击：窗口无事件，点击落到下层窗口（前台从桌宠变为 DSH）✅

## 结论

**问题 100% 在 WebView2（Tauri 的 webview 子窗口），不在系统、GPU、显示器或我们的代码。**
原生分层窗口方案可行，可以进入下一阶段。

## Cubism Native SDK 现状

- 仓库可 clone（`Live2D/CubismNativeSamples`，26.6 MB，含 Samples/Framework）
- **Core 不在仓库里**（README 明说），需要从 Live2D 官网下载（注册并同意许可）
- 许可：个人/非商业免费（年收入 < 1000 万日元实体无需 Release License）；Core 走 Live2D Proprietary Software License；样例模型走 Free Material License

## 下一步（按 GPT 顺序）

1. ~~原生窗口验证~~ ✅
2. 桌宠交互层（拖动/点击/右键/位置保存/穿透/掉落）
3. Cubism Native 接入（等待 Core）

