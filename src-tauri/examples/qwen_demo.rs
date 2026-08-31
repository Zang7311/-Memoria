// SPIKE：纯 Rust(candle)加载 Qwen2.5 GGUF 做本地对话推理，不依赖 Ollama / llama.cpp。
//
// 方案：candle-transformers::models::quantized_qwen2::ModelWeights::from_gguf
// tokenizer：不需要外部 tokenizer.json —— 直接用 GGUF 内嵌的 BPE 词表(tokenizer.ggml.tokens
//            151936 项 + tokenizer.ggml.merges 151387 项)在内存里构建 tokenizers::Tokenizer，
//            完全离线，无网络、无额外文件。
//
// 运行：cargo run --release --example qwen_demo
//       cargo run --release --example qwen_demo -- 1.5b "你的问题"

use anyhow::{anyhow, Result};
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen2::ModelWeights;
use std::fs::File;
use std::io::Write;
// Vocab 是 tokenizers 公开的类型别名(= ahash::AHashMap<String, u32>)，
// 借它拿到正确的 map 类型，省得把 ahash 加进 Cargo.toml
use tokenizers::models::bpe::{Vocab, BPE};
use tokenizers::pre_tokenizers::byte_level::ByteLevel;
use tokenizers::{AddedToken, Tokenizer};

