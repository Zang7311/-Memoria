// 《铃·记忆体》AI-5 插件管理器核心
// 管理插件生命周期：加载、启用、禁用、卸载、安装；
// 维护插件注册表（%APPDATA%/ling-memoria/plugins/registry.json）；
// 启动时自动加载所有已启用插件。
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::types::{Plugin, PluginManifest, Skill, SkillParam};
use crate::plugin::{hermes_compat, loader};

/// 应用数据根目录（与 AI-3/AI-4 保持一致）
pub fn app_data_dir() -> PathBuf {
    std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("ling-memoria")
}

/// 注册表中每个插件的运行时状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginState {
    pub enabled: bool,
    /// 用户实际授予的权限列表（默认空 = 无权限）
    #[serde(default)]
    pub granted: Vec<String>,
}

/// 插件管理器（全局单例，见 plugin/mod.rs）
pub struct PluginManager {
    pub plugins: Vec<Plugin>,
    pub registry: HashMap<String, PluginState>,
    pub user_dir: PathBuf,    // %APPDATA%/ling-memoria/plugins（用户安装的插件）
    pub builtin_dir: PathBuf, // resources/plugins（内置插件，只读）
    pub hermes_dir: PathBuf,  // resources/hermes_plugins（Hermes 兼容示例）
}

impl PluginManager {
    /// 初始化：创建目录、扫描内置 + 用户插件、加载注册表、同步启用状态
    pub fn init(app: &AppHandle) -> Self {
        let user_dir = app_data_dir().join("plugins");
        let mut resource_dir = app
            .path()
            .resource_dir()
            .unwrap_or_else(|_| PathBuf::from("src-tauri"));
        // 开发模式下资源位于 src-tauri/resources/（打包后位于 resource_dir 根）
        if resource_dir.join("resources").exists() {
            resource_dir = resource_dir.join("resources");
        }
        let builtin_dir = resource_dir.join("plugins");
        let hermes_dir = resource_dir.join("hermes_plugins");
        fs::create_dir_all(&user_dir).ok();

        let mut mgr = Self {
            plugins: Vec::new(),
            registry: load_registry(&user_dir),
            user_dir,
            builtin_dir,
            hermes_dir,
        };
        mgr.scan_all();
        mgr
    }

    /// 供测试/无 AppHandle 环境使用的构造
    pub fn new_with_dirs(user_dir: PathBuf, builtin_dir: PathBuf, hermes_dir: PathBuf) -> Self {
        let mut mgr = Self {
            plugins: Vec::new(),
            registry: load_registry(&user_dir),
            user_dir,
            builtin_dir,
            hermes_dir,
        };
        mgr.scan_all();
        mgr
    }

    /// 扫描内置插件、Hermes 示例与用户插件目录
    fn scan_all(&mut self) {
        // 先 clone 目录路径，避免借用冲突
        let builtin_dir = self.builtin_dir.clone();
        let hermes_dir = self.hermes_dir.clone();
        let user_dir = self.user_dir.clone();
        // 1. 内置插件（resources/plugins/ 下每个子目录）
        if self.builtin_dir.exists() {
            self.scan_dir(&builtin_dir, false);
        }
        // 2. Hermes 兼容示例（resources/hermes_plugins/ 下每个子目录）
        if self.hermes_dir.exists() {
            self.scan_dir(&hermes_dir, true);
        }
        // 3. 用户插件（%APPDATA%/ling-memoria/plugins/ 下每个子目录）
        if self.user_dir.exists() {
            self.scan_dir(&user_dir, false);
        }

        // 合并注册表状态：内置插件默认启用并授予 manifest 声明的权限；用户插件默认禁用无权限
        for p in &mut self.plugins {
            let path = PathBuf::from(&p.path);
            let is_builtin = path.starts_with(&self.builtin_dir) || path.starts_with(&self.hermes_dir);
            let state = self.registry.entry(p.id.clone()).or_insert_with(|| PluginState {
                enabled: is_builtin,
                granted: if is_builtin {
                    p.manifest.permissions.clone()
                } else {
                    Vec::new()
                },
            });
            p.enabled = state.enabled;
        }
        self.save_registry();
    }

