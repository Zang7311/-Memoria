// 方案3：向量检索 —— BAAI/bge-small-zh-v1.5 (BertModel, hidden=512, 4层)
//
// 流程：tokenizer.json → WordPiece input_ids → BertModel 前向 → [CLS] L2归一化
// 缓存：~/.铃记忆体/embeddings.json（id → Vec<f32>），首次计算后持久化

use anyhow::{Context, Result};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tokenizers::Tokenizer;

// ── 全局单例（懒加载，失败时不 panic）─────────────────────────────────────

struct BertEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

static EMBEDDER: OnceLock<Option<BertEmbedder>> = OnceLock::new();

fn model_dir() -> PathBuf {
    // 优先：打包内置的资源目录（Tauri resource_dir），完整版模型随安装包分发
    if let Some(res) = try_resource_dir() {
        let p = res.join("models");
        if p.join("model.safetensors").exists() {
            return p;
        }
    }
    // 回退：用户目录（用户自放模型 / 轻量版后装插件包）
    // v1.0：先看本版本目录（~/.铃记忆体-v10/models），再只读复用主线目录，
    // 免得用户为同一份 bge 模型下两次。
    let v10 = crate::config::data_dir().join("models");
    if v10.join("model.safetensors").exists() {
        return v10;
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let legacy = PathBuf::from(format!("{home}/.铃记忆体/models"));
    if legacy.join("model.safetensors").exists() {
        return legacy;
    }
    v10
}

/// 获取 Tauri 应用资源目录（仅在 Tauri 运行时可用，纯测试环境返回 None）
fn try_resource_dir() -> Option<PathBuf> {
    #[cfg(not(test))]
    {
        // tauri 打包后资源（tauri.conf bundle.resources 映射目标）放在 exe 同级目录
        // 例如安装到 C:\Program Files\铃·记忆体\ 下，models/ 与 mem.exe 同级
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                if dir.join("models").join("model.safetensors").exists() {
                    return Some(dir.to_path_buf());
                }
            }
        }
    }
    None
}

fn cache_path() -> PathBuf {
    // v1.0：向量缓存跟随本版本数据目录（~/.铃记忆体-v10/embeddings.json）
    crate::config::data_dir().join("embeddings.json")
}

fn load_embedder() -> Result<BertEmbedder> {
    let dir = model_dir();
    let config_path = dir.join("config.json");
    let tokenizer_path = dir.join("tokenizer.json");
    let model_path = dir.join("model.safetensors");

    for p in [&config_path, &tokenizer_path, &model_path] {
        anyhow::ensure!(p.exists(), "模型文件不存在: {}", p.display());
    }

    let config_str = std::fs::read_to_string(&config_path)?;
    let config: BertConfig = serde_json::from_str(&config_str)
        .context("解析 config.json 失败")?;

    let tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| anyhow::anyhow!("加载 tokenizer.json 失败: {e}"))?;

    let device = Device::Cpu;
    let vb = unsafe {
        VarBuilder::from_mmaped_safetensors(&[&model_path], DType::F32, &device)?
    };
    let model = BertModel::load(vb, &config)?;

    Ok(BertEmbedder { model, tokenizer, device })
}

fn get_embedder() -> Option<&'static BertEmbedder> {
    EMBEDDER.get_or_init(|| {
        match load_embedder() {
            Ok(e) => Some(e),
            Err(err) => {
                log::warn!("[vector] 模型加载失败，将降级 bigram: {err:#}");
                None
            }
        }
    }).as_ref()
}

// ── 向量计算 ──────────────────────────────────────────────────────────────

/// 把文本编码为 L2 归一化的 [CLS] 向量（dim=512）
pub fn encode(text: &str) -> Option<Vec<f32>> {
    let emb = get_embedder()?;

    let encoding = emb.tokenizer
        .encode(text, true)
        .ok()?;

    let ids: Vec<u32> = encoding.get_ids().to_vec();
    let type_ids: Vec<u32> = encoding.get_type_ids().to_vec();
    let attention_mask: Vec<u32> = encoding.get_attention_mask().to_vec();

    let len = ids.len();
    if len == 0 {
        return None;
    }

    let make_tensor = |v: Vec<u32>| -> Result<Tensor> {
        let t = Tensor::from_vec(v, (1, len), &emb.device)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        t.to_dtype(DType::U32).map_err(|e| anyhow::anyhow!("{e}"))
    };

    let ids_t = make_tensor(ids).ok()?;
    let type_ids_t = make_tensor(type_ids).ok()?;
    let mask_t = make_tensor(attention_mask).ok()?;

    let output = emb.model
        .forward(&ids_t, &type_ids_t, Some(&mask_t))
        .ok()?;

    // [CLS] token = index 0，shape [1, seq_len, hidden]
    let cls = output.get(0).ok()?          // [seq_len, hidden]
                           .get(0).ok()?;  // [hidden]

    let vec: Vec<f32> = cls.to_vec1().ok()?;
    Some(l2_normalize(vec))
}

