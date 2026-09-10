# v0.6.0

## 修复

- 前端 bug 修复 13 处（悬浮球磁吸、单击/双击时序、IME 回车误触、会话切换竞态、流式卡死、插件权限错配等）
- 后端安全修复 3 项（Git URL 命令注入、参数占位符注入、明文 API Key 泄漏）
- 悬浮球"打开主窗口"白屏问题（v0.5.3 起存在，已修复）

## 安全

- Git URL 安装插件时增加白名单校验（仅允许 https:// 与 git@ 开头），禁止 ext::/file:// 等危险协议
- 插件系统命令执行时对参数值做双引号包裹 + 引号翻倍，防止 cmd /C 注入
- get_config 不再返回明文 API Key，前端只拿 has_api_key / has_plain_key 布尔标记

## 已知问题

- 部分杀毒软件可能误报（纯 Rust + Tauri 打包特性，可提交白名单）
- 语义检索模型（约 91MB）需在应用内「设置 → 离线语义检索模型」单独安装

---

Assets:
- Memoria_0.6.0_x64-setup.exe（NSIS 安装包）
- Memoria_0.6.0_x64_zh-CN.msi（MSI 安装包）
- Source code (zip)
- Source code (tar.gz)
