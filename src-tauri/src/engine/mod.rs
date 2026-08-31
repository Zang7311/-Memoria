// 《铃·记忆体》引擎模块：对话模式 + 思考深度映射
//
// v1.0 离线智能版的三档运行模式：
//   local_0b / local_1b → local_llm（内置 Qwen2.5 GGUF，纯 Rust candle，真离线对话）
//   api                 → api（OpenAI 兼容云端兜底）
// script 保留为最后兜底：模型文件缺失 / 加载失败时降级到内置回复库，保证"永远有回应"。
pub mod api;
pub mod local_llm;
pub mod script;

/// 思考深度映射表：depth -> (temperature, top_p, reasoning_effort)
/// 在 API 模式与本地模式中应用
pub fn apply_depth(depth: u8) -> (f32, f32, &'static str) {
    match depth {
        1 => (0.7, 0.9, "low"),
        3 => (1.1, 0.98, "high"),
        4 => (1.3, 0.99, "high"),
        _ => (0.9, 0.95, "medium"), // 默认 depth=2 均衡
    }
}

/// depth=4（全力推理）时 max_tokens 翻倍
pub fn max_tokens_for_depth(depth: u8, base: u32) -> u32 {
    if depth >= 4 {
        base * 2
    } else {
        base
    }
}
