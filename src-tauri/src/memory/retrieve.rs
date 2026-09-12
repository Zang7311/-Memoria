// 《铃·记忆体》长期记忆相关性检索（memory/retrieve.rs）
//
// 背景：原先长期记忆是按「important 标签 + 时间新旧」挑的，跟当前话题无关也照样塞进
//       上下文 —— 库一大就白烧 token，同时真正相关的旧记忆又拿不到。
//
// 本模块改为「按相关性挑」：
//   1) important 记忆保底少量几条（用户显式标过的重要事，不该因为话题不同就丢）
//   2) 再用本地检索引擎按相关性补充（bm25 优先，无命中降级 bigram，零 LLM 成本）
//   3) 还不够则按时间倒序补最近的，保证不断档
//
// 短期记忆（当前会话的历史消息）不在这里处理 —— 那部分照旧通读，保证对话流畅。
use std::collections::HashSet;

use crate::memory::search::search_with_mode;
use crate::types::Memory;

/// important 记忆的保底条数上限（不会超过总条数上限的一半）
const IMPORTANT_MAX: usize = 2;

/// 该记忆是否被用户标为重要
fn is_important(m: &Memory) -> bool {
    m.tags
        .as_ref()
        .is_some_and(|t| t.iter().any(|x| x == "important"))
}

