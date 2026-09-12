// 《铃·记忆体》全局类型定义（Rust 端）
// 与前端 src/types/index.ts 严格对应，serde 派生保证 IPC 序列化一致
use serde::{Deserialize, Serialize};

/// 单条对话消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: String, // "user" 或 "assistant"
    pub content: String,
    pub timestamp: String,
}

/// 一条记忆
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub summary: Option<String>,
    /// 记忆分类（记忆中心：兴趣/游戏/工作/健康/家庭/日常对话等，空=未分类）
    #[serde(default)]
    pub category: Option<String>,
    /// 使用次数（被上下文引用的次数，记忆中心展示"铃常想起")
    #[serde(default)]
    pub use_count: u32,
}

// ==================== 多会话管理（收尾工程师批次3） ====================

/// 会话元信息（多会话列表展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
}

/// 一个会话（元信息 + 完整消息流）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub meta: SessionMeta,
    pub messages: Vec<Message>,
}

/// 应用设置（对应前端 Setting）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setting {
    pub theme: String,              // "light" 或 "dark"
    pub context_length: u8,         // 默认 10
    #[serde(default)]
    pub api_base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    /// API 模型名（默认 gpt-3.5-turbo，可配置 deepseek-chat 等）
    #[serde(default = "default_api_model")]
    pub api_model: String,
    pub model_mode: String,         // "script" | "api" | "local"
    pub depth: u8,                  // 1 | 2 | 3 | 4
    /// AI 自称（回复模板占位替换，默认「铃」；前端未传时为空则用默认）
    #[serde(default)]
    pub self_name: Option<String>,
    /// 对用户的称呼（回复模板占位替换，默认「主人」）
    #[serde(default)]
    pub user_name: Option<String>,
    /// 形象人格：daily 日常 / chuunibyou 中二 / healing 治愈 / lewd 涩涩
    #[serde(default = "default_persona")]
    pub persona: String,
}

impl Default for Setting {
    fn default() -> Self {
        Setting {
            theme: "dark".to_string(),
            context_length: 10,
            api_base_url: None,
            api_key: None,
            api_model: "gpt-3.5-turbo".to_string(),
            model_mode: "script".to_string(),
            depth: 2,
            self_name: None,
            user_name: None,
            persona: "daily".to_string(),
        }
}
}

/// 发送消息请求（启动流式对话）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
    #[serde(default = "default_depth")]
    pub depth: u8,
}

fn default_depth() -> u8 {
    2
}

/// 发送消息响应（仅返回初始状态，实际内容通过事件推送）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub message_id: String, // 本次回复的消息 ID
    pub stream_id: String,  // 流 ID，用于标识本次对话
}

/// 测试 API 连接请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConnectionRequest {
    pub base_url: String,
    pub api_key: String,
}

/// 测试 API 连接响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConnectionResponse {
    pub success: bool,
    pub message: String,
}

/// Token 用量（API 模式流式/非流式统计，收尾工程师新增）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ==================== AI-4 记忆系统（与 AI-3 契约对齐，index.json 存 Memory[]） ====================

/// 获取记忆列表的请求（limit 分页、keyword 搜索、set_name 指定记忆集）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMemoriesRequest {
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default)]
    pub set_name: Option<String>,
}

/// 获取记忆列表的响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMemoriesResponse {
    pub memories: Vec<Memory>,
    pub total: usize,
}

/// 删除单条记忆的请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteMemoryRequest {
    pub memory_id: String,
    #[serde(default)]
    pub set_name: Option<String>,
}

/// 删除记忆的响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteMemoryResponse {
    pub success: bool,
    pub message: String,
}

/// 切换记忆集的请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchMemorySetRequest {
    pub set_name: String,
}

/// 创建新记忆集的请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMemorySetRequest {
    pub set_name: String,
}

/// 写入单条记忆的请求（由对话引擎/前端调用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteMemoryRequest {
    pub memory: Memory,
    #[serde(default)]
    pub set_name: Option<String>,
}

// ==================== AI-5 插件系统（扩展生态） ====================

