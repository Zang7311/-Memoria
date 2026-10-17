# Live2D 着色器预编译缓存（patch 说明）

## 为什么有这个目录

Cubism Native SDK 在 **每次启动时** 用 `D3DCompile` 现场编译约 40 个着色器。
在这台机器（Xeon E5-2670 v2 + R7 350）上要 **25~27 秒**，桌面宠物不能接受这个启动时间。

## 改了什么

`CubismShader_D3D11.cpp` 的 `LoadShaderProgram()`：

1. **加载前**：先找 `FrameworkShaders/<entryPoint>_<vs|ps>.cso`
   - 找到 → 直接 `CreatePixelShader/CreateVertexShader`（毫秒级）
   - 找不到 → 走原来的 `D3DCompile`
2. **编译成功后**：把字节码写到同名 `.cso` 文件（下次启动复用）
3. `isFormat` 的 shader（`VertNormal`，要建 input layout）没有 blob 可用，自动回退现场编译（就一个，可忽略）

## 实测效果

| 运行 | CreateRenderer 耗时 | 总加载 |
|---|---|---|
| 首次（生成缓存） | 26,435 ms | 27.1 秒 |
| 之后（读缓存） | **234 ms** | **0.7 秒** |

缓存第一次运行后自动生成在 `build/FrameworkShaders/*.cso`（约 488 个文件、几 MB）。

## 如何重建工程

1. 从 `CubismSdkForNative-5-r.5.zip` 解压 `Core` 和 `Framework` 到 `pet-native/vendor/Cubism/`
2. 用本目录的 `CubismShader_D3D11.cpp` 覆盖
   `vendor/Cubism/Framework/src/Rendering/D3D11/CubismShader_D3D11.cpp`
3. 按 `live2d-render/CMakeLists.txt` 构建

> vendor/ 是第三方 SDK（21 MB），不进 git 仓库，所以在 `.gitignore` 里；
> 本目录存的是唯一被我们修改过的文件，保证工程可重现。
