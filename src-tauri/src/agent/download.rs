use crate::error::AppError;
use crate::types::{AppConfig, ExecuteToolboxResponse};
use futures_util::StreamExt;
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

#[derive(Deserialize)]
struct DownloadInput {
    url: String,
    path: String,
}

pub async fn download(
    input: Option<&str>,
    cfg: &AppConfig,
) -> Result<ExecuteToolboxResponse, AppError> {
    let request: DownloadInput = serde_json::from_str(input.unwrap_or_default())
        .map_err(|_| AppError::ToolboxError("下载参数格式不正确，请提供 url 和 path".into()))?;
    let url = reqwest::Url::parse(request.url.trim())
        .map_err(|_| AppError::ToolboxError("下载地址格式不正确".into()))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || request.path.trim().is_empty()
    {
        return Err(AppError::ToolboxError(
            "下载需要有效的网络地址和本地保存路径".into(),
        ));
    }
    let client = crate::engine::net::finish_client(
        crate::engine::net::client_builder(cfg).timeout(Duration::from_secs(30)),
    )?;
    let response = client.get(url).send().await.map_err(|error| {
        AppError::NetworkError(crate::engine::net::connection_message(
            cfg,
            &request.url,
            &error,
        ))
    })?;
    if !response.status().is_success() {
        return Err(AppError::ToolboxError(format!(
            "下载服务返回状态码 {}",
            response.status().as_u16()
        )));
    }
    let path = Path::new(&request.path);
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|_| AppError::ToolboxError("无法创建下载目录，请检查保存路径和权限".into()))?;
    }
    let mut file = tokio::fs::File::create(path)
        .await
        .map_err(|_| AppError::ToolboxError("无法创建下载文件，请检查保存路径和权限".into()))?;
    let mut stream = response.bytes_stream();
    let mut total_bytes = 0usize;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| {
            AppError::NetworkError(crate::engine::net::connection_message(
                cfg,
                &request.url,
                &error,
            ))
        })?;
        file.write_all(&chunk)
            .await
            .map_err(|_| AppError::ToolboxError("写入下载文件失败，请检查磁盘空间和权限".into()))?;
        total_bytes += chunk.len();
    }
    file.flush()
        .await
        .map_err(|_| AppError::ToolboxError("保存下载文件失败，请检查磁盘空间和权限".into()))?;
    Ok(ExecuteToolboxResponse {
        success: true,
        output: Some(format!("文件已下载（{total_bytes} 字节）")),
        error: None,
    })
}
