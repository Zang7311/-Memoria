// 《铃·记忆体》Agent 流式进度推送（stream_progress.rs）
//
// 职责：把 Agent 循环执行过程中的「进度文本」和「最终回复」，
// 以流式事件（chat_chunk / chat_end）逐步推送到前端，让用户能看到
// Agent 正在做什么，而不是干等整段返回。
//
// 设计思路：
//   1. 复用 stream::sender 的 send_chunk / send_end，前端无需任何改动，
//      直接沿用现有 useStreamRender 渲染逻辑。
//   2. 进度文本（如「正在调用工具 X…」）与最终回复都走 chat_chunk 推送，
//      前端把它们当作同一段流式内容累加显示，形成完整的执行轨迹。
//   3. 每个阶段都用 send_end 收尾，前端据此知道该阶段结束、可以换行/刷新。
//      进度文本属于「过程提示」，最终回复才是「结果」，两者都流式推送，
//      但最终回复单独成段，便于前端在视觉上区分。
//   4. AgentRunResponse 仍然返回（作为最终确认 + steps 统计），
//      但真正的内容已经通过事件推完了——两者不冲突，前端可二选一使用。
//
// 硬性约束：本文件只依赖 stream::sender，不修改 sender.rs，不改前端。

use tauri::AppHandle;

use crate::stream::sender::{send_chunk, send_end};

/// 单次 chat_chunk 推送的最大字符数。
/// 进度/回复文本按此大小切块，模拟「流式打字」效果，避免一次推一大段。
const CHUNK_MAX_CHARS: usize = 16;

/// 流式发射器：封装 AppHandle + 进度开关，统一推进度与回复。
///
/// `progress_events` 为 false 时，只推送最终回复、不推进度提示
/// （对应 AgentRunRequest.progress_events 字段，默认 true）。
pub struct StreamEmitter {
    app: AppHandle,
    progress_events: bool,
}

impl StreamEmitter {
    /// 构造发射器
    pub fn new(app: AppHandle, progress_events: bool) -> Self {
        Self {
            app,
            progress_events,
        }
    }

    /// 推送一段进度提示（受 progress_events 开关控制）。
    ///
    /// 以「\n\n🔧 …」形式作为一个独立小块推给前端，
    /// 结束后发送 chat_end 让前端识别该阶段收尾。
    pub fn push_progress(&self, text: &str) {
        if !self.progress_events {
            return;
        }
        // 进度提示较短，整段推 + 换行分隔，便于前端区分过程与结果
        let payload = format!("\n\n🔧 {text}\n");
        let _ = send_chunk(&self.app, &payload);
        let _ = send_end(&self.app);
    }

    /// 推送最终回复（逐字切块流式 + chat_end 收尾）。
    ///
    /// 无论 progress_events 开关如何，最终回复都会推送，
    /// 因为这是用户真正关心的结果。
    pub fn push_final_reply(&self, reply: &str) {
        if reply.is_empty() {
            return;
        }
        // 逐块推送，模拟流式打字
        for chunk in split_chunks(reply, CHUNK_MAX_CHARS) {
            let _ = send_chunk(&self.app, chunk);
        }
        // 收尾：前端据此停止 loading、结束本轮渲染
        let _ = send_end(&self.app);
    }
}

/// 把一段文本按最大字符数切成若干块（按字符边界切，避免截断多字节字符）。
///
/// 实现要点：以「字符起始字节位置」作为切点。遍历 char_indices，
/// 当累计字符数达到 max_chars 时，在下一个字符起始处切一刀。
/// max_chars 至少为 1，防止空块 / 死循环。
fn split_chunks(text: &str, max_chars: usize) -> Vec<&str> {
    let max_chars = max_chars.max(1);
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut count = 0usize;
    for (i, _) in text.char_indices() {
        if count == max_chars {
            chunks.push(&text[start..i]);
            start = i;
            count = 0;
        }
        count += 1;
    }
    if start < text.len() {
        chunks.push(&text[start..]);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 切块_短文本_单块() {
        let chunks = split_chunks("你好", 16);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "你好");
    }

    #[test]
    fn 切块_长文本_多块() {
        let text = "这是一段用于测试切块逻辑的比较长的文本内容，用来验证按字符切块是否正确。";
        let chunks = split_chunks(text, 5);
        assert!(chunks.len() > 1);
        // 拼接回原文，验证无丢失
        let joined: String = chunks.concat();
        assert_eq!(joined, text);
    }
}
