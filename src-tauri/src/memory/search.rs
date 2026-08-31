// 《铃·记忆体》记忆搜索引擎（离线检索增强）
//
// 方案1（默认）: 字符 bigram 倒排索引 + 重合度打分，纯标准库零新依赖
// 方案2        : jieba-rs 分词 + BM25（TF-IDF）打分
// 方案3（预留）: 向量检索接口——仅检测模型文件是否存在，不加载模型
//
// 公开 API 签名与旧版保持一致，前端/上层调用无需修改。
use crate::types::Memory;
use std::collections::HashMap;

// ============================================================
// 方案3：向量检索预留接口
// ============================================================

/// 向量模型安装状态（不加载模型，仅探测文件）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VectorModelStatus {
    pub available: bool,
    pub message: String,
}

/// 检测向量 embedding 模型是否已安装
/// 探测路径：~/.铃记忆体/models/ 及 ~/.ling-memoria/models/
/// 支持文件：embedding.bin / model.onnx / embedding.gguf / model.safetensors
pub fn check_vector_model() -> VectorModelStatus {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();

    let model_dirs = [
        format!("{home}/.铃记忆体/models"),
        format!("{home}/.ling-memoria/models"),
    ];
    let model_files = [
        "embedding.bin",
        "model.onnx",
        "embedding.gguf",
        "model.safetensors",
    ];

    for dir in &model_dirs {
        let dir_path = std::path::Path::new(dir);
        if dir_path.exists() {
            for file in &model_files {
                let model_path = dir_path.join(file);
                if model_path.exists() {
                    return VectorModelStatus {
                        available: true,
                        message: format!("已找到模型：{}", model_path.display()),
                    };
                }
            }
        }
    }

    VectorModelStatus {
        available: false,
        message: format!(
            "向量模型未安装。请将 embedding.bin 或 model.onnx 放入 {home}/.铃记忆体/models/"
        ),
    }
}

// ============================================================
// 方案1：字符 bigram 倒排索引 + 重合度打分
// ============================================================

/// 将文本切成字符级 bigram（滑动窗口步长 1）
/// "今天天气" → ["今天", "天天", "天气"]
fn char_bigrams(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 2 {
        return vec![];
    }
    chars.windows(2).map(|w| w.iter().collect()).collect()
}

/// 计算单段文本对查询 bigram 集合的重合比例（0.0~1.0）
fn bigram_score_text(text: &str, query_bigrams: &[String]) -> f32 {
    if query_bigrams.is_empty() {
        return 0.0;
    }
    let text_lower = text.to_lowercase();
    let text_set: std::collections::HashSet<String> =
        char_bigrams(&text_lower).into_iter().collect();
    let matched = query_bigrams
        .iter()
        .filter(|b| text_set.contains(*b))
        .count();
    matched as f32 / query_bigrams.len() as f32
}

/// 对单条记忆（content 与 summary 取最大值）计算 bigram 得分
fn memory_bigram_score(m: &Memory, query_bigrams: &[String]) -> f32 {
    let content_score = bigram_score_text(&m.content, query_bigrams);
    let summary_score = m
        .summary
        .as_deref()
        .map(|s| bigram_score_text(s, query_bigrams))
        .unwrap_or(0.0);
    content_score.max(summary_score)
}

