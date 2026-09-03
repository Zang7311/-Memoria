// 《铃·记忆体》Tauri IPC 封装
// 任务书 任务 7：封装 invoke 与 listen，供 ChatInput / useStreamRender 等统一调用
// 后端命令与事件由 AI-3 实现；此处仅做前端封装，不写 Rust。
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

/**
 * 发送一条用户消息（后台 AI-3 负责生成回复）
 * 对应 Rust 侧 send_message 命令，参数 content + depth + sessionId（可选）
 */
export async function sendMessage(content: string, depth: number, sessionId?: string | null): Promise<void> {
  return await invoke('send_message', { content, depth, sessionId })
}

/**
 * 监听流式回复片段（每次 2~5 个字）
 * 返回取消监听函数
 */
export function onChatChunk(callback: (chunk: string) => void): Promise<UnlistenFn> {
  return listen<string>('chat_chunk', (event) => callback(event.payload))
}

/**
 * 监听流式结束信号
 */
export function onChatEnd(callback: () => void): Promise<UnlistenFn> {
  return listen('chat_end', callback)
}

/**
 * 监听流式错误信号
 */
export function onChatError(callback: (error: string) => void): Promise<UnlistenFn> {
  return listen<string>('chat_error', (event) => callback(event.payload))
}

/**
 * 监听 token 用量事件（API 模式流式结束时推送；脚本/本地模式无此事件）
 */
export function onChatUsage(callback: (usage: ChatUsage) => void): Promise<UnlistenFn> {
  return listen<ChatUsage>('chat_usage', (event) => callback(event.payload))
}

// 兼容旧环境的 greet（保留，供测试通道使用）
import type { ChatUsage, DetectOllamaResponse, GpuVram, Message, Session, SessionMeta, TestConnectionResponse } from '../types'
export function greet(name: string): Promise<string> {
  return invoke<string>('greet', { name })
}

/**
 * 测试 API 连接（AI-3 test_api_connection）
 * @param base_url API 地址（带/不带 /v1 均可）
 * @param api_key 明文 API Key（测试当前输入框内容，不涉及已保存密文）
 */
export function testApiConnection(base_url: string, api_key: string): Promise<TestConnectionResponse> {
  return invoke('test_api_connection', { baseUrl: base_url, apiKey: api_key })
}

/**
 * 用系统默认浏览器打开外部链接（收尾工程师新增，复用 tauri-plugin-opener）
 * 用于跳转：GitHub 主页 / DeepSeek 平台 / Hermes 技能目录等
 */
export function openUrl(url: string): Promise<void> {
  return invoke('open_url', { url })
}

/** 检测是否以管理员权限运行 */
export function isAdmin(): Promise<boolean> {
  return invoke('is_admin')
}

/** 以管理员权限重启应用（弹 UAC 提示） */
export function restartAsAdmin(): Promise<void> {
  return invoke('restart_as_admin')
}

/** 检测 Ollama 是否安装 + 列出已安装模型（一键本地部署 AI） */
export function detectOllama(): Promise<DetectOllamaResponse> {
  return invoke('detect_ollama')
}

/** 一键拉取本地模型（ollama pull，阻塞等待完成） */
export function pullModel(model: string): Promise<string> {
  return invoke('pull_model', { model })
}

/** 检测显卡显存（用于判断能否跑本地 AI） */
export function detectGpuVram(): Promise<GpuVram[]> {
  return invoke('detect_gpu_vram')
}

/** 设置 Ollama 模型存储路径（OLLAMA_MODELS 用户环境变量） */
export function setOllamaModelsPath(path: string): Promise<string> {
  return invoke('set_ollama_models_path', { path })
}

// ==================== 多会话管理 IPC（收尾工程师批次3） ====================

/** 列出所有会话元信息（按更新时间倒序） */
export function listSessions(): Promise<SessionMeta[]> {
  return invoke('session_list')
}

/** 新建会话（返回空会话） */
export function createSession(): Promise<Session> {
  return invoke('session_create')
}

/** 加载单个会话（含完整消息） */
export function loadSession(id: string): Promise<Session> {
  return invoke('session_load', { id })
}

