// 《铃·记忆体》同步加密（AI-8）
//
// ⚠️ 与 AI-7 密钥体系严格统一：不自行生成密钥，
// 直接复用 config/encryption.rs 的内存派生密钥 + AES-256-GCM 加解密。
//
// 用户首次同步前必须已设置主密码并解锁（AI-7 unlock），
// 重装系统后输入相同主密码即可恢复同步加密。
use crate::config::encryption;
use crate::error::AppError;

/// 加密记忆 JSON（返回 base64 密文），未解锁时返回 Locked
pub fn encrypt_memories(plain_json: &str) -> Result<String, AppError> {
    let key = encryption::get_key()?; // 未解锁 → AppError::Locked
    encryption::encrypt_with_key(&key, plain_json)
}

/// 解密记忆 JSON（base64 密文 → 明文），未解锁或密码不符时报错
pub fn decrypt_memories(encrypted_b64: &str) -> Result<String, AppError> {
    let key = encryption::get_key()?;
    encryption::decrypt_with_key(&key, encrypted_b64)
}

/// 当前是否已解锁（可同步）
pub fn is_unlocked() -> bool {
    encryption::is_unlocked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::encryption as ce;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        // 直接注入密钥（模拟已解锁）
        let key = ce::derive_key("sync-test", b"fixed-salt-16b!!");
        ce::set_key(key);

        let plain = r#"[{"id":"m1","role":"user","content":"你好","timestamp":"2026-08-29T10:00:00Z"}]"#;
        let enc = encrypt_memories(plain).unwrap();
        // 密文不应包含明文
        assert!(!enc.contains("你好"));
        let dec = decrypt_memories(&enc).unwrap();
        assert_eq!(dec, plain);
        ce::clear_key();
    }

    #[test]
    fn locked_returns_error() {
        ce::clear_key();
        assert!(matches!(encrypt_memories("[]"), Err(AppError::Locked)));
    }
}