/// 从 GGUF 元数据中的内嵌词表构建 Qwen2 的 BPE 分词器。
///
/// Qwen2 用 GPT-2 风格 ByteLevel BPE(tokenizer.ggml.model == "gpt2")：
/// - tokens 数组下标即 token id
/// - merges 每项形如 "Ġ Ġ"，空格分隔左右两半
fn build_tokenizer(content: &gguf_file::Content) -> Result<Tokenizer> {
    let tokens = content
        .metadata
        .get("tokenizer.ggml.tokens")
        .ok_or_else(|| anyhow!("GGUF 缺少 tokenizer.ggml.tokens"))?
        .to_vec()?;
    let merges_raw = content
        .metadata
        .get("tokenizer.ggml.merges")
        .ok_or_else(|| anyhow!("GGUF 缺少 tokenizer.ggml.merges"))?
        .to_vec()?;

    let mut vocab = Vocab::with_capacity(tokens.len());
    for (id, tok) in tokens.iter().enumerate() {
        vocab.insert(tok.to_string()?.clone(), id as u32);
    }

    let mut merges: Vec<(String, String)> = Vec::with_capacity(merges_raw.len());
    for m in merges_raw.iter() {
        let s = m.to_string()?;
        // rsplit_once：merge 左半本身可能含空格转义符，从右侧切一次最稳
        let (a, b) = s
            .split_once(' ')
            .ok_or_else(|| anyhow!("merge 格式异常: {s}"))?;
        merges.push((a.to_string(), b.to_string()));
    }

    let bpe = BPE::builder()
        .vocab_and_merges(vocab, merges)
        .ignore_merges(true)
        .build()
        .map_err(|e| anyhow!("构建 BPE 失败: {e}"))?;

    let mut tk = Tokenizer::new(bpe);
    // ByteLevel：add_prefix_space=false(Qwen2 不加前导空格)，use_regex=true(GPT-2 切分正则)
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

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let size = args.get(1).map(|s| s.as_str()).unwrap_or("0.5b");
    let prompt = args.get(2).map(|s| s.as_str()).unwrap_or("你好");

    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME"))?;
    let file_name = match size {
        "1.5b" => "qwen2.5-1.5b-instruct-q4_k_m.gguf",
        _ => "qwen2.5-0.5b-instruct-q4_k_m.gguf",
    };
    let model_path = format!("{home}/.铃记忆体/models/qwen/{file_name}");

    println!("=== 铃·记忆体 SPIKE: candle + Qwen2.5 GGUF ===");
    println!("模型: {model_path}");

    let device = Device::Cpu;

    // ---- 1. 加载模型 ----
    let t_load = std::time::Instant::now();
    let mut file = File::open(&model_path)?;
    let content = gguf_file::Content::read(&mut file)?;
    let t_header = t_load.elapsed();

    let tokenizer = build_tokenizer(&content)?;
    let t_tok = t_load.elapsed();

    let mut model = ModelWeights::from_gguf(content, &mut file, &device)?;
    let load_ms = t_load.elapsed().as_millis();
    println!(
        "加载完成: 总 {load_ms}ms (GGUF头 {}ms / tokenizer {}ms / 权重 {}ms)",
        t_header.as_millis(),
        (t_tok - t_header).as_millis(),
        load_ms - t_tok.as_millis()
    );
    println!("词表大小: {}", tokenizer.get_vocab_size(true));

    // ---- 2. 构造 ChatML prompt(Qwen2.5 官方模板) ----
    let full_prompt = format!(
        "<|im_start|>system\nYou are Qwen, created by Alibaba Cloud. You are a helpful assistant.<|im_end|>\n<|im_start|>user\n{prompt}<|im_end|>\n<|im_start|>assistant\n"
    );
    let encoding = tokenizer
        .encode(full_prompt.as_str(), true)
        .map_err(|e| anyhow!("编码失败: {e}"))?;
    let prompt_tokens = encoding.get_ids().to_vec();
    println!("\n用户输入: {prompt}");
    println!("prompt token 数: {}", prompt_tokens.len());
    print!("\n模型回复: ");
    std::io::stdout().flush()?;

    // ---- 3. 采样循环 ----
    let mut logits_processor =
        LogitsProcessor::from_sampling(42, Sampling::TopP { p: 0.8, temperature: 0.7 });
    let eos_im_end = tokenizer.token_to_id("<|im_end|>").unwrap_or(151645);
    let eos_endoftext = tokenizer.token_to_id("<|endoftext|>").unwrap_or(151643);

    let max_new_tokens = 128usize;
    let mut all_tokens: Vec<u32> = Vec::new();

    // prefill：一次性喂入整个 prompt
    let t_prefill = std::time::Instant::now();
    let input = Tensor::new(prompt_tokens.as_slice(), &device)?.unsqueeze(0)?;
    let logits = model.forward(&input, 0)?.squeeze(0)?;
    let mut next_token = logits_processor.sample(&logits)?;
    let prefill_ms = t_prefill.elapsed().as_millis();
    all_tokens.push(next_token);

    // decode：逐 token 自回归
    let t_decode = std::time::Instant::now();
    let mut generated = 1usize;
    // 已打印的字符数(按 char 计，不按字节，避免切在中文 UTF-8 边界中间)
    let mut printed_chars = 0usize;
    for i in 0..max_new_tokens.saturating_sub(1) {
        if next_token == eos_im_end || next_token == eos_endoftext {
            break;
        }
        let input = Tensor::new(&[next_token], &device)?.unsqueeze(0)?;
        let logits = model
            .forward(&input, prompt_tokens.len() + i)?
            .squeeze(0)?;
        next_token = logits_processor.sample(&logits)?;
        all_tokens.push(next_token);
        generated += 1;

        // 流式输出：整体重解码后只打印新增字符。中文一个字常跨多个 BPE token，
        // 单 token 单独 decode 会出半个字，所以按 char 数做增量。
        if let Ok(text) = tokenizer.decode(&all_tokens, true) {
            let chars: Vec<char> = text.chars().collect();
            if chars.len() > printed_chars {
                let fresh: String = chars[printed_chars..].iter().collect();
                print!("{fresh}");
                std::io::stdout().flush()?;
                printed_chars = chars.len();
            }
        }
    }
    let decode_ms = t_decode.elapsed().as_millis();

    // 去掉尾部 EOS 再输出完整文本
    let clean: Vec<u32> = all_tokens
        .iter()
        .copied()
        .filter(|t| *t != eos_im_end && *t != eos_endoftext)
        .collect();
    let final_text = tokenizer
        .decode(&clean, true)
        .map_err(|e| anyhow!("解码失败: {e}"))?;

    println!("\n\n--- 完整回复 ---\n{final_text}");
    println!("\n--- 性能 ---");
    println!("加载耗时: {load_ms} ms");
    println!(
        "prefill: {prefill_ms} ms ({} prompt tokens, {:.1} tok/s)",
        prompt_tokens.len(),
        prompt_tokens.len() as f64 * 1000.0 / prefill_ms.max(1) as f64
    );
    println!(
        "decode: {decode_ms} ms ({generated} tokens, {:.1} tok/s)",
        (generated.saturating_sub(1)) as f64 * 1000.0 / decode_ms.max(1) as f64
    );
    Ok(())
}