/// 一个已注册的插件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub enabled: bool,
    pub path: String, // 插件所在目录
    pub manifest: PluginManifest,
    /// 用户实际授予的权限列表（从注册表读取，AI-5 扩展字段；不写入 manifest.json）
    #[serde(default)]
    pub granted: Vec<String>,
}

/// 插件清单（manifest.json / Hermes config.json）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub main: String,               // 入口文件（如 index.js；终端命令插件为空串）
    pub skills: Vec<Skill>,         // 注册的技能列表
    pub permissions: Vec<String>,   // 所需权限（如 "file.read", "network"）
    pub hermes_compatible: bool,    // 是否兼容 Hermes 格式
}

/// 插件注册的一个技能（可通过自然语言触发）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,               // 技能名称，如 "file_search"
    pub description: String,        // 技能描述，用于自然语言触发
    pub parameters: Vec<SkillParam>,
    /// 执行动作：
    /// - `js:<name>` / 无前缀带 main → JS 引擎执行
    /// - `command:<命令>` → 系统命令（终端命令扩展，需 system 权限）
    /// - `builtin:<动作>` → 内置 Rust 动作
    pub action: String,
}

/// 技能参数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillParam {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,              // "string", "number", "boolean", "file"
    pub required: bool,
    pub description: String,
}

/// 列出所有插件（响应）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPluginsResponse {
    pub plugins: Vec<Plugin>,
}

/// 安装插件请求（本地路径或 Git URL）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPluginRequest {
    pub source: String,
}

/// 卸载插件请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallPluginRequest {
    pub plugin_id: String,
}

/// 启用/禁用插件请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TogglePluginRequest {
    pub plugin_id: String,
}

/// 执行插件技能请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteSkillRequest {
    pub skill_name: String,
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
}

/// 执行插件技能响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteSkillResponse {
    pub success: bool,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// 权限粒度控制请求（类似 Android 权限）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetPermissionRequest {
    pub plugin_id: String,
    pub permission: String,
    pub allow: bool,
}

/// 添加自定义终端命令请求（任务 9）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddTerminalCommandRequest {
    pub name: String,          // 命令名（如 clean_temp）
    pub command: String,       // 实际执行的命令（如 del /q %TEMP%\*）
    pub description: String,
}

use std::collections::HashMap;

// ==================== AI-6 桌面交互与系统集成 ====================
// 与任务书二、前端 src/types/index.ts 严格对应

/// 当前前台窗口信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub app_name: String,      // 可执行文件名，如 "chrome.exe"
    pub window_title: String,  // 窗口标题
    pub is_fullscreen: bool,
    pub is_foreground: bool,
}

/// 屏幕监测规则（支持 app_name 通配符 *）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenMonitorRule {
    pub id: String,
    pub app_name: String,           // 匹配的应用名（支持通配符 *）
    pub trigger_reply: String,      // 触发时的回复内容
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub cooldown_seconds: u32,      // 冷却时间（秒），防止频繁弹窗
}

fn default_true() -> bool {
    true
}

fn default_floating_ball_size() -> u32 {
    200
}

fn default_floating_ball_opacity() -> f32 {
    1.0
}

/// 工具箱条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolboxItem {
    pub id: String,
    pub name: String,
    pub icon: String,               // 图标名称或路径（emoji 或名称）
    pub command: String,            // 要执行的终端命令
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 是否需要输入参数（点击先弹输入框）
    #[serde(default)]
    pub needs_input: bool,
    /// 输入框标签提示
    #[serde(default)]
    pub input_label: Option<String>,
    /// 输入框占位符
    #[serde(default)]
    pub input_placeholder: Option<String>,
    /// 组合工具（模块化，v0.6 新增）：非空时按顺序执行各步骤（模仿快捷指令）；
    /// 空 = 传统单命令工具（向后兼容旧数据）
    #[serde(default)]
    pub steps: Vec<QuickCommandStep>,
    /// Agent 权限分类（仅 Agent 专用工具使用；None = 无需特殊授权，默认可用）
    /// 取值：`download` / `software` / `file_write` / `shell`
    /// 未授权的分类不会出现在 Agent 的工具列表里（前端设置页可开关）
    #[serde(default)]
    pub agent_permission: Option<String>,
}

