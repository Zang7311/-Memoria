// 《铃·记忆体》测试 API 连接命令
// 输入 { base_url, api_key }，发送简单请求测试连通性，返回 { success, message }
use crate::error::AppError;
use crate::types::TestConnectionResponse;

/// 测试 API 连接
#[tauri::command]
pub async fn test_api_connection(
    base_url: String,
    api_key: String,
) -> Result<TestConnectionResponse, AppError> {
    // 未传明文 key 时，回退用配置中心已保存的 key（避免输入框为空时误报 401）
    let key = if api_key.trim().is_empty() {
        resolve_saved_key()
    } else {
        api_key.trim().to_string()
    };
    let url = format!("{}/models", crate::utils::normalize_v1_url(&base_url));

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .bearer_auth(&key)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => Ok(TestConnectionResponse {
            success: true,
            message: format!("连接成功（HTTP {}）", r.status()),
        }),
        Ok(r) => Ok(TestConnectionResponse {
            success: false,
            message: format!("服务可达但返回 HTTP {}", r.status()),
        }),
        Err(e) => Ok(TestConnectionResponse {
            success: false,
            message: format!("连接失败：{e}"),
        }),
    }
}

/// 从配置中心解析已保存的 API Key（优先加密、回退明文；未保存返回空串）
fn resolve_saved_key() -> String {
    let cfg = crate::config::store::get_config();
    if let Some(enc) = &cfg.api_key_encrypted {
        if !enc.is_empty() {
            if let Ok(k) = crate::config::encryption::get_key() {
                if let Ok(p) = crate::config::encryption::decrypt_with_key(&k, enc) {
                    return p;
                }
            }
        }
    }
    if let Some(plain) = &cfg.api_key_plain {
        if !plain.is_empty() {
            return plain.clone();
        }
    }
    String::new()
}