/// 检索与当前输入最相关的长期记忆。
///
/// # 参数
/// - `all`: 全库记忆（调用方已按追加顺序排好，越靠后越新）
/// - `query`: 当前用户输入（用作检索词）
/// - `limit`: 最多返回多少条（0 表示不注入长期记忆）
///
/// # 返回
/// 按「important 保底 → 相关性 → 时间兜底」优先级排好的记忆列表。
pub fn retrieve_long_term(all: &[Memory], query: &str, limit: usize) -> Vec<Memory> {
    if all.is_empty() || limit == 0 {
        return Vec::new();
    }

    let mut picked: Vec<Memory> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // 1) important 保底：用户显式标过的重要记忆，最多 IMPORTANT_MAX 条（取最新的）。
    //    ⚠️ 不能写 .max(1)：limit = 1 时「2.min(0).max(1) = 1」会把唯一的名额全给
    //    important，相关性检索与时间兜底彻底没有机会 —— 反而违背「按相关性挑」的初衷。
    //    limit 很小时允许 important_quota = 0，让相关性做主。
    let important_quota = IMPORTANT_MAX.min(limit / 2);
    if important_quota > 0 {
        for m in all.iter().rev().filter(|m| is_important(m)) {
            if picked.len() >= important_quota {
                break;
            }
            if seen.insert(m.id.clone()) {
                picked.push(m.clone());
            }
        }
    }

    // 2) 相关性补充。
    //    bigram 是「多关键词 AND」精确匹配，先用它；没命中再用 bm25 试一次
    //    （jieba 分词对近义表达召回更好）。
    //    ⚠️ 注意：bm25 对任何查询都会返回结果（分数可能为 0，且顺序是旧→新），
    //    所以只有它「确实筛掉了东西」才算有效命中 —— 否则宁可交给时间兜底。
    if picked.len() < limit && !query.trim().is_empty() {
        let mut hits = search_with_mode(all, query, "bigram");

        if hits.is_empty() {
            let bm = search_with_mode(all, query, "bm25");
            if bm.len() < all.len() {
                hits = bm;
            } else {
                log::debug!("[retrieve] bm25 未筛掉任何记忆，视为无命中");
            }
        }

        for m in hits {
            if picked.len() >= limit {
                break;
            }
            if seen.insert(m.id.clone()) {
                picked.push(m);
            }
        }
    }

    // 3) 时间兜底：仍不满就补最近的，保证上下文不断档
    if picked.len() < limit {
        for m in all.iter().rev() {
            if picked.len() >= limit {
                break;
            }
            if seen.insert(m.id.clone()) {
                picked.push(m.clone());
            }
        }
    }

    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem(id: &str, content: &str, important: bool) -> Memory {
        Memory {
            id: id.to_string(),
            role: "user".to_string(),
            content: content.to_string(),
            timestamp: String::new(),
            tags: if important {
                Some(vec!["important".to_string()])
            } else {
                None
            },
            summary: None,
            category: None,
            use_count: 0,
        }
    }

    #[test]
    fn 空库返回空() {
        assert!(retrieve_long_term(&[], "随便问问", 5).is_empty());
    }

    #[test]
    fn 上限为0时不注入() {
        let all = vec![mem("1", "主人喜欢猫", false)];
        assert!(retrieve_long_term(&all, "猫", 0).is_empty());
    }

    #[test]
    fn 上限为1时按相关性挑而不是被important独占() {
        // 回归测试：曾经写成 `IMPORTANT_MAX.min(limit / 2).max(1)`，
        // limit=1 时算出来是 1 —— 唯一的名额全给 important，
        // 相关性检索和时间兜底彻底没机会，等于没做检索。
        let mut all = vec![mem("imp", "主人的生日是 3 月 14 日", true)];
        all.push(mem("rel", "主人最喜欢的游戏是千恋万花", false));

        let got = retrieve_long_term(&all, "我喜欢的游戏是什么", 1);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, "rel", "limit=1 时应给最相关的那条，而不是 important");
    }

    #[test]
    fn 相关性优先_拣出与提问相关的旧记忆() {
        // 库里塞满无关内容，一条相关的旧记忆在很前面
        let mut all = vec![mem("old", "主人最喜欢的游戏是千恋万花", false)];
        for i in 0..20 {
            all.push(mem(&format!("n{i}"), "今天天气不错呀", false));
        }
        all.push(mem("newest", "嗯嗯", false));

        let got = retrieve_long_term(&all, "我喜欢的游戏是什么", 3);

        assert!(
            got.iter().any(|m| m.id == "old"),
            "应该拣出相关的那条旧记忆，实际拿到：{:?}",
            got.iter().map(|m| &m.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn 重要记忆保底_即使不相关也在() {
        let mut all = vec![mem("imp", "主人的生日是 3 月 14 日", true)];
        for i in 0..10 {
            all.push(mem(&format!("n{i}"), "随便聊聊别的事情", false));
        }

        let got = retrieve_long_term(&all, "帮我看看磁盘空间", 4);
        assert!(
            got.iter().any(|m| m.id == "imp"),
            "important 记忆应保底存在"
        );
    }

    #[test]
    fn 不超过上限() {
        let mut all = Vec::new();
        for i in 0..30 {
            all.push(mem(&format!("n{i}"), "内容内容内容", false));
        }
        assert_eq!(retrieve_long_term(&all, "内容", 5).len(), 5);
        assert_eq!(retrieve_long_term(&all, "内容", 1).len(), 1);
    }

    #[test]
    fn 无命中时按时间兜底_不断档() {
        let mut all = Vec::new();
        for i in 0..10 {
            all.push(mem(&format!("n{i}"), "完全无关的内容", false));
        }

        let got = retrieve_long_term(&all, "zzzzz不存在的词zzzzz", 3);
        assert_eq!(got.len(), 3, "即使检索无命中也要补满，保证不断档");
        // 兜底应取最新的（数组末尾），且顺序是 最新的在前
        assert_eq!(got[0].id, "n9");
        assert_eq!(got[1].id, "n8");
    }

    #[test]
    fn 结果不重复() {
        let mut all = vec![mem("a", "重复内容重复内容", true)];
        for _ in 0..5 {
            all.push(mem("a", "重复内容重复内容", true)); // 同 id 多条，模拟脏数据
        }

        let got = retrieve_long_term(&all, "重复内容", 5);
        let mut ids: Vec<&String> = got.iter().map(|m| &m.id).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(n, ids.len(), "结果里不该有重复 id");
    }

    #[test]
    fn 空查询也能兜底返回() {
        let mut all = Vec::new();
        for i in 0..5 {
            all.push(mem(&format!("n{i}"), "内容", false));
        }
        let got = retrieve_long_term(&all, "   ", 2);
        assert_eq!(got.len(), 2, "空查询应走时间兜底，而不是返回空");
    }
}
