// 《铃·记忆体》AI-10 多步任务规划器（planner.rs）
//
// 职责：用一次 LLM 调用把用户任务拆成有序子任务列表。
// 容错策略：直接 parse → 剥离代码块再 parse → 找首尾 [] 再 parse → 回退单步。
// 永不 panic，永不返回 Result（失败即回退）。

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// 单条子任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub step: usize,
    pub goal: String,
    pub tool_hint: Option<String>,
}

/// 把用户任务用一次 LLM 调用拆解为有序子任务列表。
/// 失败时回退为包含原始任务的单元素 Vec，永不 panic，永不返回 Err。
pub async fn plan_task(
    client: &reqwest::Client,
    url: &str,
    key: &str,
    model: &str,
    task: &str,
    tool_names: &[String],
    experience: Option<&str>,
) -> Vec<SubTask> {
    let fallback = vec![SubTask {
        step: 1,
        goal: task.to_string(),
        tool_hint: None,
    }];

    let tools_hint = if tool_names.is_empty() {
        "（暂无可用工具）".to_string()
    } else {
        tool_names.join("、")
    };

    // 历史成功经验：有就带进提示，让规划少走弯路（首跑时为空）
    let exp_block = match experience.map(str::trim) {
        Some(e) if !e.is_empty() => format!("\n{e}\n"),
        _ => String::new(),
    };

    let prompt = format!(
        "你是任务规划助手。请把下面的用户任务拆解为有序的子步骤，输出纯 JSON 数组，\
         格式：[{{\"step\":1,\"goal\":\"...\",\"tool_hint\":\"工具名或null\"}}]\n\
         \n\
         可用工具：{tools_hint}\n\
         {exp_block}\
         \n\
         用户任务：{task}\n\
         \n\
         要求：\
         1. 只输出 JSON 数组，不要任何额外解释；\
         2. tool_hint 填最可能用到的一个工具名，没有合适工具则填 null；\
         3. 步骤数控制在 1-6 步之间；\
         4. 如果任务本身就是单步，就只输出一个元素的数组；\
         5. 如果上面给了「参考经验」，优先沿用它那条成功路线来拆步骤。"
    );

    let body = json!({
        "model": model,
        "messages": [
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.2,
        "top_p": 0.9,
        "max_tokens": 512,
    });

    let resp = match client
        .post(url)
        .bearer_auth(key)
        .json(&body)
        .timeout(Duration::from_secs(30))
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return fallback,
    };

    if !resp.status().is_success() {
        return fallback;
    }

    let v: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return fallback,
    };

    let raw = match v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
    {
        Some(s) => s.to_string(),
        None => return fallback,
    };

    parse_subtasks(&raw, task)
}

/// 容错解析 LLM 返回的 JSON：三级回退，最终仍失败则返回单步回退。
fn parse_subtasks(raw: &str, original_task: &str) -> Vec<SubTask> {
    let fallback = vec![SubTask {
        step: 1,
        goal: original_task.to_string(),
        tool_hint: None,
    }];

    // 第一级：直接 parse
    if let Ok(tasks) = try_parse(raw) {
        if !tasks.is_empty() {
            return tasks;
        }
    }

    // 第二级：剥离 ```json ... ``` 代码块标记
    let stripped = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    if let Ok(tasks) = try_parse(stripped) {
        if !tasks.is_empty() {
            return tasks;
        }
    }

    // 第三级：找第一个 '[' 到最后一个 ']' 的子串
    if let (Some(start), Some(end)) = (raw.find('['), raw.rfind(']')) {
        if start < end {
            let slice = &raw[start..=end];
            if let Ok(tasks) = try_parse(slice) {
                if !tasks.is_empty() {
                    return tasks;
                }
            }
        }
    }

    fallback
}

/// 尝试将字符串解析为 Vec<SubTask>，保持 tool_hint 为 null 时映射成 None。
fn try_parse(s: &str) -> Result<Vec<SubTask>, ()> {
    let arr: Vec<Value> = serde_json::from_str(s.trim()).map_err(|_| ())?;
    let tasks: Vec<SubTask> = arr
        .into_iter()
        .enumerate()
        .filter_map(|(i, v)| {
            let goal = v.get("goal").and_then(|g| g.as_str())?.to_string();
            let step = v
                .get("step")
                .and_then(|s| s.as_u64())
                .map(|n| n as usize)
                .unwrap_or(i + 1);
            let tool_hint = v
                .get("tool_hint")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());
            Some(SubTask { step, goal, tool_hint })
        })
        .collect();
    Ok(tasks)
}
