// 《铃·记忆体》全局类型定义
// 任务书 3.1 要求：这些类型供所有 Store 与后续 AI 共享

/** 单条对话消息 */
export interface Message {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: string
}

/** 一条记忆 */
export interface Memory {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: string
  tags?: string[]
  summary?: string
  /** 记忆分类（记忆中心） */
  category?: string | null
  /** 使用次数（被上下文引用次数） */
  use_count?: number
}

/**
 * 运行模式（v1.0 离线智能版）
 * - local_0b / local_1b：内置 Qwen2.5 GGUF（0.5B / 1.5B），真离线对话，无需 Ollama
 * - api：OpenAI 兼容云端兜底
 * - script：内置离线文库模板，模型缺失时后端自动降级用它（UI 不再作为可选项）
 */
export type ModelMode = 'local_0b' | 'local_1b' | 'api' | 'script'

/** 应用设置 */
export interface Setting {
  theme: 'light' | 'dark'
  contextLength: number // 默认 10
  apiBaseUrl?: string
  apiKey?: string
  modelMode: ModelMode // 默认 'local_0b'
  depth: 1 | 2 | 3 | 4 // 默认 2
}

/** 模型模式中文名映射（可选，便于 UI 展示）——人话版（D 批次文案统一） */
export const MODEL_MODE_LABEL: Record<ModelMode, string> = {
  local_0b: '内置 0.5B',
  local_1b: '内置 1.5B',
  api: '云端AI',
  script: '离线文库',
}

// ==================== AI-4 记忆系统（与 Rust 端契约对齐） ====================

/** 获取记忆列表的请求 */
export interface GetMemoriesRequest {
  limit?: number
  keyword?: string
  set_name?: string
}

/** 获取记忆列表的响应 */
export interface GetMemoriesResponse {
  memories: Memory[]
  total: number
}

/** 删除记忆的响应 */
export interface DeleteMemoryResponse {
  success: boolean
  message: string
}

// ==================== AI-5 插件系统（与 Rust 端契约对齐） ====================

/** 技能参数定义 */
export interface SkillParam {
  name: string
  type: string // "string" | "number" | "boolean" | "file"
  required: boolean
  description: string
}

/** 插件注册的技能 */
export interface Skill {
  name: string
  description: string
  parameters: SkillParam[]
  /** js:<name> | command:<命令> | builtin:<动作> */
  action: string
}

/** 插件清单 */
export interface PluginManifest {
  main: string
  skills: Skill[]
  permissions: string[]
  hermes_compatible: boolean
}

/** 一个已安装的插件 */
export interface Plugin {
  id: string
  name: string
  version: string
  author: string
  description: string
  enabled: boolean
  path: string
  manifest: PluginManifest
  /** 用户实际授予的权限列表（后端注册表同步） */
  granted?: string[]
}

/** 列出插件响应 */
export interface ListPluginsResponse {
  plugins: Plugin[]
}

/** 执行技能请求/响应 */
export interface ExecuteSkillRequest {
  skill_name: string
  params: Record<string, unknown>
}
export interface ExecuteSkillResponse {
  success: boolean
  result?: string
  error?: string
}

/** 权限中文说明（与 Rust 端 ALL_PERMISSIONS 对应） */
export const PERMISSION_LABELS: Record<string, string> = {
  'file.read': '读取文件',
  'file.write': '写入文件',
  network: '网络请求',
  browser: '浏览器控制',
  clipboard: '剪贴板读写',
  system: '系统命令（高风险，默认禁用）',
  advanced: '高级 JS 能力（预留）',
}

// ==================== AI-6 桌面交互（与 Rust 端契约对齐） ====================

/** 当前前台窗口信息 */
export interface WindowInfo {
  app_name: string
  window_title: string
  is_fullscreen: boolean
  is_foreground: boolean
}

/** 屏幕监测规则 */
export interface ScreenMonitorRule {
  id: string
  app_name: string // 支持通配符 *
  trigger_reply: string
  enabled: boolean
  cooldown_seconds: number
}

/** 工具箱条目 */
export interface ToolboxItem {
  id: string
  name: string
  icon: string // emoji 或名称
  command: string
  enabled: boolean
  needs_input?: boolean
  input_label?: string | null
  input_placeholder?: string | null
  /** 声明的依赖 id 列表（如 ["ffmpeg"]），由统一 DependencyManager 检查 */
  dependencies?: string[]
}

