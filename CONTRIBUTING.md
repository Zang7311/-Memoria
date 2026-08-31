# 贡献指南

感谢你对「铃·记忆体」感兴趣！以下是参与贡献的方式与规范。

## 如何参与

- **提 Bug / 建议**：在 [Issues](https://github.com/Zang7311/-Memoria/issues) 新建 Issue，描述清楚复现步骤与环境（Windows 版本、内存、是否开启了某项功能）。
- **提交代码**：Fork 本仓库 → 创建分支 → 修改 → 提交 PR，PR 描述请说明改动目的与验证方式。

## 开发环境

参见 [README.md](./README.md) 的「环境要求」与「快速开始」。

## 代码规范

- 后端：Rust（Tauri 2），遵循 `cargo fmt` / `cargo clippy`，新增公开函数应有文档注释。
- 前端：Vue 3 + TypeScript + Pinia，类型标注完整，不使用 any 逃逸（除非有明确理由并注释）。
- 提交信息：`type: 简述`，type 取 `feat` / `fix` / `docs` / `refactor` / `chore`。

## 验证

提交前请确认通过：

```bash
pnpm run build                    # 前端构建（vue-tsc + vite）
cd src-tauri && cargo check       # 后端编译检查
cd src-tauri && cargo test --lib  # 单元测试（当前 76+ 通过）
```

## 测试数据说明

当前处于开发阶段，本地记忆/配置均为测试数据，无需迁移或备份。

## License

本项目以 [MIT License](./LICENSE) 发布。
