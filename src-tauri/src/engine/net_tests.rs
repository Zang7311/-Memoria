use super::*;
use crate::config::defaults::default_config;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn http_server(status: u16) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        let (mut socket, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
            .await
            .unwrap()
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        loop {
            let count = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut buffer))
                .await
                .unwrap()
                .unwrap();
            if count == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..count]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let response =
            format!("HTTP/1.1 {status} Test\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
        socket.write_all(response.as_bytes()).await.unwrap();
        String::from_utf8(request).unwrap()
    });
    (address, handle)
}

async fn closed_address() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    format!("http://{}", listener.local_addr().unwrap())
}

#[test]
fn old_configuration_deserializes_to_direct() {
    let mut value = serde_json::to_value(default_config()).unwrap();
    let object = value.as_object_mut().unwrap();
    object.remove("proxy_enabled");
    object.remove("proxy_url");
    let cfg: AppConfig = serde_json::from_value(value).unwrap();
    assert!(!cfg.proxy_enabled);
    assert!(cfg.proxy_url.is_none());
    assert!(proxy_warning(&cfg).is_none());
    assert!(policy_message(&cfg).contains("直连"));
}

#[test]
fn proxy_settings_round_trip() {
    let mut cfg = default_config();
    cfg.proxy_enabled = true;
    cfg.proxy_url = Some("http://127.0.0.1:7890".into());
    let decoded: AppConfig = serde_json::from_value(serde_json::to_value(&cfg).unwrap()).unwrap();
    assert!(decoded.proxy_enabled);
    assert_eq!(decoded.proxy_url, cfg.proxy_url);
    assert!(proxy_url(&decoded).is_some());
}

#[tokio::test]
async fn default_ignores_system_proxy_environment() {
    const CHILD_FLAG: &str = "MEM_NETWORK_DIRECT_CHILD";
    if std::env::var_os(CHILD_FLAG).is_none() {
        let dead_proxy = closed_address().await;
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "engine::net::tests::default_ignores_system_proxy_environment",
                "--nocapture",
            ])
            .env(CHILD_FLAG, "1");
        for name in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
        ] {
            command.env(name, &dead_proxy);
        }
        for name in ["NO_PROXY", "no_proxy"] {
            command.env(name, "");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "隔离子进程直连验证失败：{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    assert!(std::env::var("HTTP_PROXY")
        .unwrap()
        .starts_with("http://127.0.0.1:"));
    for stale_url in [None, Some(std::env::var("HTTP_PROXY").unwrap())] {
        let (address, server) = http_server(200).await;
        let mut cfg = default_config();
        cfg.proxy_url = stale_url;
        let response = build_client(&cfg)
            .unwrap()
            .get(address)
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 200);
        assert!(server.await.unwrap().starts_with("GET / HTTP/1.1"));
    }
}

#[tokio::test]
async fn explicitly_enabled_proxy_receives_http_request() {
    let (address, server) = http_server(200).await;
    let mut cfg = default_config();
    cfg.proxy_enabled = true;
    cfg.proxy_url = Some(format!("  {address}  "));
    let response = build_client(&cfg)
        .unwrap()
        .get("http://unresolvable.invalid/v1/models")
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .unwrap();
    assert_eq!(response.text().await.unwrap(), "ok");
    assert!(server
        .await
        .unwrap()
        .starts_with("GET http://unresolvable.invalid/v1/models HTTP/1.1"));
}

#[tokio::test]
async fn explicitly_enabled_proxy_receives_https_connect() {
    let (address, server) = http_server(401).await;
    let mut cfg = default_config();
    cfg.proxy_enabled = true;
    cfg.proxy_url = Some(address);
    assert!(build_client(&cfg)
        .unwrap()
        .get("https://unresolvable.invalid/v1/models")
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .is_err());
    assert!(server
        .await
        .unwrap()
        .starts_with("CONNECT unresolvable.invalid:443 HTTP/1.1"));
}

