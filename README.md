# 铃·记忆体（Memoria）

Windows 桌面猫娘伴侣 —— 离线优先，具备对话、记忆、屏幕感知、本地 AI 推理能力。

技术栈：**Tauri 2 + Rust + Vue 3 + TypeScript + Pinia + Vite**（原生 CSS，不引入大型 UI 库）。

## 环境要求

- Node.js 18+ 与 pnpm
- Rust（最新稳定版，含 Cargo）

## 启动开发

```bash
pnpm install      # 安装依赖
pnpm tauri dev    # 启动 Tauri 开发窗口
```

首次运行会编译 Rust 后端，耗时较长属正常现象。

## 目录结构

```
src/
├── components/        组件（后续填充）
├── views/             页面（MainLayout / SettingView）
├── stores/            Pinia Store（chat / memory / setting）
├── utils/tauri.ts     Tauri IPC 封装
├── types/index.ts     全局类型定义
└── App.vue            根组件（验证 greet IPC）
src-tauri/            Rust 后端（greet 命令在此）
```

## 验证

启动后窗口应显示「你好，主人！铃已经准备好了。」—— 表示前后端 IPC 通信正常。
