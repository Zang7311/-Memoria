// 《铃·记忆体》设置 Store（useSettingStore）AI-7 完整实现
// 对应后端 AppConfig，提供加载/更新/重置/导入导出。
// 数据源统一走 IPC get_config / update_config，不直接操作文件。
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { AppConfig, MasterPasswordStatus, UiThemePreset } from '../types'
import {
  exportConfig,
  getConfig,
  importConfig,
  masterPasswordStatus,
  resetConfig,
  setMasterPassword,
  unlock,
  updateConfig,
} from '../utils/tauri'

export const useSettingStore = defineStore('setting', () => {
  // —— 配置状态（与 AppConfig 对应，snake_case）——
  const loaded = ref(false)
  const firstLaunch = ref(true)
  const theme = ref<'light' | 'dark' | 'win10' | 'edge' | 'minimal' | 'ios-flat' | 'ios-glass'>('dark')
  // —— 外观自定义（用户可自行调整，持久化到 config.json）——
  const accentColor = ref<string | null>(null)
  const bgColor = ref<string | null>(null)
  const bgImage = ref<string | null>(null)
  const avatarSuzu = ref<string | null>(null)
  const avatarUser = ref<string | null>(null)
  const uiRadius = ref<number | null>(null)
  const bubbleUserColor = ref<string | null>(null)
  const bubbleSuzuColor = ref<string | null>(null)
  const uiThemes = ref<UiThemePreset[] | null>(null)
  const runAsAdmin = ref(false)
  const emojiMode = ref<'off' | 'partial' | 'all'>('off')
  const contextLength = ref(10)
  const apiBaseUrl = ref<string | null>(null)
  /** 加密存储的密文（不用于回显明文） */
  const apiKeyEncrypted = ref<string | null>(null)
  /** 明文存储的 API Key（未设置主密码时使用） */
  const apiKeyPlain = ref<string | null>(null)
  /** API 模型名 */
  const apiModel = ref('gpt-3.5-turbo')
  const modelMode = ref<'script' | 'api' | 'local'>('script')
  const depth = ref(2)
  const languageMixRate = ref(8)
  const floatingBallMode = ref<'avatar' | 'simple'>('avatar')
  const floatingBallPosition = ref<[number, number]>([0, 0])
  const monitorEnabled = ref(true)
  const monitorFrequency = ref(3)
  const hotkey = ref('Ctrl+Alt+L')
  const autostart = ref(false)
  const dataPath = ref('')
  const pluginEnabled = ref(true)
  const selfName = ref('铃')
  const userName = ref('主人')
  // 形象人格（daily 日常 / chuunibyou 中二 / healing 治愈 / lewd 涩涩）
  const persona = ref('daily')
  // —— 主密码状态 ——
  const hasMasterPassword = ref(false)
  const unlocked = ref(false)

  // —— 从后端同步完整配置到本地 ——
  function applyConfig(c: AppConfig) {
    firstLaunch.value = c.first_launch
    theme.value = (c.theme as 'light' | 'dark' | 'win10' | 'edge' | 'minimal' | 'ios-flat' | 'ios-glass') || 'dark'
    accentColor.value = c.accent_color ?? null
    bgColor.value = c.bg_color ?? null
    bgImage.value = c.bg_image ?? null
    avatarSuzu.value = c.avatar_suzu ?? null
    avatarUser.value = c.avatar_user ?? null
    uiRadius.value = c.ui_radius ?? null
    bubbleUserColor.value = c.bubble_user_color ?? null
    bubbleSuzuColor.value = c.bubble_suzu_color ?? null
    uiThemes.value = c.ui_themes ?? null
    runAsAdmin.value = c.run_as_admin ?? false
    emojiMode.value = (c.emoji_mode as 'off' | 'partial' | 'all') || 'off'
    contextLength.value = c.context_length
    apiBaseUrl.value = c.api_base_url ?? null
    apiKeyEncrypted.value = c.api_key_encrypted ?? null
    apiKeyPlain.value = c.api_key_plain ?? null
    apiModel.value = c.api_model ?? 'gpt-3.5-turbo'
    modelMode.value = (c.model_mode as 'script' | 'api' | 'local') || 'script'
    depth.value = c.depth
    languageMixRate.value = c.language_mix_rate
    floatingBallMode.value = (c.floating_ball_mode as 'avatar' | 'simple') || 'avatar'
    floatingBallPosition.value = c.floating_ball_position ?? [0, 0]
    monitorEnabled.value = c.monitor_enabled
    monitorFrequency.value = c.monitor_frequency
    hotkey.value = c.hotkey
    autostart.value = c.autostart
    dataPath.value = c.data_path
    pluginEnabled.value = c.plugin_enabled
    selfName.value = c.self_name ?? '铃'
    userName.value = c.user_name ?? '主人'
    persona.value = c.persona ?? 'daily'
    hasMasterPassword.value = c.has_master_password
    loaded.value = true
  }

  /** 从后端加载完整配置（App.vue 启动时调用） */
  async function loadConfig() {
    const res = await getConfig()
    applyConfig(res.config)
    // 同步主密码解锁状态
    try {
      const st = await masterPasswordStatus()
      unlocked.value = st.unlocked
      hasMasterPassword.value = st.has_master_password
    } catch { /* 忽略 */ }
  }

  /** 增量更新配置并刷新本地状态 */
  async function update(updates: Record<string, unknown>) {
    const res = await updateConfig(updates)
    applyConfig(res.config)
    return res.config
  }

  /** 切换主题（持久化） */
  async function toggleTheme() {
    const next = theme.value === 'dark' ? 'light' : 'dark'
    await update({ theme: next })
  }

  /** 设置指定主题（暗/亮 + 5 套 UI 风格：win10/edge/minimal/ios-flat/ios-glass） */
  async function setTheme(t: 'light' | 'dark' | 'win10' | 'edge' | 'minimal' | 'ios-flat' | 'ios-glass') {
    theme.value = t
    await update({ theme: t })
  }

  /** 把组合值应用到当前字段（不写盘） */
  function applyPreset(p: UiThemePreset) {
    accentColor.value = p.accent_color || null
    bgColor.value = p.bg_color || null
    bgImage.value = p.bg_image ?? null
    bubbleUserColor.value = p.bubble_user_color || null
    bubbleSuzuColor.value = p.bubble_suzu_color || null
    uiRadius.value = p.ui_radius ?? null
    avatarSuzu.value = p.avatar_suzu ?? null
  }

  /** 把当前外观自定义另存为一套命名主题组合（同名覆盖） */
  async function saveThemePreset(name: string): Promise<UiThemePreset> {
    const preset: UiThemePreset = {
      name,
      accent_color: accentColor.value ?? '#ff7a94',
      bg_color: bgColor.value ?? '#1d1b1f',
      bg_image: bgImage.value,
      bubble_user_color: bubbleUserColor.value ?? '#2d2d2d',
      bubble_suzu_color: bubbleSuzuColor.value ?? '#3a3438',
      ui_radius: uiRadius.value ?? 12,
      avatar_suzu: avatarSuzu.value,
    }
    const themes = uiThemes.value ?? []
    const idx = themes.findIndex((t) => t.name === name)
    if (idx >= 0) themes[idx] = preset
    else themes.push(preset)
    uiThemes.value = themes
    await update({ ui_themes: themes })
    return preset
  }

  /** 一键切换到指定主题组合（加载并持久化） */
  async function switchThemePreset(name: string) {
    const p = (uiThemes.value ?? []).find((t) => t.name === name)
    if (!p) return
    applyPreset(p)
    await update({
      accent_color: p.accent_color || null,
      bg_color: p.bg_color || null,
      bg_image: p.bg_image ?? null,
      bubble_user_color: p.bubble_user_color || null,
      bubble_suzu_color: p.bubble_suzu_color || null,
      ui_radius: p.ui_radius ?? null,
      avatar_suzu: p.avatar_suzu ?? null,
    })
  }

  /** 删除指定主题组合 */
  async function deleteThemePreset(name: string) {
    uiThemes.value = (uiThemes.value ?? []).filter((t) => t.name !== name)
    await update({ ui_themes: uiThemes.value })
  }

  /** 保存 API Key 明文（后端按主密码状态决定：已解锁→加密存储，未设置→明文存储） */
  async function saveApiKey(plain: string) {
    await update({ api_key: plain })
  }

  /** 重置所有配置为默认（保留主密码与已加密 Key） */
  async function resetAll() {
    const res = await resetConfig()
    applyConfig(res.config)
    return res.config
  }

  /** 导出配置为 JSON 文件 */
  async function exportToFile() {
    return await exportConfig()
  }

  /** 从 JSON 文件导入配置 */
  async function importFromFile(path: string) {
    const res = await importConfig(path)
    applyConfig(res.config)
    return res.config
  }

  // —— 主密码 ——
  /** 设置主密码（首次引导 / 修改） */
  async function setupMasterPassword(password: string) {
    await setMasterPassword(password)
    hasMasterPassword.value = true
    unlocked.value = true
  }

  /** 解锁 */
  async function unlockVault(password: string) {
    await unlock(password)
    unlocked.value = true
  }

  /** 查询主密码状态 */
  async function refreshMasterStatus(): Promise<MasterPasswordStatus> {
    const st = await masterPasswordStatus()
    hasMasterPassword.value = st.has_master_password
    unlocked.value = st.unlocked
    return st
  }

  return {
    loaded, firstLaunch,
    theme, contextLength, apiBaseUrl, apiKeyEncrypted, apiKeyPlain, apiModel, modelMode, depth,
    accentColor, bgColor, bgImage, avatarSuzu, avatarUser, uiRadius,
    bubbleUserColor, bubbleSuzuColor, uiThemes,
    runAsAdmin,
    emojiMode,
    saveThemePreset, switchThemePreset, deleteThemePreset,
    languageMixRate, floatingBallMode, floatingBallPosition, monitorEnabled,
    monitorFrequency, hotkey, autostart, dataPath, pluginEnabled, selfName, userName, persona,
    hasMasterPassword, unlocked,
    applyConfig, loadConfig, update, toggleTheme, setTheme, saveApiKey, resetAll,
    exportToFile, importFromFile, setupMasterPassword, unlockVault, refreshMasterStatus,
  }
})