    fn scan_dir(&mut self, dir: &Path, is_hermes_example: bool) {
        let Ok(entries) = fs::read_dir(dir) else {
            log::warn!("插件目录不可读：{}", dir.display());
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // 跳过符号链接（防止遍历逃逸到插件目录外）
            if path.is_symlink() {
                log::warn!("插件目录中发现符号链接，已跳过：{}", path.display());
                continue;
            }
            if !path.is_dir() {
                continue; // 跳过 registry.json 等文件
            }
            let Some(id) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let plugin = match loader::load_plugin_from_dir(&path, id) {
                Ok(p) => p,
                Err(e) => {
                    log::warn!("插件加载失败（不阻塞启动）：{}", e);
                    continue;
                }
            };
            // Hermes 示例目录用 hermes_compat 生成的 id（hermes_xxx）
            if is_hermes_example {
                if let Ok(p) = hermes_compat::load_hermes_plugin(&path, id) {
                    self.plugins.push(p);
                }
                continue;
            }
            if self.plugins.iter().any(|p| p.id == plugin.id) {
                log::warn!("插件 id 冲突，跳过：{}", plugin.id);
                continue;
            }
            self.plugins.push(plugin);
        }
    }

    /// 插件是否为内置（路径位于内置目录下）
    fn is_builtin_plugin(&self, plugin: &Plugin) -> bool {
        let p = Path::new(&plugin.path);
        p.starts_with(&self.builtin_dir) || p.starts_with(&self.hermes_dir)
    }

    /// 列出全部插件（启用状态与授权权限已从注册表同步）
    pub fn list(&self) -> Vec<Plugin> {
        self.plugins
            .iter()
            .map(|p| {
                let mut p = p.clone();
                p.granted = self
                    .registry
                    .get(&p.id)
                    .map(|s| s.granted.clone())
                    .unwrap_or_default();
                p
            })
            .collect()
    }

