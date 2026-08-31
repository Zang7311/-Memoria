// 《铃·记忆体》数据包封装/解析（AI-8）
//
// 帧协议（任务书任务 3）：
//   [4-byte 大端长度][JSON payload]
// 内容为 SyncEnvelope 枚举：Request / Payload / Response 三态。
//
// 校验和：对加密前的明文 JSON 计算 SHA-256（hex），接收端解密后验证完整性。
use crate::error::AppError;
use crate::types::{SyncPayload, SyncRequest, SyncResponse};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 传输信封（一帧 = 一个枚举成员）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncEnvelope {
    /// 客户端 → 服务端：请求同步
    Request { request: SyncRequest },
    /// 服务端 → 客户端：一批加密记忆（每批 ≤ MAX_BATCH 条）
    Payload { payload: SyncPayload },
    /// 客户端 → 服务端：接收结果确认
    Response { response: SyncResponse },
}

/// 每批最多携带的记忆条数（任务书：每 100 条发送一次进度更新）
pub const MAX_BATCH: usize = 100;

/// 计算明文的 SHA-256 校验和（hex 小写）
pub fn sha256_hex(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    let digest = hasher.finalize();
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// 编码一帧：[4-byte 大端长度][JSON]
pub fn encode_frame(envelope: &SyncEnvelope) -> Result<Vec<u8>, AppError> {
    let json = serde_json::to_vec(envelope)?;
    let mut frame = Vec::with_capacity(4 + json.len());
    frame.extend_from_slice(&(json.len() as u32).to_be_bytes());
    frame.extend_from_slice(&json);
    Ok(frame)
}

/// 解码一帧（帧必须以完整长度前缀开头；不足返回 Err）
pub fn decode_frame(frame: &[u8]) -> Result<SyncEnvelope, AppError> {
    if frame.len() < 4 {
        return Err(AppError::SyncError("帧长度不足 4 字节".into()));
    }
    let len = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
    if frame.len() < 4 + len {
        return Err(AppError::SyncError(format!(
            "帧数据不完整：声明 {len} 字节，实际 {} 字节",
            frame.len().saturating_sub(4)
        )));
    }
    let env: SyncEnvelope = serde_json::from_slice(&frame[4..4 + len])?;
    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> SyncRequest {
        SyncRequest {
            device_id: "dev-1".into(),
            set_name: "default".into(),
            last_sync_time: None,
            pairing_code: Some("test-pairing".into()),
        }
    }

    #[test]
    fn frame_roundtrip_request() {
        let env = SyncEnvelope::Request {
            request: sample_request(),
        };
        let frame = encode_frame(&env).unwrap();
        let decoded = decode_frame(&frame).unwrap();
        match decoded {
            SyncEnvelope::Request { request } => {
                assert_eq!(request.device_id, "dev-1");
                assert_eq!(request.set_name, "default");
            }
            _ => panic!("类型不匹配"),
        }
    }

    #[test]
    fn frame_roundtrip_payload() {
        let env = SyncEnvelope::Payload {
            payload: SyncPayload {
                device_id: "dev-2".into(),
                set_name: "default".into(),
                encrypted_data: "aGVsbG8=".into(),
                checksum: "abc".into(),
                incremental: false,
            },
        };
        let frame = encode_frame(&env).unwrap();
        // 帧头 = 大端长度
        let len = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        assert_eq!(len, frame.len() - 4);
        let decoded = decode_frame(&frame).unwrap();
        match decoded {
            SyncEnvelope::Payload { payload } => {
                assert_eq!(payload.encrypted_data, "aGVsbG8=");
                assert!(!payload.incremental);
            }
            _ => panic!("类型不匹配"),
        }
    }

    #[test]
    fn frame_truncated_fails() {
        let env = SyncEnvelope::Request {
            request: sample_request(),
        };
        let frame = encode_frame(&env).unwrap();
        // 截断到只剩长度前缀的前 2 字节
        assert!(decode_frame(&frame[..2]).is_err());
    }

    #[test]
    fn sha256_is_stable() {
        let a = sha256_hex("hello");
        let b = sha256_hex("hello");
        let c = sha256_hex("hello2");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 64);
    }
}