/** 获取监测状态 + 规则列表响应 */
export interface GetMonitorRulesResponse {
  rules: ScreenMonitorRule[]
  enabled: boolean
  interval_seconds: number
  available: boolean
}

// ==================== AI-9 快捷指令系统 ====================

/** 快捷指令中的单步动作 */
export interface QuickCommandStep {
  /** 动作标识：volume / music / power-balanced / power-high / power-saver 或工具箱工具 id */
  tool: string
  /** 该步可选输入（音量数值 / 音乐文件路径） */
  input?: string | null
}

/** 一条快捷指令 */
export interface QuickCommand {
  id: string
  /** 指令名，如「晚安模式」 */
  name: string
  steps: QuickCommandStep[]
  /** 执行完后铃说的话 */
  say?: string | null
}

/** 列出快捷指令响应 */
export interface ListQuickCommandsResponse {
  commands: QuickCommand[]
}

/** 执行快捷指令响应 */
export interface ExecuteQuickCommandResponse {
  success: boolean
  results: string[]
  say?: string | null
  error?: string | null
}

/** 执行工具箱命令响应 */
export interface ExecuteToolboxResponse {
  success: boolean
  output?: string
  error?: string
}

/** 监测触发事件（气泡弹窗） */
export interface MonitorTriggerEvent {
  app_name: string
  window_title: string
  reply: string
  rule_id: string
}

// ==================== AI-7 配置与诊断（与 Rust 端契约对齐，snake_case） ====================

/** 全局配置（对应 Rust AppConfig）。monitor_rules/toolbox_items 归 AI-6 管理，AI-7 不持久化 */
export interface AppConfig {
  config_version: number
  theme: 'light' | 'dark' | string
  context_length: number
  api_base_url?: string | null
  /** 加密存储的 API Key（AES-256-GCM 密文，非明文） */
  api_key_encrypted?: string | null
  /** 明文存储的 API Key（未设置主密码时使用；设置后自动转为加密） */
  api_key_plain?: string | null
  /** API 模型名（如 gpt-4o-mini / deepseek-chat） */
  api_model?: string
  model_mode: ModelMode | string
  depth: number
  language_mix_rate: number
  floating_ball_mode: 'avatar' | 'simple' | string
  floating_ball_position: [number, number]
  monitor_enabled: boolean
  monitor_frequency: number
  monitor_rules: ScreenMonitorRule[]
  toolbox_items: ToolboxItem[]
  hotkey: string
  autostart: boolean
  data_path: string
  first_launch: boolean
  plugin_enabled: boolean
  enabled_plugins: string[]
  self_name?: string | null
  user_name?: string | null
  /** 形象人格（daily 日常 / chuunibyou 中二 / healing 治愈 / lewd 涩涩） */
  persona?: string
  master_password_salt?: string | null
  has_master_password: boolean
  /** 外观自定义（用户可自行调整，均选填） */
  accent_color?: string | null
  bg_color?: string | null
  bg_image?: string | null
  avatar_suzu?: string | null
  avatar_user?: string | null
  ui_radius?: number | null
  bubble_user_color?: string | null
  bubble_suzu_color?: string | null
  danger_color?: string | null
  ui_themes?: UiThemePreset[] | null
  run_as_admin?: boolean
  emoji_mode?: string | null
  ai_toolbox?: boolean
  /** —— AI-9 快捷指令列表 —— */
  quick_commands?: QuickCommand[]
}

/** 一套完整的外观自定义组合（用户命名保存，可一键切换） */
export interface UiThemePreset {
  name: string
  accent_color: string
  danger_color?: string | null
  bg_color: string
  bg_image?: string | null
  bubble_user_color: string
  bubble_suzu_color: string
  ui_radius: number
  avatar_suzu?: string | null
}

export interface GetConfigResponse {
  config: AppConfig
}

/** 测试 API 连接响应 */
export interface TestConnectionResponse {
  success: boolean
  message: string
}

/** Token 用量（API 模式流式统计，会话底部显示） */
export interface ChatUsage {
  prompt_tokens: number
  completion_tokens: number
  total_tokens: number
}

/** 单档内置 Qwen2.5 模型状态（v1.0 离线智能版） */
export interface LocalModelInfo {
  /** "0.5b" | "1.5b" */
  size: string
  /** 人话名（内置 0.5B / 内置 1.5B） */
  label: string
  /** GGUF 文件名 */
  file_name: string
  /** 是否已就位（可直接对话） */
  available: boolean
  /** 已就位时的绝对路径 */
  path?: string | null
  /** 文件大小（MB） */
  size_mb: number
  /** 未就位时的引导文案 */
  hint?: string | null
}

