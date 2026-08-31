// 《铃·记忆体》内置本地大模型引擎（v1.0 离线智能版）
//
// 纯 Rust 方案：candle-transformers::models::quantized_qwen2::ModelWeights::from_gguf
// 直接加载 Qwen2.5 Instruct 的 GGUF（q4_k_m），不依赖 Ollama / llama.cpp / 任何外部进程。
// tokenizer 也不需要外部 tokenizer.json —— 用 GGUF 内嵌的 BPE 词表在内存里构建，完全离线。
//
// 来源：SPIKE examples/qwen_demo.rs（单轮 + 采样循环）+ examples/qwen_mem.rs（多轮 + RSS）。
//
// 关键设计：
// - 模型实例全局懒加载（LLM: Mutex<Option<LoadedLlm>>）。0.5B 加载约 4s、1.5B 更久，
//   不能每轮对话都付这个代价；两档模型共用一个槽位，切档时换出旧模型（省内存）。
// - 推理是纯 CPU 密集的同步循环，必须放在 tokio::task::spawn_blocking，
//   否则会把 Tauri 的异步事件循环钉死（前端事件、IPC 全部卡住）。
// - 逐 token 解码后通过现有 stream::sender 通道 emit（chat_chunk / chat_end），
//   与 api / script 两个引擎的前端契约完全一致。
use crate::engine;
use crate::error::AppError;
use crate::stream::sender;
use crate::types::{Memory, Setting};
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen2::ModelWeights;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::AppHandle;
use tokenizers::models::bpe::{Vocab, BPE};
use tokenizers::pre_tokenizers::byte_level::ByteLevel;
use tokenizers::{AddedToken, Tokenizer};

/// 内置模型档位：0.5B（轻快）/ 1.5B（更聪明但更慢更吃内存）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSize {
    /// qwen2.5-0.5b-instruct-q4_k_m.gguf（约 400MB，RSS 约 700MB）
    B05,
    /// qwen2.5-1.5b-instruct-q4_k_m.gguf（约 1.1GB，RSS 约 1.6GB）
    B15,
}

impl ModelSize {
    /// GGUF 文件名（与 SPIKE 验证过的文件一致）
    pub fn file_name(self) -> &'static str {
        match self {
            ModelSize::B05 => "qwen2.5-0.5b-instruct-q4_k_m.gguf",
            ModelSize::B15 => "qwen2.5-1.5b-instruct-q4_k_m.gguf",
        }
    }

    /// 给日志/前端看的人话名
    pub fn label(self) -> &'static str {
        match self {
            ModelSize::B05 => "内置 0.5B",
            ModelSize::B15 => "内置 1.5B",
        }
    }

    /// 前端/配置里的字符串标识（"0.5b" / "1.5b"）
    pub fn as_str(self) -> &'static str {
        match self {
            ModelSize::B05 => "0.5b",
            ModelSize::B15 => "1.5b",
        }
    }

    /// 宽松解析："0.5b" / "05b" / "local_0b" 等都能落到 B05；其余含 "1.5"/"1b" 落到 B15
    pub fn parse(s: &str) -> Self {
        let t = s.trim().to_ascii_lowercase();
        if t.contains("1.5") || t.contains("1_5") || t.contains("1b") {
            ModelSize::B15
        } else {
            ModelSize::B05
        }
    }
}

// ==================== 模型文件定位 ====================

fn home_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
}

/// 模型搜索目录（按优先级）：
/// ① 打包内置资源（exe 同级 models/qwen，完整版随安装包分发）
/// ② 本版本独立数据目录（~/.铃记忆体-v10/models/qwen）
/// ③ 主线 / SPIKE 旧目录（~/.铃记忆体/models/qwen）—— 只读复用，不写入，避免用户重复下 1.5GB
pub fn model_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            dirs.push(d.join("models").join("qwen"));
        }
    }
    dirs.push(crate::config::data_dir().join("models").join("qwen"));
    dirs.push(home_dir().join(".铃记忆体").join("models").join("qwen"));
    dirs
}