/// 获取前台窗口信息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetWindowInfoResponse {
    pub info: WindowInfo,
}

/// 获取屏幕监测规则列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMonitorRulesResponse {
    pub rules: Vec<ScreenMonitorRule>,
    pub enabled: bool,
    pub interval_seconds: u32,
    pub available: bool, // 屏幕监测是否可用（三层兜底：完全失败时为 false）
}

/// 更新规则请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMonitorRuleRequest {
    pub rule: ScreenMonitorRule,
}

/// 删除规则请求（前端 MonitorSettings 删除按钮）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteMonitorRuleRequest {
    pub rule_id: String,
}

/// 监测开关/频率请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetMonitoringRequest {
    pub enabled: bool,
    #[serde(default)]
    pub interval_seconds: Option<u32>,
}

/// 执行工具箱命令请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteToolboxRequest {
    pub item_id: String,
    /// 需要输入参数的工具：用户输入值（对应命令里的 {input} 占位符）
    #[serde(default)]
    pub input: Option<String>,
    /// 危险操作二次确认（format-disk 必须为 true 才执行）
    #[serde(default)]
    pub confirm: bool,
}

/// 执行工具箱命令响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteToolboxResponse {
    pub success: bool,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// 列出工具箱条目响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolboxItemsResponse {
    pub items: Vec<ToolboxItem>,
}

/// 新增/更新工具箱条目请求（前端添加工具）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveToolboxItemRequest {
    pub item: ToolboxItem,
}

/// 删除工具箱条目请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteToolboxItemRequest {
    pub item_id: String,
}

/// 悬浮球可见性请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetFloatingBallVisibilityRequest {
    pub visible: bool,
}

/// 开机自启动请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetAutostartRequest {
    pub enabled: bool,
}

/// 开机自启动状态响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAutostartResponse {
    pub enabled: bool,
}

/// 快捷键注册请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterHotkeyRequest {
    pub accelerator: String, // 如 "Ctrl+Alt+L"
}

/// 快捷键注册响应（accelerator 为实际生效的组合，供前端回显）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterHotkeyResponse {
    pub registered: bool,
    pub accelerator: String,
}

/// 屏幕监测触发事件负载（后端 → 前端，气泡弹窗）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorTriggerEvent {
    pub app_name: String,
    pub window_title: String,
    pub reply: String,
    pub rule_id: String,
}

// ==================== AI-7 配置与诊断 ====================
// 统一配置模型（任务书二）。存储于 ~/.铃记忆体/config.json。
//
// ⚠️ 架构债务（主人拍板）：monitor_rules / toolbox_items 字段由 AI-6 所有，
// AI-7 不负责持久化（运行时数据仍由 AI-6 读写 %APPDATA%/ling-memoria/），
// 此处仅作为配置模型的完整性保留，未来统一入口预留，当前不写入实际数据。

