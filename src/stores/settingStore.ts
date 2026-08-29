// 《铃·记忆体》设置 Store（useSettingStore）AI-7 完整实现
// 对应后端 AppConfig，提供加载/更新/重置/导入导出。
// 数据源统一走 IPC get_config / update_config，不直接操作文件。
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { AppConfig, MasterPasswordStatus } from '../types'
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
  const theme = ref<'light' | 'dark'>('dark')
  const contextLength = ref(10)
  const apiBaseUrl = ref<string | null>(null)
  /** 加密存储的密文（不用于回显明文） */
  const apiKeyEncrypted = ref<string | null>(null)
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
  // —— 主密码状态 ——
  const hasMasterPassword = ref(false)
  const unlocked = ref(false)

  // —— 从后端同步完整配置到本地 ——
  function applyConfig(c: AppConfig) {
    firstLaunch.value = c.first_launch
    theme.value = (c.theme as 'light' | 'dark') || 'dark'
    contextLength.value = c.context_length
    apiBaseUrl.value = c.api_base_url ?? null
    apiKeyEncrypted.value = c.api_key_encrypted ?? null
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

  /** 保存 API Key 明文（后端加密存储） */
  async function saveApiKey(plain: string) {
    if (!unlocked.value) throw new Error('请先设置/输入主密码解锁')
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
    theme, contextLength, apiBaseUrl, apiKeyEncrypted, modelMode, depth,
    languageMixRate, floatingBallMode, floatingBallPosition, monitorEnabled,
    monitorFrequency, hotkey, autostart, dataPath, pluginEnabled, selfName, userName,
    hasMasterPassword, unlocked,
    applyConfig, loadConfig, update, toggleTheme, saveApiKey, resetAll,
    exportToFile, importFromFile, setupMasterPassword, unlockVault, refreshMasterStatus,
  }
})
