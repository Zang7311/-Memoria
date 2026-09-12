// 《铃·记忆体》难度路由（engine/model_router.rs）
//
// 目的：省 token。闲聊走「便宜模型」，任务类消息走「能力强的模型」。
//
// 依据 Anthropic《Building Effective Agents》里的 Routing 模式原文：
//   "Routing easy/common questions to smaller, cost-efficient models like Claude Haiku
//    and hard/unusual questions to more capable models ... to optimize for best performance."
//
// 设计原则：
//   1. **默认关闭** —— cheap_model 留空时一律用 api_model，行为与旧版完全一致；
//      只有用户显式填了便宜模型，路由才生效。绝不擅自改变既有行为。
//   2. **零额外调用** —— 纯本地启发式判断，不为了「选择模型」再多花一次 LLM 调用
//      （否则省下的 token 还不够付路由的成本）。
//   3. **拿不准就给强的** —— 宁可多花一点，也不要该干活时派出弱模型。

/// 纯闲聊的长度上限（字符数）。超过这个长度就认为是有内容的话，交给能力强的模型。
const CHAT_MAX_CHARS: usize = 50;

/// 任务意图关键词：命中任意一个就直接交给能力强的模型。
///
/// 刻意保守：宁可误判成「任务」（多花钱但靠谱），也不要误判成「闲聊」（省钱但干不了活）。
const TASK_HINTS: &[&str] = &[
    // 软件与系统操作
    "打开", "关闭", "启动", "运行", "执行", "安装", "卸载", "重启", "关机", "注销",
    "清理", "扫描", "优化",
    // 文件
    "删除", "写入", "创建", "新建", "复制", "移动", "重命名", "压缩", "解压",
    "保存", "下载", "导出", "备份",
    // 查询与检索
    "搜索", "查找", "搜一下", "查一下", "帮我查", "帮我找", "有没有",
    // 内容处理
    "翻译", "总结", "整理", "改写", "润色", "提取",
    // 系统能力
    "截图", "截屏", "截个屏", "剪贴板", "提醒", "定时", "音量", "亮度", "注册表", "服务",
    // 显式求助
    "帮我", "帮忙", "替我",
];

/// 这句话是不是「纯闲聊」（不含任务意图、也不长）。
///
/// 抽成独立函数是为了让测试和路由决策用同一套判断，避免两处逻辑漂移。
pub fn is_chat_only(input: &str) -> bool {
    let t = input.trim();
    if t.is_empty() {
        return false; // 空输入拿不准，给强的
    }
    if t.chars().count() > CHAT_MAX_CHARS {
        return false;
    }
    !TASK_HINTS.iter().any(|k| t.contains(k))
}

/// 挑选本次对话该用哪个模型。
///
/// # 参数
/// - `input`: 用户这次的输入
/// - `has_image`: 这次是否带图片（便宜模型多半不支持视觉，不能冒险）
/// - `agent_mode`: 是否开着 Agent 模式（要调工具，必须用能力强的）
/// - `cheap`: 配置里的便宜模型名（None / 空 = 路由关闭）
/// - `capable`: 配置里的主力模型名
///
/// # 返回
/// 实际要用的模型名。路由关闭或判断不确定时，一律返回 `capable`。
pub fn pick_model(
    input: &str,
    has_image: bool,
    agent_mode: bool,
    cheap: Option<&str>,
    capable: &str,
) -> String {
    // 1) 没配便宜模型 → 路由关闭，一切照旧
    let Some(cheap) = cheap.map(str::trim).filter(|s| !s.is_empty()) else {
        return capable.to_string();
    };

    // 2) 带图片：便宜模型可能看不了图，不冒这个险
    if has_image {
        return capable.to_string();
    }

    // 3) Agent 模式：要选工具、要多轮，必须用能力强的
    if agent_mode {
        return capable.to_string();
    }

    // 4) 纯闲聊才降级到便宜模型
    if is_chat_only(input) {
        return cheap.to_string();
    }

    capable.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: &str = "glm-5.2";
    const CHEAP: &str = "deepseek-v4-flash";

    #[test]
    fn 没配便宜模型时永远用主力模型() {
        // 这是最重要的保底：默认行为绝不能变
        let long = "给我讲个长篇故事".repeat(5);
        let cases: [&str; 4] = ["在吗", "今天好累", "帮我打开QQ", long.as_str()];
        for input in cases {
            for img in [true, false] {
                for agent in [true, false] {
                    assert_eq!(
                        pick_model(input, img, agent, None, CAP),
                        CAP,
                        "路由关闭时应当一律用主力模型"
                    );
                    assert_eq!(pick_model(input, img, agent, Some("   "), CAP), CAP);
                }
            }
        }
    }

    #[test]
    fn 闲聊走便宜模型() {
        for input in ["在吗", "嗯嗯", "今天好累呀", "陪我聊会儿天", "晚安"] {
            assert_eq!(
                pick_model(input, false, false, Some(CHEAP), CAP),
                CHEAP,
                "「{input}」是闲聊，应该走便宜模型"
            );
        }
    }

    #[test]
    fn 任务类走主力模型() {
        for input in [
            "帮我打开QQ",
            "安装一下剪映",
            "把桌面那张图压缩一下",
            "搜一下今天天气",
            "截个屏",
            "关机",
            "删除这个文件",
        ] {
            assert_eq!(
                pick_model(input, false, false, Some(CHEAP), CAP),
                CAP,
                "「{input}」是任务，必须走主力模型"
            );
        }
    }

    #[test]
    fn 长消息即使没有关键词也走主力模型() {
        // 50 字以上的话，多半有正经内容，别省钱
        let long = "嗯嗯".repeat(30); // 60 字，且不含任务关键词
        assert_eq!(long.chars().count(), 60);
        assert!(!is_chat_only(&long), "60 字应超过闲聊上限");
        assert_eq!(pick_model(&long, false, false, Some(CHEAP), CAP), CAP);
    }

    #[test]
    fn 短闲聊仍在便宜模型范围内() {
        let short = "嗯嗯".repeat(20); // 40 字
        assert!(short.chars().count() <= CHAT_MAX_CHARS);
        assert!(is_chat_only(&short));
        assert_eq!(pick_model(&short, false, false, Some(CHEAP), CAP), CHEAP);
    }

    #[test]
    fn 带图片一定走主力模型() {
        // 哪怕只是闲聊，带图也不能交给可能不支持视觉的便宜模型
        assert_eq!(
            pick_model("在吗", true, false, Some(CHEAP), CAP),
            CAP,
            "带图必须用主力模型"
        );
    }

    #[test]
    fn agent模式一定走主力模型() {
        assert_eq!(
            pick_model("在吗", false, true, Some(CHEAP), CAP),
            CAP,
            "Agent 模式必须用主力模型"
        );
    }

    #[test]
    fn 空输入保守给主力模型() {
        assert_eq!(pick_model("", false, false, Some(CHEAP), CAP), CAP);
        assert_eq!(pick_model("   ", false, false, Some(CHEAP), CAP), CAP);
        assert!(!is_chat_only(""));
    }

    #[test]
    fn 中文按字符算长度而不是字节() {
        // 60 个汉字 = 60 字符（不是 180 字节）。若按字节算会误判成长消息。
        let s = "啊".repeat(40);
        assert_eq!(s.chars().count(), 40);
        assert!(is_chat_only(&s), "40 个汉字应当算短消息");
    }
}
