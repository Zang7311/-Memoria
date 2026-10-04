# 任务：修复独立审查发现的问题（AI 难度判断这批改动）

## 项目

- 路径：`C:\Users\zang-服务器\Desktop\ling\mem`（Tauri 2 + Rust + Vue 3）
- **中文注释**；错误统一 `AppError`；前端 Vue 3 `<script setup lang="ts">`，UI 文案纯文字、不要 emoji
- 相关工作已在仓库里（`2b4de41` / `5fdfc18` / `4d1c939` + 未提交的便宜模型凭据改动）

## 背景

「AI 判断难度 + 视觉需求」这批改动过了独立审查，以下问题**已逐条回源码核实为真**，请全部修掉。

## 必须修的（🔴）

### A. 离线/本地模式下绝不能把消息发给云端分类器

`src-tauri/src/commands/send_message.rs` 的 `ai_enabled` 只检查了 `cfg.ai_router` 与模型类型，
**没检查 `cfg.model_mode`**。用户在 `script`（离线内置文库）或 `local`（Ollama）模式下，
消息仍会被发到云端分类接口（还多等最多 8 秒）——这破坏设置页「完全离线」的承诺。

- 修法：`ai_enabled` 追加条件 **`cfg.model_mode == "api"`**。
- 判据：`model_mode` 为 `script` / `local` 时，**一次分类请求都不发**，直接走本地关键词判断。

### B. 分类结果必须严格解析

`src-tauri/src/engine/model_router.rs` 的 `parse_verdict` 只用 `contains`：
`"not easy"` 会被判成 easy；只回 `"easy"`（缺视觉结论）也被采纳 → 可能把该干活的消息派给便宜模型。

- 修法：**严格**解析，要求**同时**拿到两个维度：
  - 难度：`easy` 或 `hard`
  - 视觉：`text` 或 `vision`
  缺任一维度 → 返回 `None`。
  匹配方式：先把回答按非字母数字切分成小写 token 再判断（避免 `easygoing` 这类子串误命中）；
  出现否定词（`not` / `no`）或同一维度两个值同时出现 → 返回 `None`（保守）。
- 判据（补测试）：`"easy text"` ✅、`"hard vision"` ✅、`"EASY TEXT"` ✅、
  `"easy"`（缺视觉）→ None、`"vision"`（缺难度）→ None、`"not easy text"` → None、
  `"easygoing"` → None、`""` / `"???"` → None、`"hard"` → None（缺视觉）。

### C. 长消息被截断时不得据此降级

`classify_with_ai` 只把**前 500 字**发给分类器，但路由决定作用于**整条消息**；
真正的任务或看图要求在后半段时会被判成 `easy` → 误降级到便宜模型。

- 修法：输入长度超过 500 字（按字符数）时，**直接 `return None`**（连请求都不发，省一次调用），
  交由本地关键词判断（长消息本地规则本来就保守给主力模型）。
- 判据：`> 500` 字符时函数不发网络请求、返回 `None`；`<= 500` 时行为不变。

### D. 模型能力判定要与前端统一，并引入「未知」

后端 `model_kind` 只认 `vision/vl/4v/image/omni/看图/视觉`；
而前端 `src/views/SettingView.vue` 的 `VISION_MODELS` 还包含 `4o` / `4.1` / `llava` / `gemini` / `gpt-4` / `claude` 等
→ `gpt-4o` 前端认视觉、后端当纯文本，两边打架。

- 修法：
  1. 把后端的关键词表**对齐前端**（至少覆盖：`vision` `vl` `4v` `4o` `4.1` `llava` `gemini` `gpt-4` `claude` `image` `omni`）
  2. 引入第三种取值 `ModelKind::Unknown`：**名字里没有任何线索时归 Unknown**
  3. `ai_router_allowed` 的规则：
     - 两边都已知且**不同** → `false`
     - 两边都 Unknown → **允许**（保持可用性：如 `deepseek-flash` + `deepseek-v4-pro` 这类同厂未知模型）
     - 其余组合（一个已知一个 Unknown）→ `false`（保守）
