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
}

/** 应用设置 */
export interface Setting {
  theme: 'light' | 'dark'
  contextLength: number // 默认 10
  apiBaseUrl?: string
  apiKey?: string
  modelMode: 'script' | 'api' | 'local' // 默认 'script'
  depth: 1 | 2 | 3 | 4 // 默认 2
}

/** 模型模式中文名映射（可选，便于 UI 展示） */
export const MODEL_MODE_LABEL: Record<Setting['modelMode'], string> = {
  script: '脚本模式',
  api: 'API 模式',
  local: '本地模式',
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
}

/** 获取监测状态 + 规则列表响应 */
export interface GetMonitorRulesResponse {
  rules: ScreenMonitorRule[]
  enabled: boolean
  interval_seconds: number
  available: boolean
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
  model_mode: 'script' | 'api' | 'local' | string
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
  master_password_salt?: string | null
  has_master_password: boolean
}

export interface GetConfigResponse {
  config: AppConfig
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