/// 查找某一档模型的 GGUF 文件；找不到返回 None（调用方据此降级到离线文库）
pub fn find_model(size: ModelSize) -> Option<PathBuf> {
    let name = size.file_name();
    model_dirs()
        .into_iter()
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// 模型文件缺失时的引导文案（前端设置页/降级提示复用）
pub fn missing_model_hint(size: ModelSize) -> String {
    format!(
        "未找到{}模型文件 {}，请放入 {}",
        size.label(),
        size.file_name(),
        crate::config::data_dir()
            .join("models")
            .join("qwen")
            .display()
    )
}

// ==================== 全局懒加载单例 ====================

struct LoadedLlm {
    size: ModelSize,
    model: ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
    /// ChatML 结束符（<|im_end|> / <|endoftext|>），从词表取，取不到用官方 id 兜底
    eos: [u32; 2],
}

/// 已加载的模型（两档共用一个槽位：切档时换出旧模型，避免同时驻留 2.3GB）
static LLM: Mutex<Option<LoadedLlm>> = Mutex::new(None);

/// 从 GGUF 元数据中的内嵌词表构建 Qwen2 的 BPE 分词器。
///
/// Qwen2 用 GPT-2 风格 ByteLevel BPE（tokenizer.ggml.model == "gpt2"）：
/// - tokens 数组下标即 token id
/// - merges 每项形如 "Ġ Ġ"，空格分隔左右两半
fn build_tokenizer(content: &gguf_file::Content) -> Result<Tokenizer, AppError> {
    let err = |m: String| AppError::ModelError(m);

    let tokens = content
        .metadata
        .get("tokenizer.ggml.tokens")
        .ok_or_else(|| err("GGUF 缺少 tokenizer.ggml.tokens".into()))?
        .to_vec()
        .map_err(|e| err(format!("读取词表失败：{e}")))?;
    let merges_raw = content
        .metadata
        .get("tokenizer.ggml.merges")
        .ok_or_else(|| err("GGUF 缺少 tokenizer.ggml.merges".into()))?
        .to_vec()
        .map_err(|e| err(format!("读取 merges 失败：{e}")))?;

    let mut vocab = Vocab::with_capacity(tokens.len());
    for (id, tok) in tokens.iter().enumerate() {
        let s = tok
            .to_string()
            .map_err(|e| err(format!("词表项非字符串：{e}")))?;
        vocab.insert(s.clone(), id as u32);
    }

    let mut merges: Vec<(String, String)> = Vec::with_capacity(merges_raw.len());
    for m in merges_raw.iter() {
        let s = m
            .to_string()
            .map_err(|e| err(format!("merges 项非字符串：{e}")))?;
        let (a, b) = s
            .split_once(' ')
            .ok_or_else(|| err(format!("merge 格式异常：{s}")))?;
        merges.push((a.to_string(), b.to_string()));
    }

    let bpe = BPE::builder()
        .vocab_and_merges(vocab, merges)
        .ignore_merges(true)
        .build()
        .map_err(|e| err(format!("构建 BPE 失败：{e}")))?;

    let mut tk = Tokenizer::new(bpe);
    // ByteLevel：add_prefix_space=false（Qwen2 不加前导空格），use_regex=true（GPT-2 切分正则）
    tk.with_pre_tokenizer(Some(ByteLevel::new(false, true, true)));
    tk.with_decoder(Some(ByteLevel::new(false, true, true)));
    // ChatML 控制 token 必须注册为 special，否则会被 BPE 拆碎
    let specials: Vec<AddedToken> = ["<|im_start|>", "<|im_end|>", "<|endoftext|>"]
        .iter()
        .map(|s| AddedToken::from(s.to_string(), true))
        .collect();
    tk.add_special_tokens(&specials);

    Ok(tk)
}

/// 真正加载一档模型（约 0.5B 4s / 1.5B 10s+，只在首次或切档时付这个代价）
fn load(size: ModelSize) -> Result<LoadedLlm, AppError> {
    let path = find_model(size).ok_or_else(|| AppError::ModelError(missing_model_hint(size)))?;
    let t0 = std::time::Instant::now();

    let mut file = File::open(&path)
        .map_err(|e| AppError::ModelError(format!("打开模型失败 {}：{e}", path.display())))?;
    let content = gguf_file::Content::read(&mut file)
        .map_err(|e| AppError::ModelError(format!("解析 GGUF 失败：{e}")))?;

    let tokenizer = build_tokenizer(&content)?;
    let device = Device::Cpu;
    let model = ModelWeights::from_gguf(content, &mut file, &device)
        .map_err(|e| AppError::ModelError(format!("加载模型权重失败：{e}")))?;

    let eos = [
        tokenizer.token_to_id("<|im_end|>").unwrap_or(151645),
        tokenizer.token_to_id("<|endoftext|>").unwrap_or(151643),
    ];
    log::info!(
        "[local_llm] {} 加载完成，耗时 {}ms，词表 {}，路径 {}",
        size.label(),
        t0.elapsed().as_millis(),
        tokenizer.get_vocab_size(true),
        path.display()
    );

    Ok(LoadedLlm {
        size,
        model,
        tokenizer,
        device,
        eos,
    })
}

/// 取到已加载的模型（必要时懒加载/换档）并执行推理闭包。
/// 持锁期间是纯同步计算，调用方必须已在 spawn_blocking 里。
fn with_llm<T>(
    size: ModelSize,
    f: impl FnOnce(&mut LoadedLlm) -> Result<T, AppError>,
) -> Result<T, AppError> {
    // 上一次推理 panic 导致锁中毒时，模型本身仍然可用，恢复内部值继续跑
    let mut guard = LLM.lock().unwrap_or_else(|e| e.into_inner());
    let need_load = guard.as_ref().map(|l| l.size != size).unwrap_or(true);
    if need_load {
        if let Some(old) = guard.take() {
            log::info!("[local_llm] 换档：{} → {}", old.size.label(), size.label());
            drop(old); // 先释放旧模型内存，再加载新档，避免峰值双份驻留
        }
        *guard = Some(load(size)?);
    }
    let llm = guard
        .as_mut()
        .ok_or_else(|| AppError::ModelError("模型未加载".into()))?;
    f(llm)
}

// ==================== prompt 构造 ====================

/// 上下文消息条数上限（0.5B 上下文越长越慢也越容易跑偏，给得比 API 保守）
fn max_context_messages(size: ModelSize, depth: u8) -> usize {
    let base = match size {
        ModelSize::B05 => 6,
        ModelSize::B15 => 10,
    };
    if depth >= 3 {
        base + 4
    } else {
        base
    }
}

/// 生成上限：depth 越高允许说得越多
fn max_new_tokens(depth: u8) -> usize {
    match depth {
        1 => 96,
        3 => 320,
        4 => 512,
        _ => 192,
    }
}

/// 单条上下文消息的字符上限（防一条超长记忆吃满 prompt）
const MAX_MSG_CHARS: usize = 400;

fn truncate_chars(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    let mut out: String = chars[..max].iter().collect();
    out.push('…');
    out
}

/// 构造 Qwen2.5 官方 ChatML prompt：system 人格 + 历史上下文 + 当前输入
fn build_prompt(input: &str, context: &[Memory], setting: &Setting, size: ModelSize, depth: u8) -> String {
    let self_name = setting.self_name.as_deref().unwrap_or("铃");
    let user_name = setting.user_name.as_deref().unwrap_or("主人");
    let sys = format!(
        "{}\n【身份约定】你的名字叫「{self_name}」，用户是你的「{user_name}」。所有回复默认以这个称呼关系进行；\n\
         【回复风格】语气自然口语化，像真人聊天，一次只说两三句，不要长篇大论、不要重复自己；\n\
         【记忆】上文里的历史消息是你们的过往对话，请自然地延续话题。",
        engine::api::persona_system_prompt(&setting.persona)
    );

    let mut p = format!("<|im_start|>system\n{sys}<|im_end|>\n");

    // 只取最近 N 条，且跳过空消息；role 归一到 user/assistant（Qwen 只认这两个）
    let keep = max_context_messages(size, depth);
    let start = context.len().saturating_sub(keep);
    for m in &context[start..] {
        let content = m.content.trim();
        if content.is_empty() {
            continue;
        }
        let role = if m.role == "assistant" { "assistant" } else { "user" };
        p.push_str(&format!(
            "<|im_start|>{role}\n{}<|im_end|>\n",
            truncate_chars(content, MAX_MSG_CHARS)
        ));
    }

    p.push_str(&format!(
        "<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
        input.trim()
    ));
    p
}

// ==================== 推理主循环（同步，跑在 spawn_blocking 里）====================

/// 重复惩罚窗口：小模型容易复读，对最近 64 个 token 施加惩罚
const REPEAT_LAST_N: usize = 64;
const REPEAT_PENALTY: f32 = 1.1;

fn generate_blocking(
    app: &AppHandle,
    size: ModelSize,
    prompt: &str,
    depth: u8,
) -> Result<String, AppError> {
    let (temperature, top_p, _) = engine::apply_depth(depth);
    let max_new = max_new_tokens(depth);

    with_llm(size, |llm| {
        // 每轮从零重放整个对话：KV cache 必须清，否则 index_pos 会和缓存长度错位
        llm.model.clear_kv_cache();

        let ids = llm
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| AppError::ModelError(format!("编码 prompt 失败：{e}")))?
            .get_ids()
            .to_vec();
        if ids.is_empty() {
            return Err(AppError::ModelError("prompt 编码为空".into()));
        }
        log::info!(
            "[local_llm] {} 开始推理：prompt {} tokens，上限 {} tokens",
            size.label(),
            ids.len(),
            max_new
        );

        let mut lp = LogitsProcessor::from_sampling(
            rand_seed(),
            Sampling::TopP {
                p: top_p as f64,
                temperature: temperature as f64,
            },
        );

        // prefill：一次性喂入整个 prompt
        let t_prefill = std::time::Instant::now();
        let input = Tensor::new(ids.as_slice(), &llm.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(cd_err)?;
        let logits = llm
            .model
            .forward(&input, 0)
            .and_then(|t| t.squeeze(0))
            .map_err(cd_err)?;
        let mut next = lp.sample(&logits).map_err(cd_err)?;
        let prefill_ms = t_prefill.elapsed().as_millis();

        let mut out: Vec<u32> = vec![next];
        // 已 emit 的字符数（按 char 计而非字节：中文一个字常跨多个 BPE token，
        // 单 token 单独 decode 会吐出半个字，所以整体重解码后做字符级增量）
        let mut emitted_chars = 0usize;
        let mut full = String::new();
        let t_decode = std::time::Instant::now();
        let mut generated = 1usize;

        for i in 0..max_new.saturating_sub(1) {
            if llm.eos.contains(&next) {
                break;
            }
            let input = Tensor::new(&[next], &llm.device)
                .and_then(|t| t.unsqueeze(0))
                .map_err(cd_err)?;
            let mut logits = llm
                .model
                .forward(&input, ids.len() + i)
                .and_then(|t| t.squeeze(0))
                .map_err(cd_err)?;
            // 复读抑制（小模型必需）
            if REPEAT_PENALTY != 1.0 {
                let from = out.len().saturating_sub(REPEAT_LAST_N);
                logits = candle_transformers::utils::apply_repeat_penalty(
                    &logits,
                    REPEAT_PENALTY,
                    &out[from..],
                )
                .map_err(cd_err)?;
            }
            next = lp.sample(&logits).map_err(cd_err)?;
            out.push(next);
            generated += 1;

            // 流式：整体重解码后只 emit 新增字符
            if let Ok(text) = llm.tokenizer.decode(&out, true) {
                let chars: Vec<char> = text.chars().collect();
                if chars.len() > emitted_chars {
                    let fresh: String = chars[emitted_chars..].iter().collect();
                    emitted_chars = chars.len();
                    full = text;
                    sender::send_chunk(app, &fresh)?;
                }
            }
        }

        // 去掉尾部控制 token 再取完整文本（emit 时 skip_special_tokens 已过滤，这里对齐入库文本）
        let clean: Vec<u32> = out.into_iter().filter(|t| !llm.eos.contains(t)).collect();
        if let Ok(text) = llm.tokenizer.decode(&clean, true) {
            full = text;
        }

        let decode_ms = t_decode.elapsed().as_millis().max(1);
        log::info!(
            "[local_llm] {} 完成：prefill {prefill_ms}ms，decode {generated} tokens / {decode_ms}ms（{:.1} tok/s）",
            size.label(),
            generated as f64 * 1000.0 / decode_ms as f64
        );

        Ok(full.trim().to_string())
    })
}

/// candle 错误 → AppError
fn cd_err(e: candle_core::Error) -> AppError {
    AppError::ModelError(format!("推理失败：{e}"))
}

/// 采样种子：用时间戳，避免同一句话每次都得到一模一样的回复
fn rand_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(42)
}

