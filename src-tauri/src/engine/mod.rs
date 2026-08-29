// 《铃·记忆体》引擎模块：三种对话模式 + 思考深度映射
pub mod script;
pub mod api;
pub mod local;

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