/** 保存会话消息（更新标题/计数/时间；不存在则自动新建） */
export function saveSession(id: string, messages: Message[]): Promise<Session> {
  return invoke('session_save', { id, messages })
}

/** 重命名会话 */
export function renameSession(id: string, title: string): Promise<SessionMeta> {
  return invoke('session_rename', { id, title })
}

/** 删除会话 */
export function deleteSession(id: string): Promise<void> {
  return invoke('session_delete', { id })
}

// —— 类型提示：避免未使用告警 ——
export type { Message }

// ==================== AI-4 记忆 IPC 封装 ====================
import type {
  DeleteMemoryResponse,
  GetMemoriesRequest,
  GetMemoriesResponse,
  Memory,
} from '../types'

/** 获取记忆列表（支持分页/搜索/记忆集） */
export function getMemories(req: GetMemoriesRequest = {}): Promise<GetMemoriesResponse> {
  return invoke('get_memories', { req })
}

// ==================== 记忆中心（大项目） ====================

/** 记忆中心统计（条数/容量/分类分布/重复数） */
export function memoryStats(setName?: string | null): Promise<{
  total: number
  size_mb: number
  important_count: number
  duplicate_count: number
  categories: { name: string; count: number }[]
}> {
  return invoke('memory_stats', { setName: setName ?? null })
}

/** 批量删除记忆 */
export function deleteMemoriesBatch(ids: string[], setName?: string | null): Promise<number> {
  return invoke('delete_memories_batch', { ids, setName: setName ?? null })
}

/** 批量标记重要/取消 */
export function markImportantBatch(ids: string[], important: boolean, setName?: string | null): Promise<number> {
  return invoke('mark_important_batch', { ids, important, setName: setName ?? null })
}

/** 编辑记忆内容 */
export function editMemoryContent(id: string, content: string, setName?: string | null): Promise<void> {
  return invoke('edit_memory_content', { id, content, setName: setName ?? null })
}

/** 删除单条记忆 */
export function deleteMemory(memory_id: string, set_name?: string): Promise<DeleteMemoryResponse> {
  return invoke('delete_memory', { req: { memory_id, set_name } })
}

/** 切换记忆集，返回当前集名称 */
export function switchMemorySet(set_name: string): Promise<string> {
  return invoke('switch_memory_set', { req: { set_name } })
}

// ==================== P3 陪伴记录（与铃的日记） ====================

/** 记录一个里程碑（首次见面日期在第一次调用时自动记下；同一 key 幂等） */
export function recordMilestone(key: string, label: string): Promise<void> {
  return invoke('record_milestone', { key, label })
}

/** 获取陪伴记录（含陪伴天数） */
export function getMilestones(): Promise<{
  first_date: string | null
  days: number
  items: { key: string; label: string; date: string }[]
  daily: { date: string; chat_count: number; tool_count: number; topics: string[] }[]
}> {
  return invoke('get_milestones')
}

/** 记录一次聊天（当日累积：句数+1、话题合并） */
export function recordDailyChat(text: string): Promise<void> {
  return invoke('record_daily_chat', { text })
}

/** 记录一次工具箱使用（当日工具数+1） */
export function recordDailyTool(toolName: string): Promise<void> {
  return invoke('record_daily_tool', { toolName })
}

// ==================== P3 救援模式 ====================

/** 救援检测：检查配置/记忆/插件/日志等关键资源 */
export function recoveryCheck(): Promise<{ name: string; ok: boolean; detail: string }[]> {
  return invoke('recovery_check')
}

/** 重置配置（自动备份后重建默认） */
export function recoveryResetConfig(): Promise<string> {
  return invoke('recovery_reset_config')
}

/** 创建新记忆集，返回新集名称 */
export function createMemorySet(set_name: string): Promise<string> {
  return invoke('create_memory_set', { req: { set_name } })
}

/** 列出所有记忆集 */
export function listMemorySets(): Promise<string[]> {
  return invoke('list_memory_sets')
}

/** 标记记忆为重要（⭐） */
export function markMemoryImportant(memory_id: string, set_name?: string): Promise<Memory> {
  return invoke('mark_memory_important', { memoryId: memory_id, setName: set_name })
}

