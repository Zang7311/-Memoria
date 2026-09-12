// 《铃·记忆体》AI-10 Agent 能力模块入口
//
// 子模块：
//   - tools.rs           工具描述生成器（工具箱 + 插件 → OpenAI tools）
//   - router.rs          工具执行路由器（LLM tool_call → 现有执行器）
//   - loop_.rs           Agent 多轮循环核心（LLM function calling 循环）
//   - recovery.rs        错误恢复策略（诊断错误类型 → 给 LLM 换方案建议）
//   - stream_progress.rs Agent 流式进度推送（chat_chunk / chat_end）
//   - vision.rs          视觉理解（把图片送给多模态模型，实现「看图」）
//   - experience.rs      经验记忆（成功的工具序列沉淀下来，下次复用少走弯路）
//   - evaluator.rs       任务完成自评（独立一次调用核对任务是否真的完成，减少「假装完成」）
pub mod evaluator;
pub mod experience;
pub mod loop_;
pub mod planner;
pub mod recovery;
pub mod router;
pub mod stream_progress;
pub mod tools;
pub mod vision;
