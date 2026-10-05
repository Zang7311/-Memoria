use crate::error::AppError;
use base64::Engine;

pub fn is_network_tool(id: &str) -> bool {
    matches!(
        id,
        "agent_web_search" | "agent_web_fetch" | "ip-lookup" | "speedtest" | "agent_git"
    )
}

pub fn prepare_command(command: &str) -> Result<String, AppError> {
    let (prefix, encoded) = command
        .split_once("-EncodedCommand ")
        .ok_or_else(|| AppError::ToolboxError("网络工具命令格式不正确".into()))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|_| AppError::ToolboxError("网络工具命令解码失败".into()))?;
    if bytes.len() % 2 != 0 {
        return Err(AppError::ToolboxError("网络工具命令编码不正确".into()));
    }
    let words: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    let script = String::from_utf16(&words)
        .map_err(|_| AppError::ToolboxError("网络工具命令编码不正确".into()))?;
    let prelude = r#"[Console]::OutputEncoding=[System.Text.Encoding]::UTF8
try {
  [System.Net.WebRequest]::DefaultWebProxy = $null
  if ($env:MEM_PROXY_URL) {
    $uri = [Uri]$env:MEM_PROXY_URL
    $builder = New-Object System.UriBuilder($uri)
    $builder.UserName = ''
    $builder.Password = ''
    $proxy = New-Object System.Net.WebProxy($builder.Uri)
    if ($uri.UserInfo) {
      $parts = $uri.UserInfo -split ':', 2
      $user = [Uri]::UnescapeDataString($parts[0])
      $password = if ($parts.Length -gt 1) { [Uri]::UnescapeDataString($parts[1]) } else { '' }
      $proxy.Credentials = New-Object System.Net.NetworkCredential($user, $password)
    }
    [System.Net.WebRequest]::DefaultWebProxy = $proxy
  }
} catch { Write-Output $env:MEM_NETWORK_ERROR; exit 1 }
"#;
    let script = script
        .replace(
            "Search failed: $($_.Exception.Message)",
            "$env:MEM_NETWORK_ERROR",
        )
        .replace(
            "Fetch failed: $($_.Exception.Message)",
            "$env:MEM_NETWORK_ERROR",
        )
        .replace(
            "'Query failed: '+$_.Exception.Message",
            "$env:MEM_NETWORK_ERROR",
        )
        .replace(
            "'Speed test failed: '+$_.Exception.Message",
            "$env:MEM_NETWORK_ERROR",
        );
    let script = format!("{prelude}\n{script}");
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    Ok(format!(
        "{prefix}-EncodedCommand {}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_http_tools_disable_system_proxy_and_use_only_private_environment() {
        let items: Vec<crate::types::ToolboxItem> =
            serde_json::from_str(include_str!("../../resources/agent_tools.json")).unwrap();
        let presets: Vec<crate::types::ToolboxItem> =
            serde_json::from_str(include_str!("../../resources/toolbox_presets.json")).unwrap();
        let mut count = 0;
        for item in items
            .iter()
            .chain(presets.iter())
            .filter(|item| is_network_tool(&item.id))
        {
            let command = prepare_command(&item.command).unwrap();
            let encoded = command.split_once("-EncodedCommand ").unwrap().1;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .unwrap();
            let words: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .collect();
            let script = String::from_utf16(&words).unwrap();
            assert!(script.contains("DefaultWebProxy = $null"));
            assert!(script.contains("$env:MEM_PROXY_URL"));
            assert!(!script.contains("$_.Exception.Message"));
            assert!(!script.contains("http.proxy=http://127.0.0.1:7890"));
            count += 1;
        }
        assert_eq!(count, 5);
    }

    #[test]
    fn invalid_encoded_commands_fail_without_echoing_input() {
        for command in [
            "sensitive-input",
            "powershell -EncodedCommand sensitive-input",
        ] {
            let error = prepare_command(command).unwrap_err().to_string();
            assert!(!error.contains("sensitive-input"));
            assert!(error.contains("网络工具"));
        }
    }

    #[cfg(windows)]
    async fn run_powershell_probe(proxy: &str, target: &str) -> String {
        let script = r#"try { $response=Invoke-WebRequest -Uri $env:MEM_TEST_URL -UseBasicParsing -TimeoutSec 3; $response.StatusCode } catch { Write-Output $env:MEM_NETWORK_ERROR; exit 1 }"#;
        let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let command = prepare_command(&format!(
            "powershell -NoProfile -EncodedCommand {}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
        .unwrap();
        let encoded = command.split_once("-EncodedCommand ").unwrap().1;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let words: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        let script = format!("[System.Net.WebRequest]::DefaultWebProxy = New-Object System.Net.WebProxy('http://127.0.0.1:1')\n{}", String::from_utf16(&words).unwrap());
        let encoded = base64::engine::general_purpose::STANDARD.encode(
            script
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::process::Command::new("powershell")
                .args(["-NoProfile", "-EncodedCommand", &encoded])
                .env("MEM_PROXY_URL", proxy)
                .env("MEM_TEST_URL", target)
                .env("MEM_NETWORK_ERROR", "网络连接失败，请检查「设置 → 网络」")
                .env("HTTP_PROXY", "http://127.0.0.1:1")
                .env("HTTPS_PROXY", "http://127.0.0.1:1")
                .creation_flags(0x08000000)
                .output(),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(
            output.status.success(),
            "PowerShell 网络策略验证失败：{}",
            String::from_utf8_lossy(&output.stdout)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    #[cfg(windows)]
    async fn http_server() -> (String, tokio::task::JoinHandle<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            let (mut socket, _) =
                tokio::time::timeout(std::time::Duration::from_secs(10), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
            let mut buffer = [0u8; 4096];
            let count = socket.read(&mut buffer).await.unwrap();
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await
                .unwrap();
            String::from_utf8(buffer[..count].to_vec()).unwrap()
        });
        (address, handle)
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn powershell_http_ignores_mock_system_and_environment_proxies_by_default() {
        let (address, server) = http_server().await;
        assert!(run_powershell_probe("", &address).await.contains("200"));
        assert!(server.await.unwrap().starts_with("GET / HTTP/1.1"));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn powershell_http_uses_only_explicit_proxy() {
        let (address, server) = http_server().await;
        assert!(
            run_powershell_probe(&address, "http://unresolvable.invalid/probe")
                .await
                .contains("200")
        );
        assert!(server
            .await
            .unwrap()
            .starts_with("GET http://unresolvable.invalid/probe HTTP/1.1"));
    }
}