    /// 按 id 查找插件（含授权权限）
    pub fn get(&self, id: &str) -> Result<Plugin, AppError> {
        let mut plugin = self
            .plugins
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| AppError::PluginNotFound(id.to_string()))?;
        plugin.granted = self
            .registry
            .get(id)
            .map(|s| s.granted.clone())
            .unwrap_or_default();
        Ok(plugin)
    }

    /// 查找已启用插件中注册的技能（返回插件 + 技能 + 授权权限）
    pub fn find_enabled_skill(
        &self,
        skill_name: &str,
    ) -> Result<(Plugin, Skill, Vec<String>), AppError> {
        for p in &self.plugins {
            if !p.enabled {
                continue;
            }
            if let Some(skill) = p.manifest.skills.iter().find(|s| s.name == skill_name) {
                let granted = self
                    .registry
                    .get(&p.id)
                    .map(|s| s.granted.clone())
                    .unwrap_or_default();
                return Ok((p.clone(), skill.clone(), granted));
            }
        }
        Err(AppError::SkillNotFound(format!(
            "技能「{skill_name}」未找到（请确认对应插件已安装并启用）"
        )))
    }

    /// 启用/禁用插件
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<Plugin, AppError> {
        let idx = self
            .plugins
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| AppError::PluginNotFound(id.to_string()))?;
        let state = self
            .registry
            .entry(id.to_string())
            .or_insert_with(|| PluginState {
                enabled: false,
                granted: Vec::new(),
            });
        state.enabled = enabled;
        self.plugins[idx].enabled = enabled;
        self.save_registry();
        log::info!(
            "{}插件「{}」",
            if enabled { "启用" } else { "禁用" },
            self.plugins[idx].name
        );
        Ok(self.plugins[idx].clone())
    }

    /// 权限粒度控制（allow=true 授予，false 收回）
    pub fn set_permission(&mut self, id: &str, permission: &str, allow: bool) -> Result<Plugin, AppError> {
        let idx = self
            .plugins
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| AppError::PluginNotFound(id.to_string()))?;
        let state = self
            .registry
            .entry(id.to_string())
            .or_insert_with(|| PluginState {
                enabled: false,
                granted: Vec::new(),
            });
        if allow {
            if !state.granted.iter().any(|p| p == permission) {
                state.granted.push(permission.to_string());
            }
        } else {
            state.granted.retain(|p| p != permission);
        }
        self.save_registry();
        log::info!(
            "插件「{}」权限变更：{} → {}",
            self.plugins[idx].name,
            permission,
            if allow { "允许" } else { "拒绝" }
        );
        Ok(self.plugins[idx].clone())
    }

    /// 从本地路径安装插件（复制整个目录到用户插件目录）
    pub fn install_from_path(&mut self, source: &str) -> Result<Plugin, AppError> {
        let src = PathBuf::from(source);
        if !src.exists() || !src.is_dir() {
            return Err(AppError::PluginInstallError(format!(
                "本地路径不存在或不是目录：{source}"
            )));
        }
        let id = src
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| AppError::PluginInstallError("无法确定插件目录名".into()))?
            .to_string();

        // 预加载校验：确认是合法插件（避免复制了垃圾目录）
        loader::load_plugin_from_dir(&src, &id)
            .map_err(|e| AppError::PluginInstallError(format!("插件校验失败：{e}")))?;

        if self.plugins.iter().any(|p| p.id == id) {
            return Err(AppError::PluginAlreadyExists(format!(
                "插件「{id}」已安装（请先卸载）"
            )));
        }

        let dst = self.user_dir.join(&id);
        if dst.exists() {
            return Err(AppError::PluginAlreadyExists(format!(
                "插件目录已存在：{}",
                dst.display()
            )));
        }
        copy_dir_all(&src, &dst)?;

        let mut plugin = loader::load_plugin_from_dir(&dst, &id)
            .map_err(|e| AppError::PluginInstallError(format!("安装后加载失败：{e}")))?;
        plugin.enabled = false; // 新安装插件默认禁用，等待用户启用
        self.registry.insert(
            id.clone(),
            PluginState {
                enabled: false,
                granted: Vec::new(),
            },
        );
        self.plugins.push(plugin.clone());
        self.save_registry();
        log::info!("安装插件成功：{}（{}）", plugin.name, id);
        Ok(plugin)
    }

    /// 指定 id 的插件是否已安装（Git 安装前的重名预检）
    pub fn is_installed(&self, id: &str) -> bool {
        self.plugins.iter().any(|p| p.id == id)
    }

    /// 完成 Git 安装的后半段：从已克隆的临时目录校验 → 复制到用户插件目录 → 注册
    /// （clone 本身在 `install_from_git` 中通过 spawn_blocking 执行，见该函数注释）
    pub fn install_from_cloned_dir(
        &mut self,
        tmp: &Path,
        repo_name: &str,
    ) -> Result<Plugin, AppError> {
        // 插件可能位于仓库根或子目录（含 manifest.json 的子目录）
        let mut plugin_dir = tmp.to_path_buf();
        let mut sub_name: Option<String> = None;
        if !tmp.join("manifest.json").exists() && !tmp.join("config.json").exists() {
            if let Ok(entries) = fs::read_dir(tmp) {
                let found = entries.flatten().find(|e| {
                    let d = e.path();
                    d.is_dir() && (d.join("manifest.json").exists() || d.join("config.json").exists())
                });
                if let Some(found) = found {
                    plugin_dir = found.path();
                    sub_name = found.file_name().to_str().map(|s| s.to_string());
                }
            }
        }

        // 插件 id：仓库根 → repo_name；子目录 → 子目录名。
        // 两者都必须过 sanitize_repo_name，杜绝 ".."/"." 等目录遍历写到 user_dir 之外
        let id = match sub_name {
            Some(name) => sanitize_repo_name(&name)?,
            None => sanitize_repo_name(repo_name)?,
        };
        loader::load_plugin_from_dir(&plugin_dir, &id)
            .map_err(|e| AppError::PluginInstallError(format!("插件校验失败：{e}")))?;

        if self.plugins.iter().any(|p| p.id == id) {
            return Err(AppError::PluginAlreadyExists(format!(
                "插件「{id}」已安装（请先卸载）"
            )));
        }

        let dst = self.user_dir.join(&id);
        if dst.exists() {
            return Err(AppError::PluginAlreadyExists(format!(
                "插件目录已存在：{}",
                dst.display()
            )));
        }
        copy_dir_all(&plugin_dir, &dst)?;

        let mut plugin = loader::load_plugin_from_dir(&dst, &id)
            .map_err(|e| AppError::PluginInstallError(format!("安装后加载失败：{e}")))?;
        plugin.enabled = false;
        self.registry.insert(
            id.clone(),
            PluginState {
                enabled: false,
                granted: Vec::new(),
            },
        );
        self.plugins.push(plugin.clone());
        self.save_registry();
        log::info!("Git 安装插件成功：{}（{}）", plugin.name, id);
        Ok(plugin)
    }

    /// 卸载插件（删除目录 + 注册表移除；内置插件禁止卸载）
    pub fn uninstall(&mut self, id: &str) -> Result<(), AppError> {
        let plugin = self.get(id)?;
        if self.is_builtin_plugin(&plugin) {
            return Err(AppError::PluginInstallError(format!(
                "内置插件「{}」不可卸载（可禁用）",
                plugin.name
            )));
        }
        let dir = PathBuf::from(&plugin.path);
        if dir.exists() {
            fs::remove_dir_all(&dir)
                .map_err(|e| AppError::PluginInstallError(format!("删除插件目录失败：{e}")))?;
        }
        self.plugins.retain(|p| p.id != id);
        self.registry.remove(id);
        self.save_registry();
        log::info!("卸载插件成功：{}", plugin.name);
        Ok(())
    }

    /// 添加自定义终端命令（任务 9）：以"内置形式"注册为用户插件，无需 JS 引擎
    pub fn add_terminal_command(
        &mut self,
        name: &str,
        command: &str,
        description: &str,
    ) -> Result<Plugin, AppError> {
        let name = name.trim();
        let command = command.trim();
        if name.is_empty() || command.is_empty() {
            return Err(AppError::PluginInstallError("命令名与命令内容不能为空".into()));
        }
        if self.plugins.iter().any(|p| p.id == name) {
            return Err(AppError::PluginAlreadyExists(format!(
                "终端命令「{name}」已存在"
            )));
        }
        // 名称仅允许字母数字下划线
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(AppError::PluginInstallError(
                "命令名仅允许字母、数字、下划线".into(),
            ));
        }

        let dir = self.user_dir.join(name);
        if dir.exists() {
            return Err(AppError::PluginAlreadyExists(format!(
                "终端命令目录已存在：{name}"
            )));
        }
        fs::create_dir_all(&dir).map_err(|e| AppError::PluginInstallError(e.to_string()))?;

        let manifest = serde_json::json!({
            "name": format!("终端命令：{name}"),
            "version": "0.1.0",
            "author": "主人",
            "description": description,
            "main": "",
            "skills": [{
                "name": name,
                "description": description,
                "parameters": [],
                "action": format!("command:{command}")
            }],
            "permissions": ["system"],
            "hermes_compatible": false
        });
        fs::write(dir.join("manifest.json"), serde_json::to_string_pretty(&manifest).unwrap())
            .map_err(|e| AppError::PluginInstallError(format!("写入 manifest 失败：{e}")))?;

        let plugin = loader::load_plugin_from_dir(&dir, name)
            .map_err(|e| AppError::PluginInstallError(format!("终端命令注册失败：{e}")))?;
        // 终端命令默认禁用（system 高风险），用户需手动启用
        self.registry.insert(
            name.to_string(),
            PluginState {
                enabled: false,
                granted: Vec::new(),
            },
        );
        self.plugins.push(plugin.clone());
        self.save_registry();
        log::info!("添加终端命令成功：{name} → {command}");
        Ok(plugin)
    }

    /// 持久化注册表（原子写：tmp → rename）
    fn save_registry(&self) {
        let path = self.user_dir.join("registry.json");
        let tmp = self.user_dir.join("registry.json.tmp");
        let json = match serde_json::to_string_pretty(&self.registry) {
            Ok(j) => j,
            Err(e) => {
                log::error!("注册表序列化失败：{e}");
                return;
            }
        };
        if let Err(e) = fs::write(&tmp, json) {
            log::error!("注册表写入失败：{e}");
            return;
        }
        let _ = fs::rename(&tmp, &path);
    }
}