// ==================== AI-5 插件 IPC 封装 ====================
import type {
  ExecuteSkillRequest,
  ExecuteSkillResponse,
  ListPluginsResponse,
  Plugin,
} from '../types'

/** 列出所有已安装插件 */
export function listPlugins(): Promise<ListPluginsResponse> {
  return invoke('list_plugins')
}

/** 安装插件（source：本地目录路径 或 Git URL） */
export function installPlugin(source: string): Promise<Plugin> {
  return invoke('install_plugin', { req: { source } })
}

/** 卸载插件 */
export function uninstallPlugin(plugin_id: string): Promise<void> {
  return invoke('uninstall_plugin', { req: { plugin_id } })
}

/** 启用插件 */
export function enablePlugin(plugin_id: string): Promise<Plugin> {
  return invoke('enable_plugin', { req: { plugin_id } })
}

/** 禁用插件 */
export function disablePlugin(plugin_id: string): Promise<Plugin> {
  return invoke('disable_plugin', { req: { plugin_id } })
}

/** 执行插件技能（自然语言触发入口） */
export function executeSkill(req: ExecuteSkillRequest): Promise<ExecuteSkillResponse> {
  return invoke('execute_skill', { req })
}

/** 授予/收回插件权限 */
export function setPluginPermission(plugin_id: string, permission: string, allow: boolean): Promise<Plugin> {
  return invoke('set_plugin_permission', { req: { plugin_id, permission, allow } })
}

/** 添加自定义终端命令 */
export function addTerminalCommand(name: string, command: string, description: string): Promise<Plugin> {
  return invoke('add_terminal_command', { req: { name, command, description } })
}

// ==================== AI-6 桌面交互 IPC 封装 ====================
import type {
  ExecuteQuickCommandResponse,
  ExecuteToolboxResponse,
  GetMonitorRulesResponse,
  ListQuickCommandsResponse,
  MonitorTriggerEvent,
  QuickCommand,
  ScreenMonitorRule,
  ToolboxItem,
  WindowInfo,
} from '../types'

/** 获取当前前台窗口信息 */
export function getWindowInfo(): Promise<{ info: WindowInfo }> {
  return invoke('get_window_info')
}

/** 获取监测状态 + 规则列表 */
export function getMonitorRules(): Promise<GetMonitorRulesResponse> {
  return invoke('get_monitor_rules')
}

/** 更新（或新增）单条监测规则 */
export function updateMonitorRule(rule: ScreenMonitorRule): Promise<void> {
  return invoke('update_monitor_rule', { request: { rule } })
}

/** 删除单条监测规则 */
export function deleteMonitorRule(rule_id: string): Promise<void> {
  return invoke('delete_monitor_rule', { request: { rule_id } })
}

/** 启用/禁用屏幕监测（可附带调整轮询频率），返回最终是否启用 */
export function toggleMonitoring(enabled: boolean, interval_seconds?: number): Promise<boolean> {
  return invoke('toggle_monitoring', { request: { enabled, interval_seconds } })
}

/** 列出工具箱条目（预设 + 用户自定义） */
export function listToolboxItems(): Promise<{ items: ToolboxItem[] }> {
  return invoke('list_toolbox_items')
}

/** 执行工具箱命令（需要输入参数的工具传 input，对应 {input} 占位符） */
export function executeToolbox(item_id: string, input?: string): Promise<ExecuteToolboxResponse> {
  return invoke('execute_toolbox', { request: { item_id, input: input ?? null } })
}

// ==================== AI-9 快捷指令 IPC 封装 ====================

/** 列出所有快捷指令 */
export function listQuickCommands(): Promise<ListQuickCommandsResponse> {
  return invoke('list_quick_commands')
}

/** 新增/更新一条快捷指令 */
export function saveQuickCommand(command: QuickCommand): Promise<void> {
  return invoke('save_quick_command', { request: { command } })
}

/** 删除一条快捷指令 */
export function deleteQuickCommand(command_id: string): Promise<void> {
  return invoke('delete_quick_command', { request: { command_id } })
}