/// 全局配置（与前端 useSettingStore 严格对应，snake_case）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 配置版本号（迁移用，当前 1）
    #[serde(default = "default_config_version")]
    pub config_version: u32,
    /// "light" | "dark"
    pub theme: String,
    /// 上下文长度（对话保留条数，默认 10）
    pub context_length: u8,
    /// 长期记忆注入条数（按相关性检索后注入，默认 5；设 0 则完全不注入长期记忆）
    #[serde(default = "default_long_term_limit")]
    pub long_term_memory_limit: u8,
    #[serde(default)]
    pub api_base_url: Option<String>,
    /// 加密存储的 API Key（AES-256-GCM，非明文）
    #[serde(default)]
    pub api_key_encrypted: Option<String>,
    /// 明文存储的 API Key（未设置主密码时使用；设置主密码后自动转为加密存储）
    #[serde(default)]
    pub api_key_plain: Option<String>,
    /// API 模型名（如 gpt-4o-mini / deepseek-chat），默认 gpt-3.5-turbo
    #[serde(default = "default_api_model")]
    pub api_model: String,
    /// 视觉（看图）专用模型名，如 glm-4v。
    /// 留空则回退到 api_model；填了就用它做「看图」。
    #[serde(default)]
    pub vision_model: Option<String>,
    /// "script" | "api" | "local"
    pub model_mode: String,
    /// 思考深度 1|2|3|4
    pub depth: u8,
    /// 日语修饰词浓度 0~30
    pub language_mix_rate: u8,
    /// 悬浮球模式 "avatar" | "simple" | "live2d"
    pub floating_ball_mode: String,
    /// 是否启用悬浮球
    #[serde(default = "default_true")]
    pub floating_ball_enabled: bool,
    /// 悬浮球大小（px，默认 200）
    #[serde(default = "default_floating_ball_size")]
    pub floating_ball_size: u32,
    /// 悬浮球透明度 0.0~1.0（默认 1.0）
    #[serde(default = "default_floating_ball_opacity")]
    pub floating_ball_opacity: f32,
    /// 是否开启呼吸动画（默认 true）
    #[serde(default = "default_true")]
    pub floating_ball_breathing: bool,
    /// 是否开启消息闪烁（默认 true）
    #[serde(default = "default_true")]
    pub floating_ball_flash: bool,
    /// 悬浮球位置 (x, y)
    #[serde(default)]
    pub floating_ball_position: (u32, u32),
    pub monitor_enabled: bool,
    /// 屏幕监测频率（秒）
    pub monitor_frequency: u32,
    /// 归属 AI-6，AI-7 不持久化（见顶部注释）
    #[serde(default)]
    pub monitor_rules: Vec<ScreenMonitorRule>,
    /// 归属 AI-6，AI-7 不持久化（见顶部注释）
    #[serde(default)]
    pub toolbox_items: Vec<ToolboxItem>,
    /// 全局快捷键，如 "Ctrl+Alt+L"
    pub hotkey: String,
    pub autostart: bool,
    /// 记忆存储路径
    pub data_path: String,
    /// 是否首次启动（引导完成后置 false）
    pub first_launch: bool,
    pub plugin_enabled: bool,
    #[serde(default)]
    pub enabled_plugins: Vec<String>,
    /// AI 自称（对话占位替换，默认「铃」）
    #[serde(default)]
    pub self_name: Option<String>,
    /// 对主人的称呼（对话占位替换，默认「主人」）
    #[serde(default)]
    pub user_name: Option<String>,
    /// 形象人格（daily 日常 / chuunibyou 中二 / healing 治愈 / lewd 涩涩）
    #[serde(default = "default_persona")]
    pub persona: String,
    /// 主密码派生密钥的盐（base64，非密钥材料，可安全落盘；用于重装后重新派生密钥）
    #[serde(default)]
    pub master_password_salt: Option<String>,
    /// 是否已设置过主密码（true 表示已设置；解锁状态 unlocked 是内存态，不落盘）
    #[serde(default)]
    pub has_master_password: bool,
    /// 密码校验段：对固定明文 "ling-check-v1" 的加密结果，用于 unlock 时验证密码正确性
    #[serde(default)]
    pub master_password_check: Option<String>,
    /// —— 外观自定义（用户可自行调整，均选填，大版本开放给用户自定义）——
    /// 主色 accent（十六进制，如 #ff7a94）
    #[serde(default)]
    pub accent_color: Option<String>,
    /// 危险/错误色 danger（十六进制，如 #ff453a；不设置则随主题/主色联动）
    #[serde(default)]
    pub danger_color: Option<String>,
    /// 主界面背景色（十六进制）
    #[serde(default)]
    pub bg_color: Option<String>,
    /// 背景图本地路径（可选，优先于背景色）
    #[serde(default)]
    pub bg_image: Option<String>,
    /// 铃的头像（emoji / 文字 / 图片路径）
    #[serde(default)]
    pub avatar_suzu: Option<String>,
    /// 用户头像（emoji / 文字 / 图片路径）
    #[serde(default)]
    pub avatar_user: Option<String>,
    /// UI 圆角（px，0=无圆角）
    #[serde(default)]
    pub ui_radius: Option<u8>,
    /// 气泡-用户 背景色（十六进制）
    #[serde(default)]
    pub bubble_user_color: Option<String>,
    /// 气泡-铃 背景色（十六进制或渐变）
    #[serde(default)]
    pub bubble_suzu_color: Option<String>,
    /// 自定义主题组合（多套，用户可保存/切换，大版本核心功能）
    #[serde(default)]
    pub ui_themes: Option<Vec<UiThemePreset>>,
    /// 是否始终以管理员身份运行（用户自选；开启后启动时自动提权，需弹 UAC）
    #[serde(default)]
    pub run_as_admin: bool,
    /// Emoji 显示模式："off"(默认关闭)/"partial"(局部)/"all"(全部开启)
    #[serde(default)]
    pub emoji_mode: Option<String>,
    /// 是否允许 AI（铃）调用工具箱工具（默认关闭；开启后消息匹配工具意图时自动执行）
    #[serde(default)]
    pub ai_toolbox: bool,
    /// —— AI-9 快捷指令（用户自定义组合指令，如「晚安模式」）——
    #[serde(default)]
    pub quick_commands: Vec<QuickCommand>,
    /// 搜索引擎模式："bigram"（默认）/ "bm25" / "vector"
    #[serde(default = "default_search_mode")]
    pub search_mode: String,
    /// —— Agent 权限开关（默认全部关闭，需用户在设置页显式开启）——
    /// 允许 Agent 下载文件
    #[serde(default)]
    pub agent_allow_download: bool,
    /// 允许 Agent 安装 / 卸载软件
    #[serde(default)]
    pub agent_allow_software: bool,
    /// 允许 Agent 写入 / 删除文件
    #[serde(default)]
    pub agent_allow_file_write: bool,
    /// 允许 Agent 执行任意命令（shell，最高危：能改系统、装东西、删数据）
    #[serde(default)]
    pub agent_allow_shell: bool,
}

