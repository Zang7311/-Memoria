# pet-native —— 铃·桌宠（Win32 原生，不经过 WebView2）

一个 200 KB 出头的独立 exe：透明分层窗口、待机浮动、单击说话气泡、长按摸头、
拖动、松手掉到屏幕底（带弹跳）、右键菜单、位置记忆。
角色画面优先用 **Live2D**（PetL2D.dll），DLL 不在或出帧失败时自动退回 PNG 头像。

## 目录

| 路径 | 说明 |
|---|---|
| `src/main.rs` | 窗口、绘制、交互全部在这里（含气泡/掉落的像素绘制） |
| `src/live2d.rs` | PetL2D.dll 的运行时加载、取帧、裁剪缩放 |
| `assets/avatar.png` | PNG 兜底头像（圆形裁剪 + 预乘 alpha 在代码里做） |
| `scripts/sync-live2d.ps1` | 把 `live2d-render/build` 的运行时同步到目标目录的 `live2d/` |
| `live2d-render/` | C++ 渲染工程（CMake + Ninja），产出 `PetL2D.dll` |
| `live2d-test/` | Cubism Core 的最小验证程序 |

## 构建

```powershell
# 1) 渲染 DLL（需要 MSVC + cmake + ninja；着色器缓存已在 build/FrameworkShaders）
Set-Location live2d-render\build
cmake --build . --target PetL2D

# 2) 桌宠
Set-Location ..\..
cargo build --release

# 3) 把 live2d 运行时（DLL + 着色器缓存 + Haru 模型，约 5.8 MB）放到 exe 旁边
powershell -File scripts\sync-live2d.ps1                      # -> target\release\live2d
powershell -File scripts\sync-live2d.ps1 -Target 'F:\软件\LingPet'
```

桌宠运行时要找 `<exe目录>\live2d\PetL2D.dll`；`assets\avatar.png` 用于兜底。

## 配置：exe 同目录 `pet-config.txt`

```
size=150       # 角色区逻辑尺寸，80..320（默认 150；150% 缩放下物理 225）
bounce=1       # 掉落时是否弹跳（1 开 / 0 关）
live2d=1       # 0 = 强制用 PNG 头像（对比/排查用）
console=1      # 1 = 保留控制台黑框看日志（默认隐藏）
```

尺寸/弹跳也能在右键菜单里改，改完会写回这个文件。
（读配置时会自动去掉 UTF-8 BOM —— 记事本存的文件带 BOM 也能认。）

## Live2D 这一层怎么接的

- DLL 固定输出 **960×540**（取景参数 Scale 2.6 / Translate y -1.5 是照着这个尺寸调的，
  换方形画布会走 DLL 里另一条投影分支），`pet_l2d_load(model, resources, w, h)` 一次，
  之后每帧 `pet_l2d_render(t, outRGBA, 960, 540)` 拿一帧预乘 RGBA。
- 桌宠窗口是正方形，所以取帧后 **裁 (240,60,480×480) 再双线性缩放到角色区边长**。
  裁剪框按 20 秒 96 帧的 alpha 包围盒（x 293..666、y 86..539）定的，留了活动余量。
- DLL 读着色器/模型用的是相对路径，所以加载时把工作目录临时切到 `live2d/`，完了切回来。
- 配置改尺寸时只重算采样表，不重建 DLL 侧资源。
- 连续 120 帧出不了画面就放弃 Live2D、切回 PNG 头像。

## 已知事项

- DLL 首次在新目录运行会被 Defender 全量扫一遍：第一次加载约 13 秒，
  之后 0.5 秒左右（着色器已缓存）。别把首次启动当卡死。
- 模型现在是 Live2D 官方样例 **Haru**，不是铃 —— 等 Cubism Editor 做完模型再换。
- 退出时不做重量级清理（`pet_l2d_destroy()` 只置标志位），避免 DLL 析构阶段崩。
- 点击命中遵循分层窗口的逐像素 alpha：角色透明的地方点不到（会穿透到下面的窗口）。
