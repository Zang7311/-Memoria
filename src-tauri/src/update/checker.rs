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
    let cfg = crate::config::store::get_config();
    let client = crate::engine::net::finish_client(crate::engine::net::client_builder(&cfg)
        .user_agent("Memoria-Client/1.0")
        .timeout(std::time::Duration::from_secs(8))
    )?;

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            // 静默降级：网络失败不阻塞启动
            let message = crate::engine::net::connection_message(&cfg, &url, &e);
            log::warn!("[update] 版本检查失败（网络）：{message}");
            return Ok(CheckUpdateResponse {
                has_update: false,
                version_info: None,
                error: Some(message),
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

/// 版本号比较：cur < latest 返回负数，相等 0，cur > latest 正数。
///
/// 按 semver 的规则来（只做需要的那部分）：
/// - 核心三段数字逐个比，缺的位按 0 算
/// - 预发布后缀（-beta.9）不是修订号：0.7.0-beta.9 < 0.7.0 < 0.7.1
/// - 两边都有预发布时逐个标识比，数字按数值（beta.10 > beta.9），数字小于字母
/// - 构建元数据（+xxx）不参与比较
///
/// 之前是直接 split('.') 再 parse::<i32>()，后缀里那段 "0-beta" 解析失败被丢掉，
/// 结果 0.7.0-beta.9 被当成 0.7.9 —— 稳定版 0.7.0 和 0.7.1~0.7.8 全被判成「已是最新」。
fn compare_versions(cur: &str, latest: &str) -> i32 {
    fn split_version(s: &str) -> (Vec<i64>, Vec<String>) {
        let s = s.trim().trim_start_matches('v');
        let s = s.split('+').next().unwrap_or(s);
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (s, None),
        };
        let nums: Vec<i64> = core
            .split('.')
            .map(|p| p.trim().parse::<i64>().unwrap_or(0))
            .collect();
        let pres: Vec<String> = pre
            .map(|p| p.split('.').map(|x| x.to_string()).collect())
            .unwrap_or_default();
        (nums, pres)
    }

    let (a, a_pre) = split_version(cur);
    let (b, b_pre) = split_version(latest);

    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return if x < y { -1 } else { 1 };
        }
    }

    // 核心版本相同：带预发布后缀的那一方更旧
    match (a_pre.is_empty(), b_pre.is_empty()) {
        (true, true) => 0,
        (false, true) => -1,
        (true, false) => 1,
        (false, false) => {
            for i in 0..a_pre.len().max(b_pre.len()) {
                match (a_pre.get(i), b_pre.get(i)) {
                    (None, None) => break,
                    (None, Some(_)) => return -1,
                    (Some(_), None) => return 1,
                    (Some(x), Some(y)) => {
                        let ord = match (x.parse::<i64>(), y.parse::<i64>()) {
                            (Ok(p), Ok(q)) => p.cmp(&q),
                            // 数字标识比字母标识旧（semver 规定）
                            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
                            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
                            (Err(_), Err(_)) => x.cmp(y),
                        };
                        match ord {
                            std::cmp::Ordering::Less => return -1,
                            std::cmp::Ordering::Greater => return 1,
                            std::cmp::Ordering::Equal => {}
                        }
                    }
                }
            }
            0
        }
    }
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

    #[test]
    fn version_compare_prerelease() {
        // 稳定版比同号预发布新
        assert!(compare_versions("0.7.0-beta.9", "0.7.0") < 0);
        assert!(compare_versions("0.7.0", "0.7.0-beta.9") > 0);
        // 预发布后缀不能混进修订号：0.7.1 比 0.7.0-beta.9 新
        assert!(compare_versions("0.7.0-beta.9", "0.7.1") < 0);
        assert!(compare_versions("0.7.5", "0.7.0-beta.9") > 0);
        // 预发布之间：数字按数值比，不能按字符串比
        assert!(compare_versions("0.7.0-beta.9", "0.7.0-beta.10") < 0);
        assert!(compare_versions("0.7.0-beta.9", "0.7.0-beta.8") > 0);
        // 带 v 前缀、带构建元数据
        assert_eq!(compare_versions("0.7.0-beta.9", "v0.7.0-beta.9"), 0);
        assert_eq!(compare_versions("0.7.0+build.5", "0.7.0"), 0);
        // 主次版本不同时后缀不影响判断
        assert!(compare_versions("0.6.3", "0.7.0-beta.1") < 0);
        assert!(compare_versions("0.8.0-beta.1", "0.7.0") > 0);
    }
}
