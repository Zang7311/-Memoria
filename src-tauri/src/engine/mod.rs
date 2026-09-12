// 《铃·记忆体》引擎模块：三种对话模式 + 思考深度映射
pub mod script;
pub mod api;
pub mod local;
// 难度路由：闲聊走便宜模型、任务走主力模型（默认关闭，见 model_router.rs）
pub mod model_router;

/// 思考深度映射表：depth -> (temperature, top_p, reasoning_effort)
/// 在 API 模式与本地模式中应用
///
/// ⚠️ 必须返回 f64（不能用 f32）：f32 序列化进 JSON 时会被提升成 f64，
/// 产生长尾小数（如 0.9f32 → 0.8999999761581421），部分中转 API 会因此报
/// 「temperature 参数非法：限制小数点 2 位」而直接拒绝请求。
pub fn apply_depth(depth: u8) -> (f64, f64, &'static str) {
    match depth {
        1 => (0.7, 0.9, "low"),
        3 => (1.1, 0.98, "high"),
        4 => (1.3, 0.99, "high"),
        _ => (0.9, 0.95, "medium"), // 默认 depth=2 均衡
    }
}

/// 把采样参数规整到 2 位小数（部分中转 API 严格限制 2 位，超出即 400）
/// 例：0.8999999761581421 → 0.9
pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// depth=4（全力推理）时 max_tokens 翻倍
pub fn max_tokens_for_depth(depth: u8, base: u32) -> u32 {
    if depth >= 4 {
        base * 2
    } else {
        base
    }
}
