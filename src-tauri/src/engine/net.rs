use crate::error::AppError;
use crate::types::AppConfig;
use reqwest::{Client, ClientBuilder, Proxy, Url};
use std::time::Duration;

#[cfg(test)]
#[path = "net_tests.rs"]
mod tests;

pub const INVALID_PROXY_WARNING: &str = "代理地址为空或格式不对，已改为直连";

pub fn proxy_url(cfg: &AppConfig) -> Option<Url> {
    if !cfg.proxy_enabled {
        return None;
    }
    let url = Url::parse(cfg.proxy_url.as_deref()?.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || url.port_or_known_default().is_none()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || Proxy::all(url.clone()).is_err()
    {
        return None;
    }
    Some(url)
}

pub fn proxy_warning(cfg: &AppConfig) -> Option<&'static str> {
    (cfg.proxy_enabled && proxy_url(cfg).is_none()).then_some(INVALID_PROXY_WARNING)
}

pub fn safe_address(address: &str) -> String {
    match Url::parse(address) {
        Ok(url) if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() => {
            url.origin().ascii_serialization()
        }
        _ => "未设置或格式不正确的地址".into(),
    }
}

pub fn policy_message(cfg: &AppConfig) -> String {
    match proxy_url(cfg) {
        Some(url) => format!("当前网络策略：走代理 {}", safe_address(url.as_str())),
        None => "当前网络策略：直连（默认，不走系统代理或环境变量代理）".into(),
    }
}

pub fn client_builder(cfg: &AppConfig) -> ClientBuilder {
    let builder = Client::builder().no_proxy();
    match proxy_url(cfg).and_then(|url| Proxy::all(url).ok()) {
        Some(proxy) => builder.proxy(proxy),
        None => builder,
    }
}

pub fn finish_client(builder: ClientBuilder) -> Result<Client, AppError> {
    builder.build().map_err(|_| {
        AppError::NetworkError(
            "无法初始化网络连接，请检查本机网络及「设置 → 网络」中的代理配置".into(),
        )
    })
}

pub fn build_client(cfg: &AppConfig) -> Result<Client, AppError> {
    finish_client(client_builder(cfg))
}

pub fn connection_message(cfg: &AppConfig, address: &str, error: &reqwest::Error) -> String {
    let reason = if error.is_timeout() {
        "连接超时"
    } else if error.is_connect() {
        "连接被拒绝、地址无法解析或安全连接失败"
    } else if error.is_body() || error.is_decode() {
        "服务响应传输或解析失败"
    } else {
        "请求未能完成"
    };
    let advice = if proxy_url(cfg).is_some() {
        "请检查代理软件是否已启动及代理地址是否正确，也可在「设置 → 网络」关闭代理开关后重试。"
    } else {
        "请检查本机网络。若要访问国外 API（OpenAI / Claude 等），请在「设置 → 网络」启用代理并填写代理地址。"
    };
    let warning = proxy_warning(cfg)
        .map(|warning| format!("\n{warning}。"))
        .unwrap_or_default();
    format!(
        "无法连接到 {}。\n{}。{warning}\n{advice}\n失败原因：{reason}。",
        safe_address(address),
        policy_message(cfg)
    )
}

fn diagnostic_targets(cfg: &AppConfig) -> Vec<String> {
    let mut targets = Vec::new();
    for address in cfg
        .api_base_url
        .iter()
        .chain(cfg.cheap_api_base_url.iter())
        .chain(cfg.models.iter().filter_map(|slot| slot.base_url.as_ref()))
    {
        let address = address.trim();
        if !address.is_empty() && !targets.iter().any(|target| target == address) {
            targets.push(address.to_string());
        }
    }
    if targets.is_empty() {
        targets.extend([
            "https://api.deepseek.com".into(),
            "https://www.78code.cc".into(),
        ]);
    }
    targets
}

async fn probe_address(client: &Client, address: &str) -> (bool, String) {
    let label = safe_address(address);
    let mut url = match Url::parse(address) {
        Ok(url) if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() => url,
        _ => {
            return (
                false,
                format!("  {label}：地址格式不正确，请在模型设置中修改"),
            )
        }
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    match client.get(url).send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            let note = if status == 401 {
                "服务可达，未授权属正常"
            } else {
                "服务已响应"
            };
            (true, format!("  {label}：通（{status}，{note}）"))
        }
        Err(_) => (
            false,
            format!("  {label}：未连通，请检查网络及「设置 → 网络」"),
        ),
    }
}

pub async fn diagnose(cfg: &AppConfig) -> Result<String, AppError> {
    let mut lines = vec![policy_message(cfg)];
    if let Some(warning) = proxy_warning(cfg) {
        lines.push(warning.into());
    }
    if let Some(url) = proxy_url(cfg) {
        let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
        let port = url.port_or_known_default().unwrap_or(80);
        let reachable = matches!(
            tokio::time::timeout(
                Duration::from_secs(3),
                tokio::net::TcpStream::connect((host, port))
            )
            .await,
            Ok(Ok(_))
        );
        lines.push(if reachable {
            "代理端口：有响应（实际代理转发能力以下方地址测试为准）".into()
        } else {
            "代理端口：无响应（可能代理软件没开），建议关闭代理开关或启动代理软件".into()
        });
    }
    let client = finish_client(
        client_builder(cfg)
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none()),
    )?;
    lines.push(
        if proxy_url(cfg).is_some() {
            "代理连接测试："
        } else {
            "直连测试："
        }
        .into(),
    );
    let targets = diagnostic_targets(cfg);
    let results = futures_util::future::join_all(
        targets
            .iter()
            .map(|address| probe_address(&client, address)),
    )
    .await;
    let any_reachable = results.iter().any(|(reachable, _)| *reachable);
    lines.extend(results.into_iter().map(|(_, line)| line));
    lines.push(if any_reachable {
        "本机网络：正常（个别地址失败请检查对应服务或代理设置）".into()
    } else {
        "本机网络：检测地址均未连通，请检查本机网络或代理设置".into()
    });
    Ok(lines.join("\n"))
}