fn default_search_mode() -> String {
    "bigram".to_string()
}

/// 一套完整的外观自定义组合（用户命名保存，可一键切换）
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct UiThemePreset {
    pub name: String,
    #[serde(default)]
    pub accent_color: String,
    #[serde(default)]
    pub danger_color: Option<String>,
    #[serde(default)]
    pub bg_color: String,
    #[serde(default)]
    pub bg_image: Option<String>,
    #[serde(default)]
    pub bubble_user_color: String,
    #[serde(default)]
    pub bubble_suzu_color: String,
    #[serde(default)]
    pub ui_radius: u8,
    #[serde(default)]
    pub avatar_suzu: Option<String>,
}

fn default_config_version() -> u32 {
    1
}

/// 默认 API 模型名
fn default_api_model() -> String {
    "gpt-3.5-turbo".to_string()
}

/// 默认长期记忆注入条数（按相关性检索后注入这么多条）
fn default_long_term_limit() -> u8 {
    5
}

/// 默认形象人格（日常）
fn default_persona() -> String {
    "daily".to_string()
}

impl AppConfig {
    /// 生成脱敏配置（发往前端的唯一形态）
    ///
    /// 去掉两个敏感字段，替换为布尔标记：
    /// - `api_key_plain`（明文 Key）→ `has_plain_key`
    /// - `api_key_encrypted`（AES-256-GCM 密文，可离线爆破弱主密码）→ 与明文一起归并到 `has_api_key`
    ///
    /// 用 serde_json::Value 而非另建一份结构体：AppConfig 有 50+ 字段且持续增长，
    /// 手抄副本必然与主结构漂移，脱敏字段反而可能漏掉。
    pub fn to_public(&self) -> Result<serde_json::Value, serde_json::Error> {
        let has_plain_key = self.api_key_plain.is_some();
        let has_api_key = has_plain_key || self.api_key_encrypted.is_some();
        let mut v = serde_json::to_value(self)?;
        if let Some(obj) = v.as_object_mut() {
            obj.remove("api_key_plain");
            obj.remove("api_key_encrypted");
            obj.insert("has_api_key".into(), serde_json::Value::Bool(has_api_key));
            obj.insert("has_plain_key".into(), serde_json::Value::Bool(has_plain_key));
        }
        Ok(v)
    }
}