/** 按顺序执行一条快捷指令的全部动作 */
export function executeQuickCommand(command_id: string): Promise<ExecuteQuickCommandResponse> {
  return invoke('execute_quick_command', { request: { command_id } })
}

/** 切换系统电源计划（mode: balanced | high | power-saver） */
export function setPowerMode(mode: string): Promise<string> {
  return invoke('set_power_mode', { mode })
}

/** 设置系统主音量（0-100） */
export function setVolume(level: number): Promise<string> {
  return invoke('set_volume', { level })
}

/** 启动音乐（path 为空则打开用户音乐目录） */
export function playMusic(path?: string | null): Promise<string> {
  return invoke('play_music', { path: path ?? null })
}

/** 保存像素画 PNG（data URL）到桌面，返回保存路径 */
export function savePixelArt(dataUrl: string): Promise<string> {
  return invoke('save_pixel_art', { dataUrl })
}

/** 保存 UI 图片（背景图/头像）到应用数据目录 ui_assets/，返回保存路径 */
export function saveUiImage(dataUrl: string, prefix: string): Promise<string> {
  return invoke('save_ui_image', { dataUrl, prefix })
}

// ==================== moon10 二维码生成与识别 ====================

/** 生成二维码 PNG 保存到桌面，返回保存路径（text→图片） */
export function generateQrcode(text: string, size?: number): Promise<string> {
  return invoke('generate_qrcode', { text, size: size ?? null })
}

/** 识别图片中的二维码，返回解码出的文本内容（图片路径→文本） */
export function decodeQrcode(imagePath: string): Promise<string> {
  return invoke('decode_qrcode', { imagePath })
}

/** OCR 识别图片文字（moon11）：返回 { engine, text, success }，优先 Windows OCR，失败自动降级 Tesseract */
export function ocrImage(imagePath: string): Promise<{ engine: string; text: string; success: boolean }> {
  return invoke('ocr_image', { imagePath })
}

/** 把本地文件路径转为 webview 可加载的 asset URL（背景图/头像图片用，解决本地路径无法显示） */
export function assetUrl(path: string): string {
  return convertFileSrc(path)
}

/** 统一依赖管理器：检查依赖是否安装（返回安装状态/引导/下载页） */
export function checkDependency(id: string): Promise<{ id: string; installed: boolean; name: string; required: boolean; install: string; url: string | null }> {
  return invoke('check_dependency', { id })
}

/** 判断字符串是否为本地图片路径（含路径分隔符或图片扩展名） */
export function isImagePath(v?: string | null): boolean {
  if (!v) return false
  return v.includes('\\') || v.includes('/') || /\.(png|jpe?g|gif|webp|bmp|ico)$/i.test(v)
}

/** 新增/更新用户自定义工具箱条目 */
export function saveToolboxItem(item: ToolboxItem): Promise<void> {
  return invoke('save_toolbox_item', { request: { item } })
}

/** 删除用户自定义工具箱条目 */
export function deleteToolboxItem(item_id: string): Promise<void> {
  return invoke('delete_toolbox_item', { request: { item_id } })
}

/** 显示/隐藏悬浮球 */
export function setFloatingBallVisibility(visible: boolean): Promise<void> {
  return invoke('set_floating_ball_visibility', { request: { visible } })
}

// ==================== v0.6 悬浮球重构：鼠标穿透 ====================

/** 切换悬浮球鼠标穿透（开启后窗口忽略鼠标事件，托盘菜单可恢复），返回最新状态 */
export function setFloatingBallClickThrough(enabled: boolean): Promise<boolean> {
  return invoke('set_floating_ball_click_through', { enabled })
}

/** 查询悬浮球鼠标穿透状态（初始化 UI 用） */
export function getFloatingBallClickThrough(): Promise<boolean> {
  return invoke('get_floating_ball_click_through')
}

/** 监听穿透状态变化（悬浮球/主窗口同步 UI 用） */
export function onBallClickThroughChanged(callback: (enabled: boolean) => void): Promise<UnlistenFn> {
  return listen<boolean>('ball-click-through-changed', (event) => callback(event.payload))
}