#[tokio::test]
async fn invalid_or_empty_proxy_falls_back_to_direct_without_panicking() {
    for address in [
        None,
        Some(""),
        Some("  "),
        Some("not a url"),
        Some("127.0.0.1:7890"),
        Some("http://"),
        Some("ftp://localhost:7890"),
        Some("socks5://localhost:7890"),
        Some("http://localhost:7890/path"),
        Some("http://localhost:7890?key=secret"),
        Some("http://localhost:99999"),
        Some("http://localhost:7890#secret"),
    ] {
        let mut cfg = default_config();
        cfg.proxy_enabled = true;
        cfg.proxy_url = address.map(str::to_string);
        assert_eq!(proxy_warning(&cfg), Some(INVALID_PROXY_WARNING));
        let (target, server) = http_server(200).await;
        assert_eq!(
            build_client(&cfg)
                .unwrap()
                .get(target)
                .timeout(Duration::from_secs(3))
                .send()
                .await
                .unwrap()
                .status()
                .as_u16(),
            200
        );
        server.await.unwrap();
    }
}

#[tokio::test]
async fn valid_but_closed_proxy_does_not_silently_switch_to_direct() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut cfg = default_config();
    cfg.proxy_enabled = true;
    cfg.proxy_url = Some(closed_address().await);
    let error = build_client(&cfg)
        .unwrap()
        .get(format!("http://{}", listener.local_addr().unwrap()))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .unwrap_err();
    assert!(connection_message(&cfg, "https://api.deepseek.com", &error).contains("代理软件"));
    assert!(
        tokio::time::timeout(Duration::from_millis(100), listener.accept())
            .await
            .is_err()
    );
}

#[test]
fn proxy_credentials_and_url_secrets_are_not_displayed() {
    let mut cfg = default_config();
    cfg.proxy_enabled = true;
    cfg.proxy_url = Some("http://proxy-user:proxy-password@127.0.0.1:7890".into());
    assert_eq!(
        policy_message(&cfg),
        "当前网络策略：走代理 http://127.0.0.1:7890"
    );
    assert_eq!(safe_address("https://api-user:api-password@example.com/sk-path-secret?key=api-secret#fragment-secret"), "https://example.com");
}

#[tokio::test]
async fn chinese_connection_errors_are_actionable_and_redacted() {
    let mut cfg = default_config();
    let error = build_client(&cfg)
        .unwrap()
        .get(format!(
            "{}/sk-path-secret?key=query-secret",
            closed_address().await
        ))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .unwrap_err();
    let message = connection_message(&cfg, error.url().unwrap().as_str(), &error);
    assert!(message.contains("直连"));
    assert!(message.contains("设置 → 网络"));
    assert!(!message.contains("sk-path-secret"));
    assert!(!message.contains("query-secret"));
    cfg.proxy_enabled = true;
    assert!(connection_message(&cfg, "https://api.deepseek.com", &error)
        .contains(INVALID_PROXY_WARNING));
}

#[tokio::test]
async fn diagnostics_report_direct_401_and_keep_keys_out_of_requests_and_output() {
    let (address, server) = http_server(401).await;
    let mut cfg = default_config();
    cfg.api_base_url = Some(format!("{address}/v1?key=query-secret#fragment-secret"));
    cfg.api_key_plain = Some("configured-secret".into());
    let report = diagnose(&cfg).await.unwrap();
    assert!(report.contains("当前网络策略"));
    assert!(report.contains("直连"));
    assert!(report.contains("通（401"));
    assert!(report.contains("本机网络：正常"));
    let request = server.await.unwrap();
    for secret in ["query-secret", "fragment-secret", "configured-secret"] {
        assert!(!report.contains(secret));
        assert!(!request.contains(secret));
    }
    assert!(!request.to_lowercase().contains("authorization:"));
}

#[tokio::test]
async fn diagnostics_remove_url_authentication_before_probe() {
    let (address, server) = http_server(200).await;
    let mut cfg = default_config();
    cfg.api_base_url = Some(address.replace("http://", "http://url-user:url-password@"));
    let report = diagnose(&cfg).await.unwrap();
    assert!(!report.contains("url-user"));
    assert!(!report.contains("url-password"));
    assert!(!server
        .await
        .unwrap()
        .to_lowercase()
        .contains("authorization:"));
}