/// 获取配置响应
///
/// `config` 是 AppConfig 的**脱敏投影**（见 `AppConfig::to_public`），不含 API Key
/// 明文或密文。前端对应类型见 src/types/index.ts 的 `AppConfig`（含 has_api_key /
/// has_plain_key，无 api_key_* 字段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetConfigResponse {
    pub config: serde_json::Value,
}

impl GetConfigResponse {
    /// 从完整配置构造脱敏响应
    pub fn from_config(cfg: &AppConfig) -> Result<Self, crate::error::AppError> {
        Ok(Self {
            config: cfg.to_public().map_err(|e| {
                crate::error::AppError::InternalError(format!("配置脱敏序列化失败：{e}"))
            })?,
        })
    }
}

/// 更新配置请求（增量：仅更新传入的字段，null 表示置 None/删除）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfigRequest {
    pub updates: std::collections::HashMap<String, serde_json::Value>,
}

/// 导出配置响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfigResponse {
    pub success: bool,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// 导入配置请求（path 为 JSON 文件路径）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportConfigRequest {
    pub path: String,
}

/// 日志级别（IPC 序列化为小写字符串：trace/debug/info/warn/error）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// 返回日志等级对应的优先级数值（用于过滤：Trace=0 ... Error=4）
    pub fn order(&self) -> u8 {
        match self {
            LogLevel::Trace => 0,
            LogLevel::Debug => 1,
            LogLevel::Info => 2,
            LogLevel::Warn => 3,
            LogLevel::Error => 4,
        }
    }
}

/// 获取日志请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetLogsRequest {
    /// 返回条数上限（默认 100）
    #[serde(default = "default_log_limit")]
    pub limit: usize,
    /// 级别过滤（只返回 >= 该级别）
    #[serde(default)]
    pub level: Option<LogLevel>,
    /// 关键词搜索（匹配消息内容）
    #[serde(default)]
    pub keyword: Option<String>,
    /// 分页偏移（默认 0）
    #[serde(default)]
    pub offset: usize,
}

fn default_log_limit() -> usize {
    100
}

/// 获取日志响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetLogsResponse {
    pub logs: Vec<String>,
    pub total: usize,
}

/// 导出诊断包请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportDiagnosticRequest {
    pub include_logs: bool,
    pub include_config: bool,
    pub include_system_info: bool,
}

/// 导出诊断包响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportDiagnosticResponse {
    pub success: bool,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// 系统信息（sysinfo 采集，诊断面板展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub cpu_name: String,
    pub cpu_cores: usize,
    pub cpu_usage: f32,
    pub memory_total_mb: u64,
    pub memory_used_mb: u64,
    pub os_name: String,
    pub os_version: String,
    pub app_version: String,
    pub disks: Vec<DiskInfo>,
}

/// 单个磁盘分区信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub name: String,
    pub total_gb: u64,
    pub available_gb: u64,
}

/// 系统信息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfoResponse {
    pub info: SystemInfo,
}

/// 设置主密码请求（首次引导 / 修改主密码）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetMasterPasswordRequest {
    pub password: String,
}

/// 解锁请求（重启后输入主密码解锁 API Key）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlockRequest {
    pub password: String,
}

/// 主密码状态响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterPasswordStatus {
    /// 是否已设置过主密码
    pub has_master_password: bool,
    /// 当前是否已解锁（内存中持有派生密钥）
    pub unlocked: bool,
}

// ==================== AI-8 网络与同步 ====================
// 与任务书二、类型定义严格对应；serde snake_case 与前端 src/types/index.ts 对应。

/// 局域网内的一台「铃」设备
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDevice {
    /// 设备唯一标识（首次启动生成）
    pub device_id: String,
    /// 设备名称（如 "主人-PC"）
    pub device_name: String,
    pub ip: String,
    pub port: u16,
    /// 最后发现时间（ISO 8601）
    pub last_seen: String,
    /// 来源：udp 自动发现 / manual 手动添加
    #[serde(default = "default_source_udp")]
    pub source: String,
}

fn default_source_udp() -> String {
    "udp".to_string()
}

/// 记忆元数据（用于增量同步比对 / 冲突检测）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMeta {
    pub id: String,
    pub timestamp: String,
}

