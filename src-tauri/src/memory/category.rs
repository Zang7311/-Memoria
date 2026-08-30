// 《铃·记忆体》记忆分类引擎（记忆中心）
// 规则关键词分类：写入记忆时自动打分类标签，供记忆中心分组展示
use crate::types::Memory;

/// 分类规则：分类名 -> 关键词列表（按优先级排列，先命中先得）
const CATEGORY_RULES: &[(&str, &[&str])] = &[
    ("兴趣爱好", &["喜欢", "爱好", "像素画", "画画", "画画", "绘画", "音乐", "听歌", "唱歌", "动漫", "追番", "看番", "漫画", "小说", "读书", "摄影", "游戏", "打游戏", "我的世界", "mc", "王者", "原神", "steam", "galgame", "gal", "cosplay", "手办", "模型"]),
    ("工作学习", &["上班", "工作", "加班", "打工", "写代码", "代码", "编程", "项目", "开会", "老板", "同事", "作业", "学习", "考试", "复习", "考研", "论文", "毕业", "课程", "实习", "面试", "简历"]),
    ("健康生活", &["生病", "感冒", "发烧", "头疼", "肚子疼", "身体", "吃药", "医院", "锻炼", "跑步", "健身", "运动", "减肥", "熬夜", "睡眠", "睡觉", "失眠", "饮食", "吃饭", "早餐", "午餐", "晚餐", "外卖"]),
    ("家庭亲友", &["妈妈", "我妈", "我爸", "爸爸", "家人", "爸妈", "弟弟", "妹妹", "哥哥", "姐姐", "爷爷", "奶奶", "外婆", "外公", "朋友", "同学", "室友", "对象", "女朋友", "男朋友", "老婆", "老公", "孩子", "儿子", "女儿"]),
    ("设备网络", &["电脑", "笔记本", "显卡", "cpu", "内存", "硬盘", "固态", "系统", "windows", "软件", "驱动", "网络", "wifi", "宽带", "路由器", "手机", "iphone", "安卓", "耳机", "键盘", "鼠标", "显示器"]),
];

/// 分类中文标签（未匹配到的用"日常对话"）
pub const DEFAULT_CATEGORY: &str = "日常对话";

/// 根据内容自动分类（返回分类名）
pub fn classify(content: &str) -> String {
    for (cat, kws) in CATEGORY_RULES {
        if kws.iter().any(|k| content.contains(k)) {
            return (*cat).to_string();
        }
    }
    DEFAULT_CATEGORY.to_string()
}

/// 给记忆打分类（就地修改 category 字段，若未设置）
pub fn apply_category(m: &mut Memory) {
    if m.category.is_none() {
        m.category = Some(classify(&m.content));
    }
}

/// 统计分类分布（记忆中心用）：返回 (分类名, 数量) 列表，按数量降序
pub fn category_distribution(memories: &[Memory]) -> Vec<(String, usize)> {
    use std::collections::HashMap;
    let mut map: HashMap<String, usize> = HashMap::new();
    for m in memories {
        let cat = m.category.clone().unwrap_or_else(|| DEFAULT_CATEGORY.to_string());
        *map.entry(cat).or_insert(0) += 1;
    }
    let mut v: Vec<(String, usize)> = map.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_basic() {
        assert_eq!(classify("我喜欢玩我的世界"), "兴趣爱好");
        assert_eq!(classify("今天上班好累"), "工作学习");
        assert_eq!(classify("我感冒了难受"), "健康生活");
        assert_eq!(classify("我妈让我早点回家"), "家庭亲友");
        assert_eq!(classify("显卡驱动更新了"), "设备网络");
        assert_eq!(classify("今天天气不错"), "日常对话");
    }

    #[test]
    fn distribution() {
        let m1 = Memory { id: "1".into(), role: "user".into(), content: "喜欢像素画".into(), timestamp: "t".into(), tags: None, summary: None, category: Some("兴趣爱好".into()), use_count: 0 };
        let m2 = Memory { id: "2".into(), role: "user".into(), content: "加班".into(), timestamp: "t".into(), tags: None, summary: None, category: Some("工作学习".into()), use_count: 0 };
        let m3 = Memory { id: "3".into(), role: "user".into(), content: "跑步".into(), timestamp: "t".into(), tags: None, summary: None, category: None, use_count: 0 };
        let dist = category_distribution(&[m1, m2, m3]);
        assert!(dist.iter().any(|(c, n)| c == "日常对话" && *n == 1));
    }
}