// ==================== Git 安装（URL 校验 + 异步 clone） ====================

/// 校验 Git URL：只允许 https:// 与 git@ 两种形态。
/// 拒绝理由：
/// - `ext::<cmd>` / `file://` 等 scheme 可让 git 直接执行本地命令；
/// - 以 `-` 开头会被 git 当作选项解析（如 `--upload-pack=calc.exe`）；
/// - 换行/回车可污染日志或后续拼接。
pub fn validate_git_url(url: &str) -> Result<(), AppError> {
    let ok = (url.starts_with("https://") || url.starts_with("git@"))
        && !url.starts_with('-')
        && !url.contains('\n')
        && !url.contains('\r')
        && !url.contains('\0');
    if !ok {
        return Err(AppError::PluginInstallError(
            "仅允许 https:// 或 git@ 开头的 Git URL（禁止 ext::/file:// 等协议）".into(),
        ));
    }
    Ok(())
}

/// 校验并归一化仓库名/插件目录名：仅允许字母、数字、下划线、中划线。
/// 这道校验拦住 `..` / `.` / 含分隔符的名字，防止 `user_dir.join(name)` 逃出插件目录。
pub fn sanitize_repo_name(name: &str) -> Result<String, AppError> {
    if name.is_empty()
        || name == ".."
        || name == "."
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AppError::PluginInstallError(format!(
            "Git 仓库名含非法字符（仅允许字母、数字、下划线、中划线）：{name}"
        )));
    }
    Ok(name.to_string())
}