/// 同步数据包（TCP 传输，记忆内容已加密）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPayload {
    pub device_id: String,
    pub set_name: String,
    /// 加密后的记忆 JSON（base64），明文为 Memory[]
    pub encrypted_data: String,
    /// 数据校验和（SHA-256，对加密前的明文计算）
    pub checksum: String,
    /// 是否增量同步（true 时只含 last_sync_time 之后的记忆）
    pub incremental: bool,
}

/// 同步请求（客户端 → 服务端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequest {
    pub device_id: String,
    pub set_name: String,
    /// 增量同步时间戳（None 为全量）
    #[serde(default)]
    pub last_sync_time: Option<String>,
    /// 配对确认码：用派生密钥加密固定串 "ling-sync-auth-v1"，服务端解密验证双端密码一致
    #[serde(default)]
    pub pairing_code: Option<String>,
}

/// 同步响应（服务端 → 客户端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    pub success: bool,
    pub message: String,
    pub synced_count: usize,
    #[serde(default)]
    pub conflict_resolved: bool,
}

/// 版本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
    pub release_notes: String,
    pub is_outdated: bool,
}

/// 网络状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkStatus {
    Online,
    Offline,
    Unknown,
}

// ==================== AI-8 IPC 命令参数与返回结构 ====================

/// 发现局域网设备（响应）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverDevicesResponse {
    pub devices: Vec<SyncDevice>,
}

/// 启动同步（请求）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartSyncRequest {
    /// 目标设备 ID
    pub target_device: String,
    /// 要同步的记忆集
    pub set_name: String,
    /// 备选手动 IP
    #[serde(default)]
    pub manual_ip: Option<String>,
    /// 备选手动端口
    #[serde(default)]
    pub manual_port: Option<u16>,
}

/// 启动同步（响应）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartSyncResponse {
    pub success: bool,
    pub message: String,
    pub synced_count: usize,
}

/// 检查更新（响应）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckUpdateResponse {
    pub has_update: bool,
    #[serde(default)]
    pub version_info: Option<VersionInfo>,
    #[serde(default)]
    pub error: Option<String>,
}

/// 获取网络状态（响应）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNetworkStatusResponse {
    pub status: NetworkStatus,
}

/// 同步进度事件（后端 → 前端，传输中每 100 条推送一次）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgressEvent {
    pub current: usize,
    pub total: usize,
    pub phase: String, // "send" | "receive" | "done"
}

/// 同步状态（get_sync_status 返回）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    /// idle / discovering / syncing / done / error
    pub status: String,
    pub progress: f32,
    #[serde(default)]
    pub message: Option<String>,
    /// 最近同步历史（时间 + 设备 + 结果）
    pub history: Vec<SyncHistoryEntry>,
}

/// 一条同步历史记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncHistoryEntry {
    pub time: String,
    pub device: String,
    pub set_name: String,
    pub success: bool,
    pub message: String,
    pub synced_count: usize,
}

/// 设置同步主密码请求（与 AI-7 主密码共享同一体系）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetSyncPasswordRequest {
    pub password: String,
}

/// 同步冲突策略（用户偏好，落盘 sync_config.json）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictPolicy {
    /// 保留时间戳较新的版本（默认）
    #[default]
    Newest,
    /// 始终保留本地
    Local,
    /// 始终保留远程
    Remote,
}

// ==================== AI-9 快捷指令系统 ====================
// 一条快捷指令按顺序执行一串动作（如设电源→清临时文件→音量→音乐→铃说晚安）。
// 持久化于 config.json（走 config::store 增量 merge），与前端 src/types/index.ts 对应。

/// 快捷指令中的单步动作
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuickCommandStep {
    /// 动作标识：可为系统工具（volume / music / power-balanced / power-high / power-saver）
    /// 或任意工具箱工具 id（如 clean-temp，复用工具箱执行逻辑）
    pub tool: String,
    /// 该步的可选输入（如音量数值 0-100 / 音乐文件路径）
    #[serde(default)]
    pub input: Option<String>,
}