/** 监听配置更新事件（悬浮球/气泡窗口收到后重新加载配置） */
export function onConfigUpdated(callback: () => void): Promise<UnlistenFn> {
  return listen('config-updated', callback)
}

/** 确保主窗口存在（用户可能已关闭主窗；悬浮球「打开主窗口/快速提问/设置」前调用） */
export function ensureMainWindow(): Promise<void> {
  return invoke('ensure_main_window')
}

/** 注册全局快捷键（如 Ctrl+Alt+L） */
export function registerHotkey(accelerator: string): Promise<{ registered: boolean; accelerator: string }> {
  return invoke('register_hotkey', { request: { accelerator } })
}

/** 注销全部全局快捷键 */
export function unregisterHotkey(): Promise<void> {
  return invoke('unregister_hotkey')
}

/** 设置开机自启动 */
export function setAutostart(enabled: boolean): Promise<void> {
  return invoke('set_autostart', { request: { enabled } })
}

/** 查询开机自启动状态 */
export function getAutostart(): Promise<{ enabled: boolean }> {
  return invoke('get_autostart')
}

/** 监听屏幕监测触发事件（气泡弹窗内容） */
export function onMonitorTrigger(callback: (payload: MonitorTriggerEvent) => void): Promise<UnlistenFn> {
  return listen<MonitorTriggerEvent>('monitor-trigger', (event) => callback(event.payload))
}

/** 监听屏幕监测不可用事件 */
export function onMonitorUnavailable(callback: (message: string) => void): Promise<UnlistenFn> {
  return listen<string>('monitor-unavailable', (event) => callback(event.payload))
}

// ==================== AI-7 配置与诊断 IPC 封装 ====================
import type {
  ExportConfigResponse,
  ExportDiagnosticRequest,
  ExportDiagnosticResponse,
  GetConfigResponse,
  GetLogsRequest,
  GetLogsResponse,
  MasterPasswordStatus,
  SystemInfoResponse,
} from '../types'

/** 获取完整配置 */
export function getConfig(): Promise<GetConfigResponse> {
  return invoke('get_config')
}

/** 增量更新配置（null 清除字段）；返回更新后完整配置 */
export function updateConfig(updates: Record<string, unknown>): Promise<GetConfigResponse> {
  return invoke('update_config', { request: { updates } })
}

/** 导出配置为 JSON 文件 */
export function exportConfig(): Promise<ExportConfigResponse> {
  return invoke('export_config')
}

/** 从 JSON 文件导入配置 */
export function importConfig(path: string): Promise<GetConfigResponse> {
  return invoke('import_config', { request: { path } })
}

/** 重置所有配置为默认值（保留主密码与已加密 API Key） */
export function resetConfig(): Promise<GetConfigResponse> {
  return invoke('reset_config')
}

/** 获取日志（级别过滤 + 关键词 + 分页） */
export function getLogs(req: GetLogsRequest = {}): Promise<GetLogsResponse> {
  return invoke('get_logs', { request: req })
}

/** 清空日志 */
export function clearLogs(): Promise<void> {
  return invoke('clear_logs')
}

/** 导出诊断包（脱敏配置 + 日志 + 系统信息 → zip） */
export function exportDiagnostic(req: ExportDiagnosticRequest): Promise<ExportDiagnosticResponse> {
  return invoke('export_diagnostic', { request: req })
}

/** 获取系统信息 */
export function getSystemInfo(): Promise<SystemInfoResponse> {
  return invoke('get_system_info')
}

/** 设置主密码（首次引导/修改） */
export function setMasterPassword(password: string): Promise<void> {
  return invoke('set_master_password', { request: { password } })
}

/** 解锁（输入主密码恢复密钥） */
export function unlock(password: string): Promise<boolean> {
  return invoke('unlock', { request: { password } })
}

/** 查询主密码状态（是否设置过 + 是否已解锁） */
export function masterPasswordStatus(): Promise<MasterPasswordStatus> {
  return invoke('master_password_status')
}