#[tokio::test]
async fn diagnostics_continue_after_one_site_fails_and_include_all_configured_apis() {
    let (address, server) = http_server(200).await;
    let mut cfg = default_config();
    cfg.api_base_url = Some(closed_address().await);
    cfg.cheap_api_base_url = Some(address.clone());
    cfg.models.push(crate::types::ModelSlot {
        id: "disabled".into(),
        name: "disabled".into(),
        enabled: false,
        roles: Vec::new(),
        base_url: Some("invalid-secret-address".into()),
        api_key_plain: Some("slot-secret".into()),
        api_key_encrypted: None,
    });
    let report = diagnose(&cfg).await.unwrap();
    assert!(report.contains("未连通"));
    assert!(report.contains("地址格式不正确"));
    assert!(report.contains("通（200"));
    assert!(report.contains("本机网络：正常"));
    assert!(!report.contains("slot-secret"));
    assert!(!report.contains("invalid-secret-address"));
    server.await.unwrap();
}

#[tokio::test]
async fn diagnostics_warn_when_enabled_proxy_is_invalid() {
    let (address, server) = http_server(200).await;
    let mut cfg = default_config();
    cfg.api_base_url = Some(address);
    cfg.proxy_enabled = true;
    cfg.proxy_url = Some("invalid-secret-proxy".into());
    let report = diagnose(&cfg).await.unwrap();
    assert!(report.contains(INVALID_PROXY_WARNING));
    assert!(report.contains("直连"));
    assert!(!report.contains("invalid-secret-proxy"));
    server.await.unwrap();
}

#[tokio::test]
async fn diagnostics_warn_about_closed_proxy_and_still_report_each_api() {
    let mut cfg = default_config();
    cfg.proxy_enabled = true;
    cfg.proxy_url = Some(closed_address().await);
    cfg.api_base_url = Some("https://first.invalid".into());
    cfg.cheap_api_base_url = Some("https://second.invalid".into());
    let report = diagnose(&cfg).await.unwrap();
    assert!(report.contains("代理端口：无响应"));
    assert!(report.contains("建议关闭代理开关或启动代理软件"));
    assert!(report.contains("https://first.invalid"));
    assert!(report.contains("https://second.invalid"));
    assert!(report.contains("当前网络策略：走代理"));
}

#[test]
fn diagnostic_targets_deduplicate_and_skip_empty_addresses() {
    let mut cfg = default_config();
    cfg.api_base_url = Some(" https://example.com ".into());
    cfg.cheap_api_base_url = Some("https://example.com".into());
    assert_eq!(diagnostic_targets(&cfg), vec!["https://example.com"]);
    cfg.api_base_url = None;
    cfg.cheap_api_base_url = Some("  ".into());
    assert_eq!(diagnostic_targets(&cfg).len(), 2);
}

#[tokio::test]
async fn agent_download_uses_explicit_proxy_and_writes_bytes() {
    let (address, server) = http_server(200).await;
    let mut cfg = default_config();
    cfg.proxy_enabled = true;
    cfg.proxy_url = Some(address);
    let path =
        std::env::temp_dir().join(format!("mem-network-download-{}.txt", uuid::Uuid::new_v4()));
    let input = serde_json::json!({ "url": "http://unresolvable.invalid/file?key=download-secret", "path": path });
    let response = crate::agent::download::download(Some(&input.to_string()), &cfg)
        .await
        .unwrap();
    assert!(response.success);
    assert_eq!(tokio::fs::read(&path).await.unwrap(), b"ok");
    assert!(!response.output.unwrap().contains("download-secret"));
    tokio::fs::remove_file(path).await.unwrap();
    assert!(server
        .await
        .unwrap()
        .starts_with("GET http://unresolvable.invalid/file?key=download-secret HTTP/1.1"));
}

#[tokio::test]
async fn agent_download_rejects_invalid_input_without_echoing_it() {
    for input in [
        None,
        Some("message-secret"),
        Some("{\"url\":\"file:///secret\",\"path\":\"x\"}"),
    ] {
        let error = crate::agent::download::download(input, &default_config())
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("下载"));
        assert!(!error.contains("secret"));
    }
}
