// 《铃·记忆体》任务完成自评（agent/evaluator.rs）
//
// 依据 Anthropic《Building Effective Agents》的 evaluator-optimizer 模式原文：
//   "In the evaluator-optimizer workflow, one LLM call generates a response
//    while another provides evaluation and feedback in a loop."
//
// 目的：只对「真的动了手」（调用过工具）的任务做**一次独立核对**，减少「假装完成」——
//       汇报写得漂漂亮亮，但工具其实报错了、或者压根什么都没做成。
//
// 成本：每个「调用过工具」的任务多 1 次模型调用。纯聊天不触发，零额外开销。
// 容错：任何失败（网络 / 非 200 / 解析不出来）一律返回 None，**绝不阻断主流程**。

use std::time::Duration;

use serde_json::{json, Value};

/// 交给验收员看的「单条动作」字符上限（防止把上下文撑爆）
const MAX_ACTION_CHARS: usize = 300;

/// 验收结论
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// 是否真的完成了
    pub done: bool,
    /// 一句话原因（未完成时会被塞回去让 Agent 继续处理）
    pub reason: String,
}

/// 交给独立一次模型调用验收：这个任务真的完成了吗？
///
/// # 返回
/// - `Some(Verdict)`：成功拿到结论
/// - `None`：没调用过工具、或任何环节失败 —— 调用方按「不干预」处理
pub async fn evaluate(
    client: &reqwest::Client,
    url: &str,
    key: &str,
    model: &str,
    task: &str,
    actions: &[String],
    final_reply: &str,
) -> Option<Verdict> {
    // 没动过手就没什么可验收的（纯聊天、纯回答）
    if actions.is_empty() {
        return None;
    }

    let actions_text = actions
        .iter()
        .map(|a| {
            if a.chars().count() > MAX_ACTION_CHARS {
                let cut: String = a.chars().take(MAX_ACTION_CHARS).collect();
                format!("{cut}…（已截断）")
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "你是任务验收员。请严格核对下面这个任务「是否真的完成了」。\n\
         \n\
         【用户任务】\n{task}\n\
         \n\
         【实际执行的步骤（工具名 → 返回结果）】\n{actions_text}\n\
         \n\
         【助手最后的汇报】\n{final_reply}\n\
         \n\
         判断标准：\n\
         1. 只有工具返回里能找到「确实成功」的证据，才算完成；\n\
         2. 工具报错、返回空、提示失败、权限不足 —— 一律算未完成；\n\
         3. 只说不做（例如「我这就帮你打开」但没有任何工具成功返回）算未完成；\n\
         4. 任务本身只是提问 / 聊天，不涉及动手操作 —— 算完成；\n\
         5. 拿不准时倾向判「未完成」，让助手再确认一次，别放过含糊的情况。\n\
         \n\
         只输出 JSON，不要解释、不要代码块标记：\n\
         {{\"done\": true 或 false, \"reason\": \"一句话原因\"}}"
    );

    let body = json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        // 验收要的是稳定判断，不要发挥
        "temperature": 0.0,
        "max_tokens": 200,
    });

    let resp = client
        .post(url)
        .bearer_auth(key)
        .json(&body)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let v: Value = resp.json().await.ok()?;
    let raw = v
        .get("choices")?
        .get(0)?
        .get("message")?
        .get("content")?
        .as_str()?;

    parse_verdict(raw)
}

/// 容错解析验收结论：直接 parse → 剥代码块 → 抠首尾 `{}`。
/// 三级都失败就返回 None（调用方视作「不干预」）。
fn parse_verdict(raw: &str) -> Option<Verdict> {
    let t = raw.trim();
    for cand in [t.to_string(), strip_fence(t), extract_braces(t)] {
        if let Ok(v) = serde_json::from_str::<Value>(&cand) {
            if let Some(done) = v.get("done").and_then(|d| d.as_bool()) {
                return Some(Verdict {
                    done,
                    reason: v
                        .get("reason")
                        .and_then(|r| r.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string(),
                });
            }
        }
    }
    None
}

/// 剥掉 ```json ... ``` 包裹
fn strip_fence(s: &str) -> String {
    s.trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string()
}

/// 抠出第一个 `{` 到最后一个 `}` 之间的内容（模型爱在 JSON 前后唠叨）
fn extract_braces(s: &str) -> String {
    match (s.find('{'), s.rfind('}')) {
        (Some(a), Some(b)) if b > a => s[a..=b].to_string(),
        _ => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 解析_直接输出_json() {
        let v = parse_verdict(r#"{"done": true, "reason": "文件已写入"}"#).unwrap();
        assert!(v.done);
        assert_eq!(v.reason, "文件已写入");
    }

    #[test]
    fn 解析_带代码块标记() {
        let v = parse_verdict("```json\n{\"done\": false, \"reason\": \"工具报错了\"}\n```").unwrap();
        assert!(!v.done);
        assert_eq!(v.reason, "工具报错了");
    }

    #[test]
    fn 解析_前后有废话() {
        let raw = "好的，我来核对一下：{\"done\": false, \"reason\": \"没有证据\"} 以上。";
        let v = parse_verdict(raw).unwrap();
        assert!(!v.done);
        assert_eq!(v.reason, "没有证据");
    }

    #[test]
    fn 解析_缺_reason_也能用() {
        let v = parse_verdict(r#"{"done": true}"#).unwrap();
        assert!(v.done);
        assert_eq!(v.reason, "");
    }

    #[test]
    fn 解析_缺少_done_字段视为失败() {
        // 没有 done 就说明模型没按格式来，宁可当解析失败
        assert!(parse_verdict(r#"{"reason": "忘了写 done"}"#).is_none());
    }

    #[test]
    fn 解析_完全不是_json_返回_none() {
        assert!(parse_verdict("我觉得应该完成了吧").is_none());
        assert!(parse_verdict("").is_none());
    }

    #[test]
    fn 解析_done_不是布尔值视为失败() {
        // "true" 字符串不算 —— 防止模型输出不规范时误判
        assert!(parse_verdict(r#"{"done": "true"}"#).is_none());
    }

    #[test]
    fn 短动作摘录原样保留() {
        let a = "打开QQ".to_string();
        assert!(a.chars().count() < MAX_ACTION_CHARS);
    }
}