/// 一条快捷指令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickCommand {
    pub id: String,
    /// 指令名，如「晚安模式」——聊天里说该名字即可触发
    pub name: String,
    /// 按顺序执行的动作列表
    pub steps: Vec<QuickCommandStep>,
    /// 全部动作执行完后铃要说的话（可选）
    #[serde(default)]
    pub say: Option<String>,
}

/// 列出所有快捷指令（响应）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListQuickCommandsResponse {
    pub commands: Vec<QuickCommand>,
}

/// 新增/更新一条快捷指令（请求）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveQuickCommandRequest {
    pub command: QuickCommand,
}

/// 删除一条快捷指令（请求）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteQuickCommandRequest {
    pub command_id: String,
}

/// 执行一条快捷指令（请求）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteQuickCommandRequest {
    pub command_id: String,
}

/// 执行一条快捷指令（响应）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteQuickCommandResponse {
    pub success: bool,
    /// 每一步的执行结果说明（与 steps 一一对应）
    pub results: Vec<String>,
    /// 全部执行完后铃要说的话
    #[serde(default)]
    pub say: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一份带 Key 的配置用于脱敏断言
    fn cfg_with_keys(plain: Option<&str>, enc: Option<&str>) -> AppConfig {
        let mut c = crate::config::defaults::default_config();
        c.api_key_plain = plain.map(|s| s.to_string());
        c.api_key_encrypted = enc.map(|s| s.to_string());
        c
    }

    #[test]
    fn 配置脱敏_明文与密文都不出现在响应里() {
        let c = cfg_with_keys(Some("sk-super-secret-plain"), Some("ENCRYPTED_BLOB_BASE64"));
        let v = c.to_public().unwrap();
        let obj = v.as_object().unwrap();

        // 两个敏感字段被移除
        assert!(!obj.contains_key("api_key_plain"));
        assert!(!obj.contains_key("api_key_encrypted"));
        // 整个 JSON 文本里不含 Key 内容
        let text = serde_json::to_string(&v).unwrap();
        assert!(!text.contains("sk-super-secret-plain"));
        assert!(!text.contains("ENCRYPTED_BLOB_BASE64"));

        // 布尔标记正确
        assert_eq!(obj["has_api_key"], serde_json::Value::Bool(true));
        assert_eq!(obj["has_plain_key"], serde_json::Value::Bool(true));
    }

    #[test]
    fn 配置脱敏_布尔标记各种组合() {
        // 仅密文：有 Key，但不是明文存储
        let v = cfg_with_keys(None, Some("enc")).to_public().unwrap();
        assert_eq!(v["has_api_key"], serde_json::Value::Bool(true));
        assert_eq!(v["has_plain_key"], serde_json::Value::Bool(false));

        // 仅明文
        let v = cfg_with_keys(Some("sk-x"), None).to_public().unwrap();
        assert_eq!(v["has_api_key"], serde_json::Value::Bool(true));
        assert_eq!(v["has_plain_key"], serde_json::Value::Bool(true));

        // 都没有
        let v = cfg_with_keys(None, None).to_public().unwrap();
        assert_eq!(v["has_api_key"], serde_json::Value::Bool(false));
        assert_eq!(v["has_plain_key"], serde_json::Value::Bool(false));
    }

    #[test]
    fn 配置脱敏_非敏感字段完整保留() {
        let mut c = cfg_with_keys(Some("sk-x"), None);
        c.theme = "dark".into();
        c.api_model = "gpt-4o-mini".into();
        c.context_length = 20;
        let v = c.to_public().unwrap();

        assert_eq!(v["theme"], "dark");
        assert_eq!(v["api_model"], "gpt-4o-mini");
        assert_eq!(v["context_length"], 20);
        // api_base_url 之类的非敏感字段仍在（前端要用）
        assert!(v.as_object().unwrap().contains_key("api_base_url"));
    }

    #[test]
    fn 配置响应_from_config走脱敏路径() {
        let c = cfg_with_keys(Some("sk-leak-me"), None);
        let resp = GetConfigResponse::from_config(&c).unwrap();
        let text = serde_json::to_string(&resp).unwrap();
        assert!(!text.contains("sk-leak-me"));
        assert!(text.contains("has_plain_key"));
    }
}
