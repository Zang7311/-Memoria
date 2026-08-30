// 《铃·记忆体》GitHub 版本检查（AI-8 任务 7）
//
// GET https://api.github.com/repos/<owner>/铃-记忆体/releases/latest
// 解析 tag_name 与当前版本（Cargo.toml version）对比。
// 检查频率：启动时 + 每 24 小时；失败静默降级（不弹错误）。
use crate::error::AppError;
use crate::types::{CheckUpdateResponse, VersionInfo};
use chrono::{Duration, Utc};
use serde::Deserialize;
use std::sync::Mutex;

/// GitHub 仓库（发布页路径，部署时改成实际用户名）
pub const GITHUB_REPO: &str = "Zang7311/-Memoria";
/// 更新检查间隔（小时）
pub const UPDATE_INTERVAL_HOURS: i64 = 24;

/// GitHub Releases API 响应（只取需要的字段）
#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
}

/// 上次检查时间（进程内），用于 24 小时节流
static LAST_CHECK: Mutex<Option<chrono::DateTime<Utc>>> = Mutex::new(None);

/// 当前版本号（从 Cargo.toml 编译期注入）
pub fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// 检查 GitHub Releases 最新版本
/// - 距上次检查不足 24 小时 → 返回缓存结果（None 时重新检查）
/// - 失败 → 静默降级，返回 error 字段
pub async fn check_update(force: bool) -> Result<CheckUpdateResponse, AppError> {
    if !force {
        let last = *LAST_CHECK.lock().unwrap();
        if let Some(t) = last {
            if Utc::now() - t < Duration::hours(UPDATE_INTERVAL_HOURS) {
                log::debug!("[update] 距上次检查不足 24 小时，跳过");
                return Ok(CheckUpdateResponse {
                    has_update: false,
                    version_info: None,
                    error: None,
                });
            }
        }
    }

    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    // 构建客户端：支持代理（读 HTTP_PROXY/HTTPS_PROXY 环境变量）。
    // reqwest 默认不读系统代理，国内直连 GitHub 会失败（HTTP 000），
    // 用户开 FlClash 等代理工具后，环境变量里有代理地址即可走代理。
    let mut client_builder = reqwest::Client::builder()
        .user_agent("Memoria-Client/1.0")
        .timeout(std::time::Duration::from_secs(8));
    for key in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"] {
        if let Ok(proxy) = std::env::var(key) {
            if !proxy.is_empty() {
                if let Ok(p) = reqwest::Proxy::all(&proxy) {
                    client_builder = client_builder.proxy(p);
                    log::debug!("[update] 使用代理 {}（来自 {key}）", proxy);
                    break;
                }
            }
        }
    }
    let client = client_builder
        .build()
        .map_err(|e| AppError::UpdateCheckError(e.to_string()))?;

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            // 静默降级：网络失败不阻塞启动
            log::warn!("[update] 版本检查失败（网络）：{e}");
            return Ok(CheckUpdateResponse {
                has_update: false,
                version_info: None,
                error: Some(format!("网络不可用：{e}")),
            });
        }
    };

    if !resp.status().is_success() {
        log::warn!("[update] 版本检查失败（HTTP {}）", resp.status());
        return Ok(CheckUpdateResponse {
            has_update: false,
            version_info: None,
            error: Some(format!("GitHub 返回 {}", resp.status())),
        });
    }

    let release: GithubRelease = match resp.json().await {
        Ok(r) => r,
        Err(e) => {
            log::warn!("[update] 版本响应解析失败：{e}");
            return Ok(CheckUpdateResponse {
                has_update: false,
                version_info: None,
                error: Some(format!("响应解析失败：{e}")),
            });
        }
    };

    *LAST_CHECK.lock().unwrap() = Some(Utc::now());

    let cur = current_version();
    let latest = release.tag_name.trim_start_matches('v').to_string();
    let is_outdated = compare_versions(&cur, &latest) < 0;

    let info = VersionInfo {
        current_version: cur,
        latest_version: latest,
        release_url: release.html_url,
        release_notes: release.body.unwrap_or_default(),
        is_outdated,
    };
    if is_outdated {
        log::info!("[update] 发现新版本：{} → {}", info.current_version, info.latest_version);
    }
    Ok(CheckUpdateResponse {
        has_update: is_outdated,
        version_info: Some(info),
        error: None,
    })
}

/// 版本号比较：cur < latest 返回负数（简单三段比较）
fn compare_versions(cur: &str, latest: &str) -> i32 {
    let parse = |s: &str| -> Vec<i32> {
        s.split('.')
            .filter_map(|p| p.parse::<i32>().ok())
            .collect()
    };
    let a = parse(cur);
    let b = parse(latest);
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x - y;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(compare_versions("0.1.0", "0.2.0") < 0);
        assert!(compare_versions("0.2.0", "0.1.9") > 0);
        assert_eq!(compare_versions("1.2.3", "1.2.3"), 0);
        assert!(compare_versions("1.10.0", "1.9.9") > 0);
        // 前缀 v 已去掉
        assert!(compare_versions("0.1.0", "1.0.0") < 0);
    }
}
