// SPIKE 补充：测量常驻内存占用 + 验证多轮对话(KV cache 跨轮复用/清理)。
// 运行：cargo run --release --example qwen_mem -- 0.5b
use anyhow::{anyhow, Result};
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen2::ModelWeights;
use std::fs::File;
use sysinfo::{Pid, ProcessRefreshKind, System};
use tokenizers::models::bpe::{Vocab, BPE};
use tokenizers::pre_tokenizers::byte_level::ByteLevel;
use tokenizers::{AddedToken, Tokenizer};

fn rss_mb(sys: &mut System, pid: Pid) -> f64 {
    sys.refresh_process_specifics(pid, ProcessRefreshKind::new().with_memory());
    sys.process(pid).map(|p| p.memory() as f64 / 1024.0 / 1024.0).unwrap_or(0.0)
}

fn build_tokenizer(content: &gguf_file::Content) -> Result<Tokenizer> {
    let tokens = content.metadata.get("tokenizer.ggml.tokens")
        .ok_or_else(|| anyhow!("缺 tokens"))?.to_vec()?;
    let merges_raw = content.metadata.get("tokenizer.ggml.merges")
        .ok_or_else(|| anyhow!("缺 merges"))?.to_vec()?;
    let mut vocab = Vocab::with_capacity(tokens.len());
    for (id, tok) in tokens.iter().enumerate() {
        vocab.insert(tok.to_string()?.clone(), id as u32);
    }
    let mut merges = Vec::with_capacity(merges_raw.len());
    for m in merges_raw.iter() {
        let s = m.to_string()?;
        let (a, b) = s.split_once(' ').ok_or_else(|| anyhow!("merge 异常"))?;
        merges.push((a.to_string(), b.to_string()));
    }
    let bpe = BPE::builder().vocab_and_merges(vocab, merges).ignore_merges(true)
        .build().map_err(|e| anyhow!("{e}"))?;
    let mut tk = Tokenizer::new(bpe);
    tk.with_pre_tokenizer(Some(ByteLevel::new(false, true, true)));
    tk.with_decoder(Some(ByteLevel::new(false, true, true)));
    tk.add_special_tokens(&["<|im_start|>", "<|im_end|>", "<|endoftext|>"]
        .iter().map(|s| AddedToken::from(s.to_string(), true)).collect::<Vec<_>>());
    Ok(tk)
}

/// 单轮生成。history 是已累积的完整 ChatML 文本，返回 (回复, 新 history, tok/s)
fn generate(
    model: &mut ModelWeights,
    tk: &Tokenizer,
    device: &Device,
    history: &str,
    user: &str,
    max_new: usize,
) -> Result<(String, String, f64)> {
    let prompt = format!("{history}<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n");
    // 每轮从零重放整个对话：KV cache 必须清，否则 index_pos 会和缓存长度错位
    model.clear_kv_cache();

    let ids = tk.encode(prompt.as_str(), true).map_err(|e| anyhow!("{e}"))?.get_ids().to_vec();
    let mut lp = LogitsProcessor::from_sampling(42, Sampling::TopP { p: 0.8, temperature: 0.7 });
    let eos_end = tk.token_to_id("<|im_end|>").unwrap_or(151645);
    let eos_eot = tk.token_to_id("<|endoftext|>").unwrap_or(151643);

    let input = Tensor::new(ids.as_slice(), device)?.unsqueeze(0)?;
    let logits = model.forward(&input, 0)?.squeeze(0)?;
    let mut next = lp.sample(&logits)?;
    let mut out = vec![next];

    let t = std::time::Instant::now();
    let mut n = 0usize;
    for i in 0..max_new.saturating_sub(1) {
        if next == eos_end || next == eos_eot { break; }
        let input = Tensor::new(&[next], device)?.unsqueeze(0)?;
        let logits = model.forward(&input, ids.len() + i)?.squeeze(0)?;
        next = lp.sample(&logits)?;
        out.push(next);
        n += 1;
    }
    let tps = n as f64 * 1000.0 / t.elapsed().as_millis().max(1) as f64;

    let clean: Vec<u32> = out.into_iter().filter(|x| *x != eos_end && *x != eos_eot).collect();
    let reply = tk.decode(&clean, true).map_err(|e| anyhow!("{e}"))?;
    let new_history = format!("{prompt}{reply}<|im_end|>\n");
    Ok((reply, new_history, tps))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let size = args.get(1).map(|s| s.as_str()).unwrap_or("0.5b");
    let file_name = if size == "1.5b" {
        "qwen2.5-1.5b-instruct-q4_k_m.gguf"
    } else {
        "qwen2.5-0.5b-instruct-q4_k_m.gguf"
    };
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME"))?;
    let path = format!("{home}/.铃记忆体/models/qwen/{file_name}");

    let mut sys = System::new();
    let pid = Pid::from_u32(std::process::id());
    println!("=== 内存 & 多轮对话验证 ({size}) ===");
    println!("启动时 RSS: {:.1} MB", rss_mb(&mut sys, pid));

    let device = Device::Cpu;
    let t0 = std::time::Instant::now();
    let mut file = File::open(&path)?;
    let content = gguf_file::Content::read(&mut file)?;
    let tk = build_tokenizer(&content)?;
    println!("tokenizer 就绪后 RSS: {:.1} MB", rss_mb(&mut sys, pid));
    let mut model = ModelWeights::from_gguf(content, &mut file, &device)?;
    println!("模型加载后 RSS: {:.1} MB (加载 {} ms)", rss_mb(&mut sys, pid), t0.elapsed().as_millis());

    let turns = ["我叫小铃，请记住我的名字。", "我刚才说我叫什么名字？"];
    let mut history = String::from(
        "<|im_start|>system\nYou are Qwen, created by Alibaba Cloud. You are a helpful assistant.<|im_end|>\n",
    );
    for (i, user) in turns.iter().enumerate() {
        let (reply, new_hist, tps) = generate(&mut model, &tk, &device, &history, user, 96)?;
        history = new_hist;
        println!("\n[第{}轮] 用户: {user}", i + 1);
        println!("[第{}轮] 助手: {reply}", i + 1);
        println!("[第{}轮] {:.1} tok/s, RSS {:.1} MB", i + 1, tps, rss_mb(&mut sys, pid));
    }

    println!("\n峰值后 RSS: {:.1} MB", rss_mb(&mut sys, pid));
    Ok(())
}