// ==================== 对外接口 ====================

/// 运行内置本地大模型：流式推送并返回完整回复文本（供上层写入记忆）。
///
/// - `model_size`：0.5b / 1.5b 两档
/// - 推理走 spawn_blocking，不阻塞 Tauri 事件循环
/// - 逐 token 通过 stream::sender 的 chat_chunk / chat_end 事件 emit
///
/// 说明：任务书给的签名是 `run_local_llm(app, input, depth, model_size)`；这里额外收
/// `context`（会话/记忆上下文）与 `setting`（人格 + 自称/称呼），否则本地模式会丢掉
/// "记忆体" 的核心能力和人格设定，回复质量明显不如 API 模式。
pub async fn run_local_llm(
    app: &AppHandle,
    input: &str,
    context: &[Memory],
    depth: u8,
    model_size: ModelSize,
    setting: &Setting,
) -> Result<String, AppError> {
    let prompt = build_prompt(input, context, setting, model_size, depth);
    let app_cloned = app.clone();

    // CPU 密集的自回归循环放到阻塞线程池，异步 runtime 不被钉死
    let reply = tokio::task::spawn_blocking(move || {
        generate_blocking(&app_cloned, model_size, &prompt, depth)
    })
    .await
    .map_err(|e| AppError::InternalError(format!("本地推理任务异常：{e}")))??;

    if reply.is_empty() {
        return Err(AppError::ModelError("本地模型无输出".into()));
    }
    sender::send_end(app)?;
    Ok(reply)
}