- 判据（补测试）：`gpt-4o` 与 `qwen-vl-max` → 都视觉 → 允许；
  `gpt-4o` 与 `deepseek-chat` → 视觉 vs Unknown → **拒绝**；
  `deepseek-flash` + `deepseek-v4-pro` → 都 Unknown → **允许**。

### E. 修「保存 API 密钥不生效」的既有 bug

`src-tauri/src/config/store.rs` 的 `update()` 里，处理 `api_key` 后
`return update(&updates)` **递归重读配置**，把刚写进 `cfg` 的密钥整段丢弃 → 保存密钥实际不生效。
（`cheap_api_key` 分支是新加的，也请确认没有同样问题。）

- 修法：不要让密钥处理结果在递归中丢失。推荐把「明文密钥 → 加密/明文存储」的赋值
  与函数末尾的 `save_config_file(&cfg)` 放在同一条路径上（例如：密钥处理只改 `cfg`、
  不提前 return，让流程走到函数末尾统一保存；其余字段照旧用去掉密钥键的 map 处理）。
- 判据（补测试）：调用 `update({api_key: "sk-test"})` 后，`get_config()` 能读到
  `api_key_plain == Some("sk-test")`（未解锁主密码场景）；`cheap_api_key` 同理。
- ⚠️ **不要改动加密算法与主密码开关的判断逻辑**，只修「改动被丢弃」。

## 建议修的（🟡）

### F. 便宜模型与主力模型同名时不算「两个模型」

`ai_router_allowed(Some("deepseek-chat"), "deepseek-chat")` 现在返回 `true`，但实际只有一个模型。
- 修法：`trim` 后若两者相等 → `false`。补测试（含两侧带空格的写法）。

### G. 路由状态要有归属、要及时清空

`src/stores/chatStore.ts` 的 `lastRoute` 是全局单值，切会话/新回复都不会清：
会话 A 的结果会显示在会话 B 上。

- 修法（最小实现即可）：
  1. `beginStream()`（开始新回复）时清空 `lastRoute`
  2. 切换会话 / 新建 / 清空消息时清空 `lastRoute`
  3. `chat_route` 事件载荷带上本次请求标识（如用已有的流式 request id 或 `sessionId`），
     前端只接受与「当前会话/当前请求」匹配的结果
- 判据：切换会话后不残留上一条路由；发新消息的等待期间不显示上一条的结果。

### H. 监听器就绪前不要发送

`src/composables/useStreamRender.ts` 用异步 `Promise.all([...])` 注册监听，但 `send()` 不等它完成，
存在丢 `chat_route` 事件的窗口。
- 修法：保存注册的 Promise，在调用后端发送命令前 `await` 它（或注册完成前禁用发送按钮）。

### I. 缓存策略要写清楚（不要求改行为）

缓存命中时不更新访问顺序 → 实际是 **FIFO**（上限 64），不是 LRU。
- 修法：**二选一**：① 保持 FIFO，但把注释与函数名写清楚是「FIFO 上限 64」；
  ② 改为真 LRU（命中时更新顺序）。改完在汇报里说明选了哪个。

## 明确不改（有意行为，不用动）

- **关闭 AI 判断时仍然推送 `chat_route` 并显示「本地规则（未开 AI 判断）」** ——
  这是产品有意设计（用户要求"显示调用状态"），不是回归。请在代码注释里写明这一点。

## 硬约束 ⭐

- **`ai_router=false` 或未配置便宜模型时，模型选择行为必须与改动前一字不差。**
- 任何路径都不能把密钥明文写进日志、事件或返回值。
- 不引入 panic；锁一律用 `unwrap_or_else(|e| e.into_inner())`。
- 不要碰人设（persona）与 Agent 权限代码；不要顺手重构无关模块。

## 必须做的验证

1. `cargo check --lib` 通过
2. `cargo test --lib` 全绿 —— **当前基线 245 passed / 0 failed，不许回归**
3. 为 A~H 每条补单元测试（无法单测的，写明原因与手测步骤）
4. `pnpm run build` 通过
5. 汇报：改了哪些文件、每条对应改在哪、测试数字、**有没有拿不准的地方**

## 交付

完成后简要汇报，并列出「已修 / 未修（含原因）」两栏。