/** 监听配置变更事件 */
export function onConfigChanged(callback: () => void): Promise<UnlistenFn> {
  return listen('config-changed', () => callback())
}

// ==================== AI-8 网络与同步 IPC 封装 ====================
import type {
  CheckUpdateResponse,
  DiscoverDevicesResponse,
  GetNetworkStatusResponse,
  NetworkStatusEvent,
  StartSyncResponse,
  SyncProgressEvent,
  SyncStatus,
} from '../types'

/** 扫描局域网设备（UDP 广播，timeout 秒） */
export function discoverDevices(timeout_secs?: number): Promise<DiscoverDevicesResponse> {
  return invoke('discover_devices', { timeoutSecs: timeout_secs ?? 3 })
}

/** 手动添加设备（UDP 被阻断时的备选） */
export function addManualDevice(ip: string, port?: number): Promise<DiscoverDevicesResponse> {
  return invoke('add_manual_device', { ip, port: port ?? 54546 })
}

/** 获取当前设备列表（缓存，不扫描） */
export function getSyncDevices(): Promise<DiscoverDevicesResponse> {
  return invoke('get_sync_devices')
}

/** 发起同步（从目标设备拉取记忆集） */
export function startSync(req: {
  target_device: string
  set_name: string
  manual_ip?: string | null
  manual_port?: number | null
}): Promise<StartSyncResponse> {
  return invoke('start_sync', { request: req })
}

/** 获取同步状态与历史 */
export function getSyncStatus(): Promise<SyncStatus> {
  return invoke('get_sync_status')
}

/** 设置同步主密码（与 AI-7 共享同一体系） */
export function setSyncPassword(password: string): Promise<void> {
  return invoke('set_sync_password', { request: { password } })
}

/** 设置冲突解决策略 */
export function setConflictPolicy(policy: 'newest' | 'local' | 'remote'): Promise<void> {
  return invoke('set_conflict_policy', { policy })
}

/** 检查更新（force 强制重新检查） */
export function checkUpdate(force?: boolean): Promise<CheckUpdateResponse> {
  return invoke('check_update', { force: force ?? false })
}

/** 获取网络状态 */
export function getNetworkStatus(): Promise<GetNetworkStatusResponse> {
  return invoke('get_network_status')
}

/** 监听同步进度事件 */
export function onSyncProgress(callback: (payload: SyncProgressEvent) => void): Promise<UnlistenFn> {
  return listen<SyncProgressEvent>('sync-progress', (event) => callback(event.payload))
}

/** 监听网络状态变化事件 */
export function onNetworkStatusChanged(callback: (payload: NetworkStatusEvent) => void): Promise<UnlistenFn> {
  return listen<NetworkStatusEvent>('network-status-changed', (event) => callback(event.payload))
}

// ==================== 离线增强方案 IPC 封装 ====================

export type SearchMode = 'bigram' | 'bm25' | 'vector'

export interface VectorModelStatus {
  available: boolean
  message: string
}

/** 获取当前离线检索模式 */
export function getSearchMode(): Promise<SearchMode> {
  return invoke<SearchMode>('get_search_mode')
}

/** 设置离线检索模式（bigram / bm25 / vector） */
export function setSearchMode(mode: SearchMode): Promise<SearchMode> {
  return invoke<SearchMode>('set_search_mode', { mode })
}

/** 检测向量模型是否已安装（不加载模型） */
export function checkVectorModelStatus(): Promise<VectorModelStatus> {
  return invoke<VectorModelStatus>('check_vector_model_status')
}

export interface ModelCandidate {
  path: string
  filename: string
  size_mb: number
  exists_in_target: boolean
}

export interface InstallModelResult {
  success: boolean
  message: string
  missing_files: string[]
}

/** 扫描常见位置的向量模型文件（model.safetensors / bge* 等） */
export function scanModelFiles(): Promise<ModelCandidate[]> {
  return invoke<ModelCandidate[]>('scan_model_files')
}

/** 将指定模型文件及同级 config.json/tokenizer.json 安装到 ~/.铃记忆体/models/ */
export function installModel(path: string): Promise<InstallModelResult> {
  return invoke<InstallModelResult>('install_model', { path })
}
