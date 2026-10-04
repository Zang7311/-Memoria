# 任务：给「便宜模型」补上独立的 API 地址与密钥

## 项目

- 路径：`C:\Users\zang-服务器\Desktop\ling\mem`（Tauri 2 + Rust + Vue 3）
- **中文注释**；错误统一 `AppError`；前端 Vue 3 `<script setup lang="ts">`，UI 文案**纯文字、不要 emoji**

## 背景（已核实，别照猜改）

难度路由里的「便宜模型」目前**只能和主力模型共用同一个 API 地址与密钥**：

- 后端 `src-tauri/src/types.rs` 的 `AppConfig`：只有 `api_base_url` / `api_key_plain` / `api_key_encrypted`
- `AppConfig` 也已有 `cheap_model: Option<String>`（只是模型名）
- IPC 结构 `Setting`（同文件，第 54 行附近）只有 `api_base_url` / `api_key` / `api_model`
- `src-tauri/src/commands/send_message.rs` 构造 `Setting` 时，把**主力的**地址和密钥填进去；
  于是用便宜模型跑聊天时，请求也发到主力的地址、用主力的密钥
- `classify_with_ai`（做难度判断那次调用）同样只拿到主力的地址与密钥
- 密钥保存：前端 `settingStore.saveApiKey()` 走 `update({ api_key: <明文> })`；
  后端 `src-tauri/src/config/store.rs` 的 `update()` 里**特判 `api_key`**：
  已解锁主密码 → AES 加密写入 `api_key_encrypted`；未设置 → 明文写 `api_key_plain`。
  （`api_key_encrypted` 也有单独特判，未解锁时拒绝覆盖）

**用户的意见原话**：「运行模式中可以选择模型名和便宜模型，但是只有模型名在下方配有 API 密钥的填写，
便宜模型却没有 API 密钥的填写位置」

**目标**：让便宜模型可以配置**自己的** API 地址与密钥（两份都可留空 = 回退用主力的），
从而支持「主力一个服务商、便宜模型另一个服务商」的混搭。

## 需求

### 后端

1. `AppConfig`（types.rs）新增三个字段（都 `#[serde(default)]`）：
   - `cheap_api_base_url: Option<String>`
   - `cheap_api_key_plain: Option<String>`
   - `cheap_api_key_encrypted: Option<String>`
   并在 `src-tauri/src/config/defaults.rs` 补默认值（`None`）。

2. `config/store.rs` 的 `update()`：照 `api_key` 那段**一模一样**再加一段特判 `cheap_api_key`：
   - 值为 null / 空串 → 清空 `cheap_api_key_encrypted` 与 `cheap_api_key_plain`
   - 已解锁主密码 → 加密写 `cheap_api_key_encrypted`、清空 plain
   - 未解锁 → 明文写 `cheap_api_key_plain`、清空 encrypted
   - `cheap_api_key_encrypted` 也照 `api_key_encrypted` 的特判处理（未解锁拒绝直接写）

3. `commands/send_message.rs`：
   - 新增 `decrypt_cheap_api_key(cfg)`（与现有 `decrypt_api_key` 同逻辑，改读 cheap 字段）；
     **cheap 两个字段都为空时返回 `None`**（表示"回退用主力的"，不要报错）
   - 构造 `Setting` 时带上：`cheap_api_base_url`（配置里的，未填则 `None`）与 `cheap_api_key`（解密结果）
   - `classify_with_ai` 调用改用**便宜模型的地址与密钥**：
     `cheap_api_base_url` 未填 → 用主力的 `api_base_url`；`cheap_api_key` 为 `None` → 用主力的 key

4. `Setting`（IPC 结构）新增：`cheap_api_base_url: Option<String>`、`cheap_api_key: Option<String>`（都 `#[serde(default)]`）。

5. `src-tauri/src/engine/api.rs`：发聊天请求时，**如果本次选中的模型就是 `cheap_model`**，
   则使用便宜模型的地址与密钥（未配置则回退主力的）—— 这是这次改动的核心行为。
   模型名与 `cheap_model` 的比较请按去空白后的字符串相等判断。

6. `commands/config_get.rs`（`GetConfigResponse`）：
   - 返回 `cheap_api_base_url`
   - 返回布尔标志 `has_cheap_api_key`（cheap 的明文或密文任一非空即 true）——
     **绝对不要返回密钥本身**（和现有 `has_api_key` / `has_plain_key` 的做法保持一致）

### 前端

7. `src/types/index.ts` 的配置类型补 `cheap_api_base_url?` / `has_cheap_api_key?`。

8. `src/stores/settingStore.ts`：
   - 参考现有 `apiBaseUrl` / `apiKeyInput` 的写法，添加 `cheapApiBaseUrl` 状态与读取赋值
   - 新增 `saveCheapApiKey(plain: string)`：走 `update({ cheap_api_key: plain })`
   - 在导出列表里补上新状态与方法

9. `src/views/SettingView.vue`：在「便宜模型」输入框**正下方**加两个输入框（纯文字文案）：
   - 「便宜模型 API 地址」，placeholder `留空 = 用上面的地址`，绑 `cheapApiBaseUrl`
   - 「便宜模型 API 密钥」，`type="password"`，placeholder `留空 = 用上面的密钥`，旁边一个保存按钮（文案跟主密钥按钮保持一致风格）
   - 保存时随配置一起提交 `cheap_api_base_url: cheapApiBaseUrl.value.trim() || null`
   - 密钥留空时不提交（避免把已存的密钥清掉）；输入框下方加一行小字提示
     「留空则沿用上面的地址与密钥；已保存过密钥时这里不回显」

## 硬约束 ⭐

- **两处都留空时，行为必须与改动前完全一致**（主力模型照旧、便宜模型照旧用主力的地址与密钥）。
- **任何路径都不能把密钥明文写进日志、事件或返回值**（`chat_route` 之类的事件里绝不能出现密钥）。
- 不得新增 IPC 命令；密钥保存复用现有的 `update_config` 通道。
- 不要动已有的 `api_key` / `api_key_encrypted` 逻辑（只在旁边加平行的 cheap 版本）。
- 不引入 panic；不要用 `unwrap()` 处理锁。
- 不要碰人设（persona）与 Agent 权限相关代码；不要顺手重构无关代码。

## 必须做的验证

1. `cargo check --lib` 通过
2. `cargo test --lib` 全绿 —— **当前基线 225 passed / 0 failed，不许回归**
3. 补单元测试（至少覆盖）：
   - cheap 地址/密钥为空时，选中的是便宜模型 → 仍然用主力的地址与密钥（守住默认行为）
   - cheap 地址/密钥有值时，选中的是便宜模型 → 用便宜的
   - 选中的是主力模型时，无论 cheap 怎么配 → 一律用主力的
4. `pnpm run build` 通过
5. 汇报改动文件清单与测试数字

## 交付

完成后简要说明：
- 改了哪些文件
- 「选中的模型 → 用哪套凭据」的判定逻辑写在哪里
- 测试结果（数字）
- **有没有你拿不准、需要我确认的地方**