/// 生成一条 assistant 记忆
pub fn to_memory(id: &str, reply: &str) -> Memory {
    Memory {
        id: id.to_string(),
        role: "assistant".to_string(),
        content: reply.to_string(),
        timestamp: crate::utils::now_str(),
        tags: None,
        summary: None,
        category: None,
        use_count: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_model_size() {
        assert_eq!(ModelSize::parse("0.5b"), ModelSize::B05);
        assert_eq!(ModelSize::parse("local_0b"), ModelSize::B05);
        assert_eq!(ModelSize::parse(""), ModelSize::B05);
        assert_eq!(ModelSize::parse("1.5b"), ModelSize::B15);
        assert_eq!(ModelSize::parse("LOCAL_1B"), ModelSize::B15);
    }

    #[test]
    fn file_names_match_spike() {
        assert_eq!(
            ModelSize::B05.file_name(),
            "qwen2.5-0.5b-instruct-q4_k_m.gguf"
        );
        assert_eq!(
            ModelSize::B15.file_name(),
            "qwen2.5-1.5b-instruct-q4_k_m.gguf"
        );
    }

    #[test]
    fn model_dirs_cover_legacy_and_v10() {
        let dirs = model_dirs();
        let joined: Vec<String> = dirs.iter().map(|d| d.display().to_string()).collect();
        // 独立数据目录 + 主线旧目录都要在搜索路径里（避免用户重复下载 1.5GB 模型）
        assert!(joined.iter().any(|s| s.contains(".铃记忆体-v10")), "{joined:?}");
        assert!(
            joined
                .iter()
                .any(|s| s.contains(".铃记忆体") && !s.contains("-v10")),
            "{joined:?}"
        );
        assert!(joined.iter().all(|s| s.ends_with("qwen")), "{joined:?}");
    }

    #[test]
    fn prompt_uses_chatml_and_keeps_recent_context() {
        let setting = Setting::default();
        let mk = |i: usize, role: &str| Memory {
            id: format!("m{i}"),
            role: role.to_string(),
            content: format!("历史消息{i}"),
            timestamp: "2026-09-01 00:00:00".to_string(),
            tags: None,
            summary: None,
            category: None,
            use_count: 0,
        };
        let ctx: Vec<Memory> = (0..20)
            .map(|i| mk(i, if i % 2 == 0 { "user" } else { "assistant" }))
            .collect();
        let p = build_prompt("你好", &ctx, &setting, ModelSize::B05, 2);

        assert!(p.starts_with("<|im_start|>system\n"));
        assert!(p.ends_with("<|im_start|>assistant\n"));
        assert!(p.contains("<|im_start|>user\n你好<|im_end|>"));
        // depth=2 + 0.5B → 只保留最近 6 条历史（context 末尾的 14..19）
        assert!(p.contains("历史消息19"));
        assert!(p.contains("历史消息14"));
        assert!(!p.contains("历史消息13"));
        assert!(!p.contains("历史消息0<|im_end|>"));
        // 标记数 = system 1 + 历史 6 + 当前输入 1 + 结尾的 assistant 生成引导 1
        assert_eq!(p.matches("<|im_start|>").count(), 1 + 6 + 1 + 1);
    }

    #[test]
    fn prompt_truncates_overlong_message() {
        let setting = Setting::default();
        let long = "啊".repeat(MAX_MSG_CHARS + 200);
        let ctx = vec![Memory {
            id: "m".into(),
            role: "user".into(),
            content: long,
            timestamp: "2026-09-01 00:00:00".into(),
            tags: None,
            summary: None,
            category: None,
            use_count: 0,
        }];
        let p = build_prompt("嗨", &ctx, &setting, ModelSize::B05, 2);
        assert!(p.contains('…'));
        assert!(p.matches('啊').count() <= MAX_MSG_CHARS);
    }

    #[test]
    fn max_new_tokens_scales_with_depth() {
        assert!(max_new_tokens(1) < max_new_tokens(2));
        assert!(max_new_tokens(2) < max_new_tokens(3));
        assert!(max_new_tokens(3) < max_new_tokens(4));
    }
}