/// 从 URL 末段派生仓库名（去掉 .git 后缀）并做合法性校验
pub fn derive_repo_name(url: &str) -> Result<String, AppError> {
    let last = url
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or_default();
    let name = last.strip_suffix(".git").unwrap_or(last);
    sanitize_repo_name(name)
}

/// 从 Git URL 安装插件（异步：clone 走 spawn_blocking，不占用插件管理器锁）
///
/// 与旧实现的差别：
/// 1. 入口先 `validate_git_url`，仓库名过 `sanitize_repo_name`；
/// 2. `git clone` 参数加 `--` 终止选项解析，并禁用 ext 协议、关闭凭据交互提示；
/// 3. 用 `spawn_blocking` 取代 `block_in_place`（后者在 current_thread runtime 下会 panic）。
pub async fn install_from_git(url: &str) -> Result<Plugin, AppError> {
    let url = url.trim().to_string();
    validate_git_url(&url)?;
    let repo_name = derive_repo_name(&url)?;

    if crate::plugin::with_manager(|m| m.is_installed(&repo_name)) {
        return Err(AppError::PluginAlreadyExists(format!(
            "插件「{repo_name}」已安装（请先卸载）"
        )));
    }

    let tmp = std::env::temp_dir().join(format!(
        "ling_plugin_clone_{}",
        uuid::Uuid::new_v4().simple()
    ));
    let tmp_for_clone = tmp.clone();
    let url_for_clone = url.clone();
    let clone_result = tokio::task::spawn_blocking(move || {
        let dst = tmp_for_clone.to_string_lossy().to_string();
        std::process::Command::new("git")
            // 禁用 ext:: 协议（即便 URL 校验已拦住，多一层兜底）
            .args(["-c", "protocol.ext.allow=never"])
            // `--` 之后的内容一律按位置参数解析，URL 无法再被当作选项
            .args(["clone", "--depth", "1", "--", &url_for_clone, &dst])
            // 私有库缺凭据时直接失败，避免卡在交互式提示上
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
    })
    .await
    .map_err(|e| {
        let _ = fs::remove_dir_all(&tmp);
        AppError::PluginInstallError(format!("git clone 线程异常：{e}"))
    })?;

    let output = clone_result.map_err(|e| {
        let _ = fs::remove_dir_all(&tmp);
        AppError::PluginInstallError(format!("git 命令不可用：{e}"))
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = fs::remove_dir_all(&tmp);
        return Err(AppError::PluginInstallError(format!(
            "git clone 失败：{}",
            stderr.trim()
        )));
    }

    let result = crate::plugin::with_manager(|m| m.install_from_cloned_dir(&tmp, &repo_name));
    let _ = fs::remove_dir_all(&tmp);
    result
}

/// 读取注册表（损坏时重置为空并备份）
fn load_registry(user_dir: &Path) -> HashMap<String, PluginState> {
    let path = user_dir.join("registry.json");
    if !path.exists() {
        return HashMap::new();
    }
    match fs::read_to_string(&path).and_then(|c| {
        serde_json::from_str::<HashMap<String, PluginState>>(&c)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }) {
        Ok(reg) => reg,
        Err(e) => {
            log::warn!("注册表损坏，重置（备份为 registry.json.corrupted）：{e}");
            let _ = fs::rename(&path, user_dir.join("registry.json.corrupted"));
            HashMap::new()
        }
    }
}

/// 递归复制目录
fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), AppError> {
    fs::create_dir_all(dst).map_err(|e| AppError::PluginInstallError(e.to_string()))?;
    for entry in fs::read_dir(src)
        .map_err(|e| AppError::PluginInstallError(format!("读取源目录失败：{e}")))?
    {
        let entry = entry.map_err(|e| AppError::PluginInstallError(e.to_string()))?;
        let ty = entry
            .file_type()
            .map_err(|e| AppError::PluginInstallError(e.to_string()))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to)
                .map_err(|e| AppError::PluginInstallError(format!("复制文件失败：{e}")))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_plugin_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ling_mgr_{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_manifest(dir: &Path, manifest: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("manifest.json"), manifest).unwrap();
        fs::write(dir.join("index.js"), "globalThis.skills = {};").unwrap();
    }

    #[test]
    fn 扫描用户插件与注册表持久化() {
        let root = test_plugin_dir("scan");
        let user = root.join("plugins");
        let builtin = root.join("builtin");
        fs::create_dir_all(&user).unwrap();
        fs::create_dir_all(&builtin).unwrap();

        // 内置插件
        make_manifest(&builtin.join("builtin_hello"), r#"{
            "name": "内置问候", "main": "index.js",
            "skills": [{ "name": "hi", "description": "x", "parameters": [], "action": "js:hi" }],
            "permissions": ["file.read"], "hermes_compatible": false
        }"#);
        // 用户插件
        make_manifest(&user.join("user_hello"), r#"{
            "name": "用户问候", "main": "index.js",
            "skills": [{ "name": "yo", "description": "x", "parameters": [], "action": "js:yo" }],
            "permissions": [], "hermes_compatible": false
        }"#);

        let mut mgr = PluginManager::new_with_dirs(user.clone(), builtin, root.join("hermes"));
        assert_eq!(mgr.plugins.len(), 2);
        // 内置默认启用 + 预授权 file.read；用户默认禁用
        let builtin_p = mgr.get("builtin_hello").unwrap();
        assert!(builtin_p.enabled);
        assert_eq!(
            mgr.registry["builtin_hello"].granted,
            vec!["file.read".to_string()]
        );
        let user_p = mgr.get("user_hello").unwrap();
        assert!(!user_p.enabled);

        // 注册表已持久化 → 重新加载状态保持
        let mgr2 = PluginManager::new_with_dirs(user, root.join("builtin"), root.join("hermes"));
        assert!(mgr2.get("builtin_hello").unwrap().enabled);
        assert!(!mgr2.get("user_hello").unwrap().enabled);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn 启用禁用与权限变更() {
        let root = test_plugin_dir("toggle");
        let user = root.join("plugins");
        fs::create_dir_all(&user).unwrap();
        make_manifest(&user.join("p1"), r#"{
            "name": "P1", "main": "index.js",
            "skills": [{ "name": "s1", "description": "x", "parameters": [], "action": "js:s1" }],
            "permissions": ["network"], "hermes_compatible": false
        }"#);
        let mut mgr = PluginManager::new_with_dirs(user, root.join("b"), root.join("h"));

        mgr.set_enabled("p1", true).unwrap();
        assert!(mgr.get("p1").unwrap().enabled);

        mgr.set_permission("p1", "network", true).unwrap();
        assert_eq!(mgr.registry["p1"].granted, vec!["network".to_string()]);
        mgr.set_permission("p1", "network", false).unwrap();
        assert!(mgr.registry["p1"].granted.is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn 卸载用户插件但内置不可卸载() {
        let root = test_plugin_dir("uninstall");
        let user = root.join("plugins");
        let builtin = root.join("builtin");
        fs::create_dir_all(&user).unwrap();
        fs::create_dir_all(&builtin).unwrap();
        make_manifest(&builtin.join("b1"), r#"{
            "name": "B1", "main": "index.js",
            "skills": [{ "name": "sb", "description": "x", "parameters": [], "action": "js:sb" }],
            "permissions": [], "hermes_compatible": false
        }"#);
        make_manifest(&user.join("u1"), r#"{
            "name": "U1", "main": "index.js",
            "skills": [{ "name": "su", "description": "x", "parameters": [], "action": "js:su" }],
            "permissions": [], "hermes_compatible": false
        }"#);
        let mut mgr = PluginManager::new_with_dirs(user, builtin, root.join("h"));

        assert!(mgr.uninstall("b1").is_err()); // 内置不可卸载
        mgr.uninstall("u1").unwrap();
        assert!(mgr.get("u1").is_err());
        assert!(!mgr.registry.contains_key("u1"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn 终端命令注册() {
        let root = test_plugin_dir("term");
        let user = root.join("plugins");
        fs::create_dir_all(&user).unwrap();
        let mut mgr = PluginManager::new_with_dirs(user, root.join("b"), root.join("h"));

        let p = mgr.add_terminal_command("clean_temp", "del /q %TEMP%\\*", "清理临时文件").unwrap();
        assert_eq!(p.manifest.skills[0].action, "command:del /q %TEMP%\\*");
        assert_eq!(p.manifest.permissions, vec!["system".to_string()]);
        // 重名拒绝
        assert!(mgr.add_terminal_command("clean_temp", "echo hi", "x").is_err());
        // 非法名字拒绝
        assert!(mgr.add_terminal_command("bad name!", "echo hi", "x").is_err());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn git_url校验_合法形态通过() {
        assert!(validate_git_url("https://github.com/user/repo.git").is_ok());
        assert!(validate_git_url("git@github.com:user/repo.git").is_ok());
    }

    #[test]
    fn git_url校验_危险形态被拒绝() {
        // ext:: 协议可让 git 直接执行本地命令
        assert!(validate_git_url("ext::calc.exe%20arg").is_err());
        // file:// / ssh:// / http:// 一律不放行
        assert!(validate_git_url("file:///C:/evil").is_err());
        assert!(validate_git_url("http://evil.com/x.git").is_err());
        // 以 - 开头会被 git 解析为选项
        assert!(validate_git_url("--upload-pack=calc.exe").is_err());
        // 换行
        assert!(validate_git_url("https://a.com/x.git\nrm -rf /").is_err());
        assert!(validate_git_url("").is_err());
    }

    #[test]
    fn 仓库名校验_目录遍历被拦截() {
        assert_eq!(sanitize_repo_name("my-plugin_1").unwrap(), "my-plugin_1");
        // 目录遍历
        assert!(sanitize_repo_name("..").is_err());
        assert!(sanitize_repo_name(".").is_err());
        assert!(sanitize_repo_name("../../evil").is_err());
        assert!(sanitize_repo_name("a/b").is_err());
        assert!(sanitize_repo_name("a\\b").is_err());
        // 空名
        assert!(sanitize_repo_name("").is_err());
        // 带点的名字（可能是 .git / 隐藏目录）
        assert!(sanitize_repo_name("plugin.js").is_err());
    }

    #[test]
    fn 仓库名派生_正常与遍历用例() {
        assert_eq!(
            derive_repo_name("https://github.com/user/my_plugin.git").unwrap(),
            "my_plugin"
        );
        assert_eq!(
            derive_repo_name("https://github.com/user/my_plugin/").unwrap(),
            "my_plugin"
        );
        // git@host:user/repo.git 形态（rsplit 同时按 ':' 切，避免整段 host:user/repo 混入）
        assert_eq!(
            derive_repo_name("git@github.com:user/repo.git").unwrap(),
            "repo"
        );
        // URL 末段为 .. → 派生出的名字非法，拒绝（旧实现会写到 user_dir 上一级）
        assert!(derive_repo_name("https://x.com/a/../").is_err());
    }

    #[test]
    fn 安装本地路径插件() {
        let root = test_plugin_dir("install");
        let user = root.join("plugins");
        let src = root.join("src_plugin");
        fs::create_dir_all(&user).unwrap();
        make_manifest(&src, r#"{
            "name": "源插件", "main": "index.js",
            "skills": [{ "name": "hello", "description": "x", "parameters": [], "action": "js:hello" }],
            "permissions": ["file.read"], "hermes_compatible": false
        }"#);
        let mut mgr = PluginManager::new_with_dirs(user, root.join("b"), root.join("h"));

        let p = mgr.install_from_path(src.to_str().unwrap()).unwrap();
        assert_eq!(p.id, "src_plugin");
        assert!(!p.enabled);
        // 重复安装拒绝
        assert!(mgr.install_from_path(src.to_str().unwrap()).is_err());

        let _ = fs::remove_dir_all(&root);
    }
}