/** 两档内置模型的整体状态 */
export interface LocalModelStatus {
  models: LocalModelInfo[]
  /** 推荐放置目录 */
  models_dir: string
  /** 物理内存（MB），0 表示采集失败 */
  memory_total_mb: number
  /** 内存是否够跑 1.5B */
  can_run_1b: boolean
}

/** 显卡显存信息 */
export interface GpuVram {
  name: string
  vram_mb: number
}

// ==================== 多会话管理（收尾工程师批次3） ====================

/** 会话元信息（会话标签/历史列表展示） */
export interface SessionMeta {
  id: string
  title: string
  created_at: string
  updated_at: string
  message_count: number
}

/** 一个会话（元信息 + 完整消息流） */
export interface Session {
  meta: SessionMeta
  messages: Message[]
}

/** 更新配置请求（增量，null 表示清除字段） */
export interface UpdateConfigRequest {
  updates: Record<string, unknown>
}

export interface ExportConfigResponse {
  success: boolean
  path?: string
  error?: string
}

export interface ImportConfigRequest {
  path: string
}

export type LogLevel = 'trace' | 'debug' | 'info' | 'warn' | 'error'

export interface GetLogsRequest {
  limit?: number
  level?: LogLevel
  keyword?: string
  offset?: number
}

export interface GetLogsResponse {
  logs: string[]
  total: number
}

export interface ExportDiagnosticRequest {
  include_logs: boolean
  include_config: boolean
  include_system_info: boolean
}

export interface ExportDiagnosticResponse {
  success: boolean
  file_path?: string
  error?: string
}

export interface DiskInfo {
  name: string
  total_gb: number
  available_gb: number
}

export interface SystemInfo {
  cpu_name: string
  cpu_cores: number
  cpu_usage: number
  memory_total_mb: number
  memory_used_mb: number
  os_name: string
  os_version: string
  app_version: string
  disks: DiskInfo[]
}

export interface SystemInfoResponse {
  info: SystemInfo
}

export interface MasterPasswordStatus {
  has_master_password: boolean
  unlocked: boolean
}

// ==================== AI-8 网络与同步（与 Rust 端契约对齐） ====================

/** 局域网内的一台「铃」设备 */
export interface SyncDevice {
  device_id: string
  device_name: string
  ip: string
  port: number
  last_seen: string
  /** udp 自动发现 / manual 手动添加 */
  source?: string
}

/** 同步请求（增量同步时间戳） */
export interface SyncRequest {
  device_id: string
  set_name: string
  last_sync_time?: string | null
}

/** 同步数据包（TCP 传输，记忆内容已加密） */
export interface SyncPayload {
  device_id: string
  set_name: string
  encrypted_data: string
  checksum: string
  incremental: boolean
}

/** 同步响应 */
export interface SyncResponse {
  success: boolean
  message: string
  synced_count: number
  conflict_resolved: boolean
}

/** 发现设备响应 */
export interface DiscoverDevicesResponse {
  devices: SyncDevice[]
}

/** 启动同步请求 */
export interface StartSyncRequest {
  target_device: string
  set_name: string
  manual_ip?: string | null
  manual_port?: number | null
}

/** 启动同步响应 */
export interface StartSyncResponse {
  success: boolean
  message: string
  synced_count: number
}

/** 版本信息 */
export interface VersionInfo {
  current_version: string
  latest_version: string
  release_url: string
  release_notes: string
  is_outdated: boolean
}

/** 检查更新响应 */
export interface CheckUpdateResponse {
  has_update: boolean
  version_info?: VersionInfo | null
  error?: string | null
}

/** 网络状态 */
export type NetworkStatus = 'online' | 'offline' | 'unknown'

/** 获取网络状态响应 */
export interface GetNetworkStatusResponse {
  status: NetworkStatus
}

/** 同步进度事件（后端推送） */
export interface SyncProgressEvent {
  current: number
  total: number
  phase: string // send / receive / done
}

/** 同步历史记录 */
export interface SyncHistoryEntry {
  time: string
  device: string
  set_name: string
  success: boolean
  message: string
  synced_count: number
}

/** 同步状态 */
export interface SyncStatus {
  status: 'idle' | 'discovering' | 'syncing' | 'done' | 'error'
  progress: number
  message?: string | null
  history: SyncHistoryEntry[]
}

/** 网络状态变化事件（后端推送） */
export interface NetworkStatusEvent {
  status: NetworkStatus
}
