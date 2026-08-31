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
    /// 服务端 → 客户端：认证挑战（32 字节随机 nonce，base64）
    ///
    /// 每条连接一次性生成，用完即弃，使抓包重放的应答失效。
    Challenge { nonce: String },
    /// 客户端 → 服务端：挑战应答（HMAC-SHA256(派生密钥, nonce) 的 hex）
    Auth { auth: SyncAuth },
    /// 客户端 → 服务端：请求同步
    Request { request: SyncRequest },
    /// 服务端 → 客户端：一批加密记忆（每批 ≤ MAX_BATCH 条）
    Payload { payload: SyncPayload },
    /// 客户端 → 服务端：接收结果确认
    Response { response: SyncResponse },
}

/// 挑战应答内容（客户端 → 服务端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAuth {
    /// 发起方设备 ID（仅用于日志与历史记录，不作为信任凭据）
    pub device_id: String,
    /// HMAC-SHA256(派生密钥, 域分隔串 || nonce) 的小写 hex
    pub mac: String,
}

/// HMAC 域分隔前缀：绑定用途，避免同一密钥在别处的签名被挪用
pub const AUTH_DOMAIN: &str = "ling-sync-auth-v2:";

/// 挑战 nonce 长度（字节）
pub const NONCE_LEN: usize = 32;

/// 生成随机挑战 nonce（返回 base64）
pub fn generate_nonce_b64() -> String {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    use rand::RngCore;
    let mut nonce = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    STANDARD.encode(nonce)
}

/// 用派生密钥对挑战 nonce 计算 HMAC-SHA256，返回小写 hex
///
/// 双方用同一主密码派生出同一密钥才能算出相同 MAC，
/// 因此未配对（主密码不一致）的设备无法通过认证。
pub fn compute_auth_mac(key: &[u8], nonce_b64: &str) -> String {
    use hmac::{Hmac, Mac};
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key)
        .expect("HMAC-SHA256 接受任意长度密钥");
    mac.update(AUTH_DOMAIN.as_bytes());
    mac.update(nonce_b64.as_bytes());
    let tag = mac.finalize().into_bytes();
    let mut s = String::with_capacity(tag.len() * 2);
    for b in tag {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// 常量时间比较两个 MAC（hex 串），防时序侧信道
pub fn verify_auth_mac(key: &[u8], nonce_b64: &str, provided_mac: &str) -> bool {
    use subtle::ConstantTimeEq;
    let expected = compute_auth_mac(key, nonce_b64);
    // 长度不等直接失败；长度相等时走常量时间比较
    if expected.len() != provided_mac.len() {
        return false;
    }
    expected
        .as_bytes()
        .ct_eq(provided_mac.as_bytes())
        .into()
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

    // ==================== 挑战-应答认证测试 ====================

    #[test]
    fn nonce_每次不同且长度正确() {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine as _;
        let a = generate_nonce_b64();
        let b = generate_nonce_b64();
        assert_ne!(a, b, "nonce 必须每次随机，否则应答可重放");
        assert_eq!(STANDARD.decode(&a).unwrap().len(), NONCE_LEN);
    }

    #[test]
    fn mac_同密钥同nonce可验证() {
        let key = [7u8; 32];
        let nonce = generate_nonce_b64();
        let mac = compute_auth_mac(&key, &nonce);
        assert_eq!(mac.len(), 64);
        assert!(verify_auth_mac(&key, &nonce, &mac));
    }

    #[test]
    fn mac_密钥不一致则验证失败() {
        // 对应「双端主密码不一致」→ 未配对设备被拒
        let key_a = [1u8; 32];
        let key_b = [2u8; 32];
        let nonce = generate_nonce_b64();
        let mac = compute_auth_mac(&key_a, &nonce);
        assert!(!verify_auth_mac(&key_b, &nonce, &mac));
    }

    #[test]
    fn mac_换nonce则旧应答失效_防重放() {
        // 核心安全性质：抓包得到的 (nonce1, mac1) 无法用于新连接的 nonce2
        let key = [9u8; 32];
        let nonce1 = generate_nonce_b64();
        let nonce2 = generate_nonce_b64();
        let mac1 = compute_auth_mac(&key, &nonce1);
        assert!(verify_auth_mac(&key, &nonce1, &mac1));
        assert!(
            !verify_auth_mac(&key, &nonce2, &mac1),
            "旧 MAC 在新 nonce 下必须失效"
        );
    }

    #[test]
    fn mac_空或畸形应答被拒() {
        let key = [3u8; 32];
        let nonce = generate_nonce_b64();
        assert!(!verify_auth_mac(&key, &nonce, ""));
        assert!(!verify_auth_mac(&key, &nonce, "deadbeef"));
        // 全 0 的等长伪造串
        assert!(!verify_auth_mac(&key, &nonce, &"0".repeat(64)));
    }

    #[test]
    fn mac_域分隔生效() {
        // 同密钥下，对"裸 nonce"的 HMAC 不等于带域前缀的 MAC
        use hmac::{Hmac, Mac};
        let key = [5u8; 32];
        let nonce = generate_nonce_b64();
        let mut raw = <Hmac<Sha256> as Mac>::new_from_slice(&key).unwrap();
        raw.update(nonce.as_bytes());
        let raw_hex: String = raw
            .finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_ne!(raw_hex, compute_auth_mac(&key, &nonce));
    }

    #[test]
    fn frame_roundtrip_challenge与auth() {
        let nonce = generate_nonce_b64();
        let frame = encode_frame(&SyncEnvelope::Challenge {
            nonce: nonce.clone(),
        })
        .unwrap();
        match decode_frame(&frame).unwrap() {
            SyncEnvelope::Challenge { nonce: n } => assert_eq!(n, nonce),
            _ => panic!("类型不匹配"),
        }

        let env = SyncEnvelope::Auth {
            auth: SyncAuth {
                device_id: "dev-1".into(),
                mac: compute_auth_mac(&[1u8; 32], &nonce),
            },
        };
        let frame = encode_frame(&env).unwrap();
        match decode_frame(&frame).unwrap() {
            SyncEnvelope::Auth { auth } => {
                assert_eq!(auth.device_id, "dev-1");
                assert_eq!(auth.mac.len(), 64);
            }
            _ => panic!("类型不匹配"),
        }
    }
}