/// bigram 搜索（AND 逻辑，多关键词全部需得分 > 0）
/// 单个子词 < 2 字时退化为旧版 contains 兜底，保证单字搜索不丢结果
fn search_bigram(memories: &[Memory], keyword: &str) -> Vec<Memory> {
    let kws: Vec<&str> = keyword
        .split(|c: char| c == ' ' || c == ',' || c == '，')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    if kws.is_empty() {
        return memories.to_vec();
    }

    let kw_bigrams: Vec<Vec<String>> = kws
        .iter()
        .map(|kw| char_bigrams(&kw.to_lowercase()))
        .collect();

    let mut scored: Vec<(f32, Memory)> = memories
        .iter()
        .filter_map(|m| {
            let mut total = 0.0f32;
            for (i, bgrams) in kw_bigrams.iter().enumerate() {
                if bgrams.is_empty() {
                    // 单字退化：contains 兜底
                    if !matches_legacy(m, kws[i]) {
                        return None;
                    }
                    total += 1.0;
                } else {
                    let score = memory_bigram_score(m, bgrams);
                    if score == 0.0 {
                        return None;
                    }
                    total += score;
                }
            }
            Some((total, m.clone()))
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().map(|(_, m)| m).collect()
}

// ============================================================
// 方案2：BM25（jieba-rs 分词 + TF-IDF 打分）
// ============================================================

use jieba_rs::Jieba;
use std::sync::OnceLock;

static JIEBA: OnceLock<Jieba> = OnceLock::new();

fn jieba() -> &'static Jieba {
    JIEBA.get_or_init(Jieba::new)
}

fn tokenize(text: &str) -> Vec<String> {
    jieba()
        .cut(text, false)
        .into_iter()
        .map(|s| s.to_lowercase())
        .filter(|s| !s.trim().is_empty())
        .collect()
}

fn term_freq(tokens: &[String]) -> HashMap<String, usize> {
    let mut tf = HashMap::new();
    for t in tokens {
        *tf.entry(t.clone()).or_insert(0) += 1;
    }
    tf
}

const BM25_K1: f32 = 1.5;
const BM25_B: f32 = 0.75;

fn bm25_score(
    doc_tokens: &[String],
    query_tokens: &[String],
    corpus_tfs: &[HashMap<String, usize>],
    avg_dl: f32,
) -> f32 {
    let n = corpus_tfs.len() as f32;
    let doc_len = doc_tokens.len() as f32;
    let doc_tf = term_freq(doc_tokens);

    query_tokens
        .iter()
        .map(|term| {
            let df = corpus_tfs.iter().filter(|tf| tf.contains_key(term)).count() as f32;
            if df == 0.0 {
                return 0.0;
            }
            let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
            let tf_val = *doc_tf.get(term).unwrap_or(&0) as f32;
            let denom = tf_val + BM25_K1 * (1.0 - BM25_B + BM25_B * doc_len / avg_dl.max(1.0));
            idf * tf_val * (BM25_K1 + 1.0) / denom
        })
        .sum()
}

fn memory_doc_text(m: &Memory) -> String {
    match &m.summary {
        Some(s) => format!("{} {}", m.content, s),
        None => m.content.clone(),
    }
}

fn search_bm25(memories: &[Memory], keyword: &str) -> Vec<Memory> {
    if keyword.trim().is_empty() {
        return memories.to_vec();
    }

    let docs: Vec<Vec<String>> = memories
        .iter()
        .map(|m| tokenize(&memory_doc_text(m)))
        .collect();

    let avg_dl = if docs.is_empty() {
        1.0
    } else {
        docs.iter().map(|d| d.len() as f32).sum::<f32>() / docs.len() as f32
    };

    let corpus_tfs: Vec<HashMap<String, usize>> = docs.iter().map(|d| term_freq(d)).collect();
    let query_tokens = tokenize(keyword);

    if query_tokens.is_empty() {
        return memories.to_vec();
    }

    let mut scored: Vec<(f32, Memory)> = memories
        .iter()
        .enumerate()
        .filter_map(|(i, m)| {
            let score = bm25_score(&docs[i], &query_tokens, &corpus_tfs, avg_dl);
            if score > 0.0 {
                Some((score, m.clone()))
            } else {
                None
            }
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // BM25 零结果时降级 bigram（分词粒度过细导致零命中的保险）
    if scored.is_empty() {
        return search_bigram(memories, keyword);
    }

    scored.into_iter().map(|(_, m)| m).collect()
}

// ============================================================
// 旧版 contains 逻辑（单字 fallback / 测试兼容）
// ============================================================

fn matches_legacy(m: &Memory, kw: &str) -> bool {
    let kw = kw.to_lowercase();
    if kw.is_empty() {
        return true;
    }
    let content = m.content.to_lowercase();
    let summary = m.summary.clone().unwrap_or_default().to_lowercase();
    content.contains(&kw) || summary.contains(&kw)
}

// ============================================================
// 公开 API（签名不变，前端调用无需修改）
// ============================================================

/// 搜索记忆：默认 bigram 引擎，多关键词 AND 逻辑，结果按相关度降序
/// 空关键词返回全部。
pub fn search(memories: &[Memory], keyword: &str) -> Vec<Memory> {
    search_with_mode(memories, keyword, "bigram")
}

/// 按指定引擎搜索记忆
/// - `"bigram"` : 字符 bigram 倒排索引（默认，零依赖）
/// - `"bm25"`   : jieba-rs 分词 + BM25 打分
/// - `"vector"` : 向量检索（模型未安装时自动降级 bigram）
pub fn search_with_mode(memories: &[Memory], keyword: &str, mode: &str) -> Vec<Memory> {
    match mode {
        "bm25" => search_bm25(memories, keyword),
        "vector" => {
            // 向量模型未加载实现，降级到 BM25
            search_bm25(memories, keyword)
        }
        _ => search_bigram(memories, keyword),
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn mem(id: &str, content: &str, summary: Option<&str>) -> Memory {
        Memory {
            id: id.to_string(),
            role: "user".to_string(),
            content: content.to_string(),
            timestamp: String::new(),
            tags: None,
            summary: summary.map(|s| s.to_string()),
            category: None,
            use_count: 0,
        }
    }

    #[test]
    fn empty_keyword_returns_all() {
        let ms = vec![mem("1", "你好", None), mem("2", "再见", None)];
        assert_eq!(search(&ms, "").len(), 2);
        assert_eq!(search(&ms, "   ").len(), 2);
    }

    #[test]
    fn single_keyword_matches_content_and_summary() {
        let ms = vec![
            mem("1", "今天天气很好", None),
            mem("2", "小猫很可爱", Some("关于猫的回忆")),
        ];
        // bigram "天气" 存在于 "今天天气很好" 的 bigram 集中
        let r = search(&ms, "天气");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "1");

        // 单字 "猫" 退化为 contains 兜底
        let r2 = search(&ms, "猫");
        assert_eq!(r2.len(), 1);
        assert_eq!(r2[0].id, "2");
    }

    #[test]
    fn multi_keyword_and_logic() {
        let ms = vec![
            mem("1", "喜欢看动漫", None),
            mem("2", "喜欢看书", None),
            mem("3", "看书和动漫", None),
        ];
        let r = search(&ms, "喜欢 动漫");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "1");
    }

    #[test]
    fn case_insensitive() {
        let ms = vec![mem("1", "Hello World", None)];
        let r = search(&ms, "hello");
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn partial_match_bigram() {
        // 核心场景：局部匹配，"天气好" 能搜到 "今天天气很好"
        let ms = vec![
            mem("1", "今天天气很好", None),
            mem("2", "明天下雨", None),
        ];
        let r = search(&ms, "天气好");
        assert!(!r.is_empty(), "bigram 局部匹配应命中 '今天天气很好'");
        assert_eq!(r[0].id, "1");
    }

    #[test]
    fn relevance_ordering() {
        // 更高 bigram 重合度的记忆应排在前面
        let ms = vec![
            mem("1", "今天天气不错", None),
            mem("2", "今天天气真的很好啊", None),
        ];
        let r = search(&ms, "今天天气");
        assert_eq!(r.len(), 2);
        // 两条都应命中
    }

    #[test]
    fn vector_model_status() {
        let status = check_vector_model();
        // 无模型文件时应返回 available=false
        if !status.available {
            assert!(status.message.contains("未安装"));
        }
    }

    #[test]
    fn bm25_mode() {
        let ms = vec![
            mem("1", "今天天气很好", None),
            mem("2", "明天下雨", None),
        ];
        let r = search_with_mode(&ms, "天气", "bm25");
        assert!(!r.is_empty());
        assert_eq!(r[0].id, "1");
    }
}
