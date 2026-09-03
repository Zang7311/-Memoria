# 铃·记忆体（Memoria）

> 我，与你交谈，为你存忆。

**铃·记忆体** 是一款开源、离线优先的 **Windows 桌面猫娘伴侣**。她不只是一个聊天 AI——她会记住你、看懂屏幕、调用 49 个系统工具、在局域网设备间同步记忆，真正"生活"在你的电脑里。

第一眼，她是"我的铃"；相处久了你会发现，她还能帮你管理电脑。

<p align="center">
  <img src="https://img.shields.io/badge/version-0.6.0-ff7a94" alt="version">
  <img src="https://img.shields.io/badge/platform-Windows-blue" alt="platform">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="license">
  <img src="https://img.shields.io/badge/offline--first-yes-success" alt="offline-first">
</p>

> ## ⬇️ 下载安装
> 从 [Releases](https://github.com/Zang7311/-Memoria/releases) 下载最新版安装包（`.exe`），双击即可安装。
> 语义检索模型（约 91MB，可选功能）在应用内「设置 → 离线语义检索模型」一键安装，或首次使用方案3时按提示自动检索安装。



## ✨ 核心特性

| 能力 | 说明 |
|------|------|
| 💬 对话 | 三种模式：云端 API（DeepSeek / OpenAI 兼容）/ 本地 Ollama / 离线内置文库；4 种人格（日常/中二/治愈/涩涩）；思考深度 4 档；流式打字机输出 |
| 🧠 记忆 | 长期记忆自动写入，本地 JSON 存储；**记忆中心**（分类筛选 / 批量删除 / 标重要 / 编辑 / 容量统计 / 重复检测）；6 类自动分类；主密码加密（PBKDF2 + AES-256-GCM）；记忆透明页、可看可改可删 |
| 🧰 工具箱 | **49 个工具**（系统/网络/性能/文件安全/媒体/二维码/OCR/开发环境等），危险操作执行前确认 |
| 🔌 插件 | 纯 Rust JS 引擎（boa_engine）；安装时强制权限确认（最小权限原则） |
| 🔄 同步 | 局域网多设备同步（UDP 发现 + TCP 传输 + AES 加密 + SHA-256 校验），增量同步、冲突策略、跨版本兼容 |
| 🎭 形象互联 | 本地 TCP 服务（VBIL 协议），其他桌面虚拟形象接入；本地规则引擎 + off/rule_only/ai 三种响应模式；窗口扫描发现形象 |
| 📟 快捷指令 | 自定义组合指令（如"晚安模式"），一条指令按序执行一串动作，聊天里说指令名直接触发 |
| 📷 OCR / 二维码 | OCR 双引擎（Windows 内置优先，Tesseract 降级）；二维码生成/识别（纯 Rust，离线可用） |
| 🖥️ 系统集成 | 系统托盘 / 悬浮球 / 屏幕监测+气泡 / 全局快捷键 / 开机自启 / 管理员自选 |
| 🎨 外观 | 主色/背景图/头像/圆角/气泡色自定义（软件内选图）；多套主题；SVG 矢量图标；Emoji 三级开关 |
| 💝 陪伴 | 陪伴日记（1-3 天一记）；特殊事件集（节日祝福 8 + 陪伴里程碑 7/30/100/365 + 关键词彩蛋 10） |
| 🛟 兜底 | 救援模式（--recovery）；诊断报告导出；DependencyManager 统一依赖引导 |

## 🎁 关于语义检索模型（可选功能）

**安装包只有一种**，语义检索模型不打包进安装包，而是按需安装：

- **默认不带模型**：安装包轻量（~10 MB），首次启动快。
- **需要语义检索（方案3）时**：在应用内「设置 → 离线语义检索模型」一键安装（约 91MB，ModelScope 国内源），或首次使用方案3时按提示自动检索并一键安装。
- **安装过一次后**：模型存在本机 `~/.铃记忆体/models/`，往后升级版本、重装应用都会自动复用，无需重复下载。
- **模型作用**：让离线模式从"关键词匹配"升级为"语义匹配"（比如搜"天气好"能命中"今天天气很好"），更懂你的话。

> 不装模型也能正常使用：对话、记忆、工具箱、同步等全部功能照常，只是离线检索是关键词级别的。

## 📊 一图速览

- **离线文库**：1656 条回复，21 个分组（日常/安慰/吐槽/撒娇/日语/古风/情话…）
- **记忆分类**：兴趣爱好 / 工作学习 / 健康生活 / 家庭亲友 / 设备网络 / 日常对话
- **单测**：101 passed / 0 failed

---

## 🛠️ 技术栈

**Tauri 2 + Rust + Vue 3 + TypeScript + Pinia + Vite**（原生 CSS，不引入大型 UI 库）

- 后端：Rust（Tauri 2、reqwest、tokio、boa_engine JS 插件引擎、aes-gcm、qrcode/rqrr）
- 前端：Vue 3 + Pinia + vue-router，pnpm 管理依赖

## 📦 环境要求

- **Windows 10/11（x64）**
- **Node.js 18+ 与 pnpm**（`npm install -g pnpm`）
- **Rust**（最新稳定版，含 Cargo；`rustup` 安装，默认 MSVC 工具链）
- **Visual Studio Build Tools**：安装 "使用 C++ 的桌面开发" 工作负载（含 MSVC 编译器 + Windows 10/11 SDK），Tauri 在 Windows 构建必需

## 🚀 快速开始

```bash
# 1. 安装依赖
pnpm install

# 2. 启动开发窗口
pnpm tauri dev
```

首次运行会编译 Rust 后端，耗时较长属正常现象。

### 打包

```bash
# 生成 NSIS 安装包 + MSI 安装包
pnpm tauri build --bundles nsis msi
```

产物在 `src-tauri/target/release/bundle/` 下。

## 🧪 验证

```bash
pnpm run build          # 前端构建（vue-tsc + vite）
cd src-tauri && cargo check      # 后端检查
cd src-tauri && cargo test --lib # 单元测试
```

## 📁 目录结构

```
src/                        # 前端
├── components/             组件（ChatBubble / ToolboxPanel / SyncPanel / MemoryPanel / …）
├── views/                  页面（MainLayout / SettingView / RecoveryView）
├── stores/                 Pinia Store（chat / memory / setting / sync / …）
├── utils/tauri.ts          Tauri IPC 封装
└── types/index.ts          全局类型定义
src-tauri/                  # Rust 后端
├── src/
│   ├── commands/           Tauri 命令层
│   ├── engine/             对话引擎（script / api / local）
│   ├── memory/             记忆系统 + 分类引擎
│   ├── sync/               局域网同步（发现/传输/冲突/加密）
│   ├── vbil/               虚拟形象互联（TCP 服务 / 规则引擎 / 响应桥接 / 窗口扫描）
│   ├── plugin/             插件系统（boa_engine）
│   ├── update/             GitHub 更新检查
│   └── config/             配置中心（加密 / 存储 / 迁移）
└── resources/              内置资源（工具箱预设 / 离线文库 / 插件）
```

## 🔐 隐私与数据

- 所有记忆**只保存在本机**，不上传云端
- 数据目录：`~/.铃记忆体/`（配置 + 记忆 + 图片资源）
- 记忆主密码加密可选（PBKDF2-SHA256 10 万次 → AES-256-GCM）
- 三条代码契约：密钥唯一、配置唯一、记忆写锁唯一

## 🏷️ 版本历史

| 版本 | 内容 |
|------|------|
| v0.6.0 | 悬浮球 v7 全面重构（系统级拖动 + 边缘磁吸 + 单击/双击/长按分级 + 右键灵动控制面板 + 鼠标穿透 + 配置实时同步 + DPI 坐标修复 + 光晕防削边）|
| v0.5.5 | 悬浮球全面改进 v5/v6（内置猫娘头像 + Live2D 本地离线加载 Haru + DPI 拖拽修复 + 呼吸动画修复 + 位置持久化 + 滚动条隐藏）+ 悬浮球设置并入个性化 tab |
| v0.5.3 | 悬浮球全面改进（三种显示模式/大小/透明度/动画自定义 + Live2D 集成 oh-my-live2d + Haru/Shizuku 内置模型 + 设置页悬浮球 tab） |
| v0.5.2 | 新增 VBIL 虚拟形象互联（本地 TCP 服务 + 协议解析 + 心跳 + 规则引擎 + 响应桥接 + 窗口扫描 + 设置页） |
| v0.5.1 | 离线模式隐藏无意义 AI 参数 + 设置页新增离线语义检索模型入口 + 移除无用 OpenAI 卡片 + 配置 u8 容错 |
| v0.5.0 | 离线语义检索（bigram 默认 / jieba+BM25 / 约 91MB 向量）+ 检索模型一键安装（历史上曾分轻量版/完整版，v0.5.1 起回归单版本+可选模型） |
| v0.4.9 | 架构优化：记忆读缓存（5s TTL）/ 压缩脱锁 / WAL 追加写 / 同步限速（80 单测全过） |
| v0.4.8 | 离线检索增强三方案：bigram 倒排索引（默认）/ jieba+BM25 / 约 91MB 向量模型可选 + 离线增强方案面板（三档切换 + 内存检测） |
| v0.4.7 | 安全审查修复：logs 模块补全 / 主密码校验 / 同步配对码认证 / format-disk 二次确认 / JS 沙箱限制 / 密钥清零 / 低危 7 项 |
| v0.4.6 | 修复同步设备发现（UDP 广播响应器）+ 更新检查（代理支持 + 失败如实提示） |
| v0.4.5 | 记忆中心大项目第二阶段：特殊事件集 + 能力面板 + 记忆×工具联动 |
| v0.4.0 | 快捷指令 / 二维码 / OCR 双引擎 / 依赖管理器 / MSI 打包修复 / 旧数据迁移 / 主题系统 |
| v0.3.0 | 核心对话 + 记忆 + 工具箱批次 |

完整历史见 [Releases](https://github.com/Zang7311/-Memoria/releases)。

## 📄 License

本项目使用 [MIT License](./LICENSE) 开源发布，欢迎 Star / Fork / 提 Issue。

## 🤝 贡献

欢迎参与贡献！提 Issue / PR 前请阅读 [CONTRIBUTING.md](./CONTRIBUTING.md)。

## 🤖 AI 生成声明

本项目的部分实现由 Hermes Agent 生成，使用模型包括：DeepSeek V4 Pro、DeepSeek V4 Flash、DeepSeek V4 Flash Vision Exp、Qwen 3.7 Max、DeepSeek V3.2、GPT 网页免费版，Claude Sonnet 4.6
Claude Sonnet 5，Claude Opus 5，GLM 5.3。作者已对输出做了人工审查与调整，并维护 80+ 个单元测试。任何后续问题由 [@Zang7311](https://github.com/Zang7311) 负责修复。

## 🧠 嵌入模型声明（可选方案3）

- 语义检索（方案3）使用 [BAAI/FlagEmbedding](https://github.com/FlagOpen/FlagEmbedding) 的 **bge-small-zh-v1.5** 中文嵌入模型。
- 该模型基于 **MIT License** 发布（见 [FlagEmbedding](https://github.com/FlagOpen/FlagEmbedding/blob/master/LICENSE)），可免费商用。
- 来源：[ModelScope](https://modelscope.cn/models/AI-ModelScope/bge-small-zh-v1.5)，模型不打包进安装包，按需在应用内一键安装（`~/.铃记忆体/models/`）。
- 模型仅在用户开启「方案3：向量检索」时加载，其余情况不影响下载体积与内存占用。

## 🌿 分支说明

- **`main`（推荐）**：当前稳定主线，随版本持续更新（当前 v0.6.0）。
- **`v1.0`（实验分支）**：曾尝试"内置 Qwen2.5 小模型做离线对话"的实验分支（含 0.5B / 1.5B GGUF 模型）。**结论：小模型（1.5B 及以下）无法稳定承担复杂对话，体验不佳，此方案已放弃，不再内置模型。** 该分支代码与安装包仍可下载参考，但**不推荐日常使用**，请以 main 主线为准。