fn l2_normalize(mut v: Vec<f32>) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-9 {
        v.iter_mut().for_each(|x| *x /= norm);
    }
    v
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    // 向量已归一化，点积即余弦相似度
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

// ── Embedding 缓存 ────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default)]
struct EmbeddingCache {
    /// memory_id → L2归一化向量
    embeddings: HashMap<String, Vec<f32>>,
}

static CACHE: OnceLock<Mutex<EmbeddingCache>> = OnceLock::new();

fn get_cache() -> &'static Mutex<EmbeddingCache> {
    CACHE.get_or_init(|| {
        let cache = load_cache_from_disk().unwrap_or_default();
        Mutex::new(cache)
    })
}

fn load_cache_from_disk() -> Result<EmbeddingCache> {
    let path = cache_path();
    if !path.exists() {
        return Ok(EmbeddingCache::default());
    }
    let data = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}

fn save_cache_to_disk(cache: &EmbeddingCache) {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(cache) {
        let _ = std::fs::write(&path, json);
    }
}

/// 获取记忆的向量（缓存命中则直接返回，否则计算并写入缓存）
pub fn get_or_compute(id: &str, text: &str) -> Option<Vec<f32>> {
    {
        let cache = get_cache().lock().ok()?;
        if let Some(v) = cache.embeddings.get(id) {
            return Some(v.clone());
        }
    }

    let vec = encode(text)?;

    {
        let mut cache = get_cache().lock().ok()?;
        cache.embeddings.insert(id.to_string(), vec.clone());
        save_cache_to_disk(&cache);
    }

    Some(vec)
}

/// 使向量缓存失效（记忆被修改时调用）
pub fn invalidate(id: &str) {
    if let Ok(mut cache) = get_cache().lock() {
        if cache.embeddings.remove(id).is_some() {
            save_cache_to_disk(&cache);
        }
    }
}

/// 向量检索模型是否可用
pub fn is_available() -> bool {
    get_embedder().is_some()
}

// ── 公开检索接口 ──────────────────────────────────────────────────────────

use crate::types::Memory;

/// 用向量相似度对 memories 排序，返回余弦相似度 > threshold 的结果
/// 模型不可用时返回 None（调用方降级 bigram）
pub fn search_vector(
    memories: &[Memory],
    keyword: &str,
    threshold: f32,
) -> Option<Vec<Memory>> {
    if !is_available() {
        return None;
    }

    let query_vec = encode(keyword)?;

    let doc_text = |m: &Memory| -> String {
        match &m.summary {
            Some(s) => format!("{} {}", m.content, s),
            None => m.content.clone(),
        }
    };

    let mut scored: Vec<(f32, Memory)> = memories
        .iter()
        .filter_map(|m| {
            let text = doc_text(m);
            let vec = get_or_compute(&m.id, &text)?;
            let score = cosine_similarity(&query_vec, &vec);
            if score >= threshold {
                Some((score, m.clone()))
            } else {
                None
            }
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Some(scored.into_iter().map(|(_, m)| m).collect())
}

// ── 测试 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_similarity_semantic() {
        // 跳过：模型文件必须存在于 ~/.铃记忆体/models/
        if !is_available() {
            eprintln!("[skip] 向量模型未加载，跳过语义相似度测试");
            return;
        }

        let v1 = encode("天气好").expect("encode 失败");
        let v2 = encode("今天天气很好").expect("encode 失败");
        let v3 = encode("量子纠缠与黑洞热力学").expect("encode 失败");

        let sim_related = cosine_similarity(&v1, &v2);
        let sim_unrelated = cosine_similarity(&v1, &v3);

        eprintln!("相关句子相似度: {sim_related:.4}");
        eprintln!("无关句子相似度: {sim_unrelated:.4}");

        assert!(
            sim_related > sim_unrelated,
            "'天气好' vs '今天天气很好' ({sim_related:.4}) 应高于 vs 无关句子 ({sim_unrelated:.4})"
        );
    }

    #[test]
    fn l2_normalize_unit_length() {
        let v = l2_normalize(vec![3.0, 4.0]);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6, "归一化后模长应为1，实际: {norm}");
    }

    #[test]
    fn cosine_identical_vectors() {
        let v = vec![1.0_f32, 0.0, 0.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
    }
}
