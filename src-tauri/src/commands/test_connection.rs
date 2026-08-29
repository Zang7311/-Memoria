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
    let url = format!("{}/models", crate::utils::normalize_v1_url(&base_url));

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .bearer_auth(&api_key)
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
