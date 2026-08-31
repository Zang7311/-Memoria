// 《铃·记忆体》API Key 加密（AI-7 任务 2/3）
//
// 密钥派生统一约定（与 AI-8 共享同一体系）：
//   主密码 + PBKDF2(SHA-256, 100000 次迭代) → 32 字节 AES-256 密钥
//
// 加密流程：
//   1. 首次启动用户设置主密码 → 生成随机盐 → 派生密钥 → 密钥仅存内存
//   2. API Key 用 AES-256-GCM 加密（随机 12 字节 nonce）
//   3. 存储格式：base64( nonce || ciphertext )
//   4. 配置文件中只存密文 + 盐（盐非密钥材料，可落盘）；不含主密码与派生密钥
//
// 重装系统后：用户输入相同主密码 + 相同的盐 → 派生相同密钥 → 解密恢复。
use crate::error::AppError;
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use std::sync::Mutex;
use zeroize::Zeroizing;

/// PBKDF2 迭代次数（任务书强制 100000）
pub const PBKDF2_ITERATIONS: u32 = 100_000;
/// 派生密钥长度（32 字节 = AES-256）
pub const KEY_LEN: usize = 32;
/// GCM nonce 长度（12 字节）
pub const NONCE_LEN: usize = 12;
/// 盐长度（16 字节）
pub const SALT_LEN: usize = 16;

/// 主密码派生的密钥（仅存内存，不落盘）。Zeroizing 包装确保 Drop 时自动清零。
static DERIVED_KEY: Mutex<Option<Zeroizing<[u8; KEY_LEN]>>> = Mutex::new(None);

/// 当前是否已解锁（内存中持有派生密钥）
pub fn is_unlocked() -> bool {
    DERIVED_KEY.lock().unwrap().is_some()
}

/// 设置派生密钥（设置主密码 / 解锁成功时调用）
pub fn set_key(key: [u8; KEY_LEN]) {
    *DERIVED_KEY.lock().unwrap() = Some(Zeroizing::new(key));
}

/// 清除派生密钥（锁定）—— Zeroizing Drop 时自动清零内存
pub fn clear_key() {
    *DERIVED_KEY.lock().unwrap() = None;
}

/// 获取当前密钥副本；未解锁时返回 Locked
pub fn get_key() -> Result<[u8; KEY_LEN], AppError> {
    DERIVED_KEY
        .lock()
        .unwrap()
        .as_deref()
        .copied()
        .ok_or(AppError::Locked)
}

/// 由主密码 + 盐派生 32 字节密钥（PBKDF2-SHA256，100000 次）
pub fn derive_key(password: &str, salt: &[u8]) -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}

/// 生成随机盐（16 字节，返回 base64）
pub fn generate_salt_b64() -> String {
    let mut salt = [0u8; SALT_LEN];
    use rand::RngCore;
    rand::rngs::OsRng.fill_bytes(&mut salt);
    STANDARD.encode(salt)
}

/// 解密 base64 盐 → 字节（供解锁时重新派生）
pub fn decode_salt(b64: &str) -> Result<Vec<u8>, AppError> {
    STANDARD
        .decode(b64)
        .map_err(|e| AppError::DecryptionError(format!("盐解码失败：{e}")))
}

/// 用密钥加密明文（AES-256-GCM），返回 base64(nonce || ciphertext)
pub fn encrypt_with_key(key: &[u8; KEY_LEN], plaintext: &str) -> Result<String, AppError> {
    use aes_gcm::aead::Aead;
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
    use rand::RngCore;

    let cipher = Aes256Gcm::new(key.into());
    let mut nonce = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
        .map_err(|e| AppError::EncryptionError(format!("AES-GCM 加密失败：{e:?}")))?;
    // nonce || ciphertext 拼接后 base64
    let mut buf = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    buf.extend_from_slice(&nonce);
    buf.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(buf))
}

/// 用密钥解密 base64(nonce || ciphertext) → 明文
pub fn decrypt_with_key(key: &[u8; KEY_LEN], encoded: &str) -> Result<String, AppError> {
    use aes_gcm::aead::Aead;
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};

    let raw = STANDARD
        .decode(encoded)
        .map_err(|e| AppError::DecryptionError(format!("密文 base64 解码失败：{e}")))?;
    if raw.len() < NONCE_LEN {
        return Err(AppError::DecryptionError("密文数据不完整".to_string()));
    }
    let (nonce, ciphertext) = raw.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new(key.into());
    let plain = cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| AppError::DecryptionError("解密失败，主密码可能不正确".to_string()))?;
    String::from_utf8(plain)
        .map_err(|e| AppError::DecryptionError(format!("解密结果非 UTF-8：{e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_key_is_stable() {
        // 同一密码 + 同一盐 → 同一密钥（重装后可恢复）
        let salt = b"fixed-salt-16b!!".to_vec();
        let k1 = derive_key("mypassword", &salt);
        let k2 = derive_key("mypassword", &salt);
        assert_eq!(k1, k2);
    }

    #[test]
    fn derive_key_differs_with_password() {
        let salt = b"fixed-salt-16b!!".to_vec();
        let k1 = derive_key("pass-a", &salt);
        let k2 = derive_key("pass-b", &salt);
        assert_ne!(k1, k2);
    }

    #[test]
    fn encrypt_roundtrip() {
        let salt = b"fixed-salt-16b!!".to_vec();
        let key = derive_key("mypassword", &salt);
        let enc = encrypt_with_key(&key, "sk-secret-abc123").unwrap();
        // 密文不应包含明文
        assert!(!enc.contains("secret"));
        let dec = decrypt_with_key(&key, &enc).unwrap();
        assert_eq!(dec, "sk-secret-abc123");
    }

    #[test]
    fn decrypt_wrong_key_fails() {
        let key_a = derive_key("pass-a", b"fixed-salt-16b!!");
        let key_b = derive_key("pass-b", b"fixed-salt-16b!!");
        let enc = encrypt_with_key(&key_a, "hello").unwrap();
        assert!(decrypt_with_key(&key_b, &enc).is_err());
    }

    #[test]
    fn salt_b64_roundtrip() {
        let s = generate_salt_b64();
        let bytes = decode_salt(&s).unwrap();
        assert_eq!(bytes.len(), SALT_LEN);
    }
}
