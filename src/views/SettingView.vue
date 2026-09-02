<!-- 《铃·记忆体》设置页（AI-7 4.1）完整实现：9 个标签页
     通用 / 模型 / 记忆 / 监测 / 个性化 / 插件 / 同步 / 赞助 / 诊断 -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import DiagnosticView from '../components/DiagnosticView.vue'
import MonitorSettings from '../components/MonitorSettings.vue'
import PluginManager from '../components/PluginManager.vue'
import QuickCommandPanel from '../components/QuickCommandPanel.vue'
import SyncPanel from '../components/SyncPanel.vue'
import VBILSettings from './VBILSettings.vue'
import ToolboxPanel from '../components/ToolboxPanel.vue'
import { useSettingStore } from '../stores/settingStore'
import { useMilestoneStore } from '../stores/milestoneStore'
import { MODEL_MODE_LABEL } from '../types'
import TheIcon from '../components/TheIcon.vue'
import { detectGpuVram, detectOllama, getAutostart, isAdmin, openUrl, pullModel, registerHotkey, restartAsAdmin, saveUiImage, setAutostart, setOllamaModelsPath, testApiConnection, checkVectorModelStatus, scanModelFiles, installModel, type ModelCandidate } from '../utils/tauri'

const setting = useSettingStore()
const milestone = useMilestoneStore()

// —— P3 日记：1-3 天一组，折叠摘要行 + 15-40 字正文 ——
const diaryOpen = ref<number | null>(null)
const showBadges = ref(false)
function toggleDiary(i: number) {
  diaryOpen.value = diaryOpen.value === i ? null : i
}
// 按 1-3 天分组合并（倒序 daily → 正序处理 → 每 3 天一组）
interface DiaryGroup {
  end: string
  days: number
  chat: number
  tools: number
  topics: string[]
  summary: string
  body: string
}
const diaryGroups = computed<DiaryGroup[]>(() => {
  // daily 已倒序（最新在前），翻转成时间正序后每 3 天一组
  const days = [...milestone.daily].reverse()
  const groups: DiaryGroup[] = []
  for (let i = 0; i < days.length; i += 3) {
    const chunk = days.slice(i, i + 3)
    const chat = chunk.reduce((s, d) => s + d.chat_count, 0)
    const tools = chunk.reduce((s, d) => s + d.tool_count, 0)
    const topics = [...new Set(chunk.flatMap((d) => d.topics))].slice(0, 3)
    const end = chunk[chunk.length - 1].date.slice(5) // mm-dd
    const start = chunk[0].date.slice(5)
    const dayLabel = chunk.length > 1 ? `${start}~${end}` : end
    // 摘要行：mm-dd · 聊了x句 · 用了x次工具 · 话题：xx
    const summary = `${dayLabel} · 聊了${chat}句 · 用了${tools}次工具${topics.length ? ' · 话题：' + topics.join('、') : ''}`
    // 正文（15-40 字，铃的语气）
    const body = buildDiaryBody(chunk.length, chat, tools, topics)
    groups.push({ end: dayLabel, days: chunk.length, chat, tools, topics, summary, body })
  }
  return groups.reverse() // 最新在前
})
function buildDiaryBody(dayCount: number, chat: number, tools: number, topics: string[]): string {
  const t = topics.length ? topics.join('、') : ''
  if (chat === 0 && tools === 0) return `这天没什么记录，但铃记得，主人出现过～`
  if (chat === 0) return `主人这一天让铃帮忙用了${tools}次工具，忙忙碌碌的，但能帮上忙铃就很开心～`
  const topicPart = t ? `聊了「${t}」` : '聊了些家常'
  if (dayCount >= 2) return `这几天${topicPart}，共${chat}句、${tools}次工具。和主人待在一起，日子都暖暖的～`
  return `今天${topicPart}，聊了${chat}句${tools ? `、用了${tools}次工具` : ''}。铃把这些都记在日记里啦～`
}

const tabs = [
  { key: 'general', label: '通用', icon: 'settings' },
  { key: 'model', label: '模型', icon: 'command' },
  { key: 'memory', label: '记忆', icon: 'memory' },
  { key: 'monitor', label: '监测', icon: 'monitor' },
  { key: 'vbil', label: '形象', icon: 'customize' },
  { key: 'ball', label: '悬浮球', icon: 'customize' },
  { key: 'persona', label: '个性化', icon: 'customize' },
  { key: 'plugin', label: '插件', icon: 'plugin' },
  { key: 'quick', label: '指令', icon: 'command' },
  { key: 'sync', label: '同步', icon: 'sync' },
  { key: 'sponsor', label: '赞助', icon: 'sponsor' },
  { key: 'diag', label: '诊断', icon: 'diagnostic' },
]
const activeTab = ref('general')

// —— 快捷键（AI-6 复用）——
const hotkey = ref('Ctrl+Alt+L')
const hotkeyMsg = ref('')
// —— 自启动 ——
const autostart = ref(false)
const autostartMsg = ref('')
// —— 工具箱悬浮窗 ——
const showToolbox = ref(false)
// —— 通用反馈 ——
const generalMsg = ref('')
const exportPath = ref('')

// —— 模型/加密 ——
const modelMode = ref<'script' | 'api' | 'local'>('script')
const apiBaseUrl = ref('')
const apiModel = ref('gpt-3.5-turbo')
const apiKeyInput = ref('')
const depth = ref(2)
// 离线语义检索模型（bge, 方案3）状态
const vectorModelStatus = ref<{ available: boolean; message: string } | null>(null)
const vectorCandidates = ref<ModelCandidate[]>([])
const vectorScanning = ref(false)
const vectorInstalling = ref(false)
const vectorMsg = ref('')
// 检查离线语义检索模型
async function checkVectorModel() {
  try {
    vectorModelStatus.value = await checkVectorModelStatus()
  } catch (e) {
    vectorModelStatus.value = null
    vectorMsg.value = `检测失败：${e}`
  }
}
// 扫描本地常见位置的模型候选
async function scanVectorModels() {
  vectorScanning.value = true
  vectorMsg.value = ''
  try {
    vectorCandidates.value = await scanModelFiles()
    if (!vectorCandidates.value.length) {
      vectorMsg.value = '未在常见位置找到模型，请手动放入 ~/.铃记忆体/models/'
    }
  } catch (e) {
    vectorMsg.value = `扫描失败：${e}`
  } finally {
    vectorScanning.value = false
  }
}
// 一键安装选中的模型
async function doInstallVectorModel(path: string) {
  vectorInstalling.value = true
  vectorMsg.value = ''
  try {
    await installModel(path)
    await checkVectorModel()
    vectorMsg.value = '✅ 模型安装成功，方案3（语义检索）已可用'
  } catch (e) {
    vectorMsg.value = `安装失败：${e}`
  } finally {
    vectorInstalling.value = false
  }
}
// 思考档位人话标签（P1：深度 1-4 → 快速/平衡/深入/认真思考）
const DEPTH_LABEL: Record<number, string> = {
  1: '⚡ 快速',
  2: '⚖️ 平衡',
  3: '🧠 深入',
  4: '🔥 认真思考',
}
// 快速选择（P1：人话分层，一键填入推荐配置）
function pickQuick(kind: 'deepseek' | 'openai' | 'local' | 'script') {
  if (kind === 'deepseek') {
    modelMode.value = 'api'
    apiBaseUrl.value = 'https://api.deepseek.com'
    apiModel.value = 'deepseek-chat'
  } else if (kind === 'openai') {
    modelMode.value = 'api'
    if (!apiBaseUrl.value) apiBaseUrl.value = 'https://api.openai.com/v1'
    if (!apiModel.value) apiModel.value = 'gpt-4o-mini'
  } else if (kind === 'local') {
    modelMode.value = 'local'
  } else {
    modelMode.value = 'script'
  }
}
const masterPwd = ref('')
const masterPwd2 = ref('')
const unlockPwd = ref('')
const cryptoMsg = ref('')
const testMsg = ref('')
const testing = ref(false)
// 管理员权限状态（null 检测中 / true 已管理员 / false 普通）
const adminState = ref<boolean | null>(null)
// —— 一键本地部署 AI ——
const ollama = ref<{ installed: boolean; models: string[] }>({ installed: false, models: [] })
const ollamaChecked = ref(false)
const pullModelName = ref('qwen2.5:3b')
const localAiMsg = ref('')
const gpuVram = ref<{ name: string; vram_mb: number }[]>([])
const modelsPath = ref('')
const pathMsg = ref('')

async function detectLocalAI() {
  try {
    ollama.value = await detectOllama()
  } catch {
    ollama.value = { installed: false, models: [] }
  }
  ollamaChecked.value = true
  await detectVramOnly()
}
async function detectVramOnly() {
  try { gpuVram.value = await detectGpuVram() } catch { gpuVram.value = [] }
}
// 部署前检查显存，不合适弹温馨提示（含「我偏不」强制继续按钮）
const vramWarn = ref(false)
const vramWarnText = ref('')
const vramForceOk = ref(false)
async function checkVram(): Promise<boolean> {
  try {
    const gpus = await detectGpuVram()
    gpuVram.value = gpus
    if (gpus.length === 0) return true
    const maxVram = Math.max(...gpus.map((g) => g.vram_mb))
    if (maxVram < 4000 && !vramForceOk.value) {
      vramWarnText.value = `你的显卡显存只有 ${(maxVram / 1024).toFixed(1)}GB，跑本地 AI 会比较吃力哦 😿`
      vramWarn.value = true
      return false
    }
    return true
  } catch {
    return true
  }
}
function vramIgnore() {
  vramForceOk.value = true
  vramWarn.value = false
  doPullModel()
}
async function doPullModel() {
  if (!pullModelName.value.trim()) return
  if (!(await checkVram())) return
  localAiMsg.value = `⏳ 正在拉取 ${pullModelName.value.trim()}…（首次可能需几分钟，取决于网速）`
  try {
    const r = await pullModel(pullModelName.value.trim())
    localAiMsg.value = `✅ ${r}`
    await detectLocalAI()
  } catch (e) {
    localAiMsg.value = `✗ ${e}`
  }
}
async function doSaveModelsPath() {
  if (!modelsPath.value.trim()) return
  try {
    pathMsg.value = await setOllamaModelsPath(modelsPath.value.trim())
  } catch (e) {
    pathMsg.value = `✗ ${e}`
  }
}

// 常见 OpenAI 兼容模型预设（可下拉选择，也可手填自定义）
const MODEL_PRESETS = [
  'deepseek-chat',
  'deepseek-reasoner',
  'qwen-plus',
  'qwen-max',
  'qwen-turbo',
  'gpt-4o-mini',
  'gpt-4o',
  'gpt-3.5-turbo',
  'moonshot-v1-8k',
  'moonshot-v1-32k',
  'glm-4',
  'glm-4-plus',
  'kimi-latest',
]

// —— 个性化 ——
const mixRate = ref(8)
const selfName = ref('铃')
const userName = ref('主人')

// —— 能力面板（大项目）：当前模型与能力矩阵 ——
// 已知模型视觉能力表（未知模型显示"未知"）
const VISION_MODELS = ['vl', 'vision', '4o', '4.1', 'llava', 'gemini', 'gpt-4', 'claude', 'qwen2.5-vl', 'qwen3-vl', 'moonshot-vision']
// 能力矩阵（computed，跟随当前模式/模型实时变化）
const abilityMatrix = computed(() => {
  const mode = setting.modelMode
  const model = (mode === 'api' ? setting.apiModel : mode === 'local' ? (ollama.value.models[0] || '') : '').toLowerCase()
  const isVision = VISION_MODELS.some((k) => model.includes(k))
  return {
    modeLabel: MODEL_MODE_LABEL[mode] || mode,
    modelName: mode === 'api' ? (setting.apiModel || '未设置') : mode === 'local' ? (ollama.value.models[0] || '未拉取模型') : '内置回复库',
    rows: [
      { name: '文字对话', ok: true, detail: '基础能力，始终可用' },
      { name: '图片理解', ok: mode === 'script' ? false : isVision, unknown: mode !== 'script' && !isVision && mode === 'api', detail: mode === 'script' ? '离线模式不支持' : isVision ? `「${model}」支持视觉` : '当前模型不支持' },
      { name: '联网能力', ok: mode !== 'script', detail: mode === 'script' ? '完全离线' : mode === 'local' ? '模型本地，联网可选' : '云端调用，需联网' },
      { name: '记忆存储', ok: true, detail: '铃的本体能力，所有模式可用' },
      { name: '工具箱', ok: true, detail: '44 个工具，所有模式可用' },
      { name: '离线可用', ok: mode !== 'api', detail: mode === 'api' ? '云端模式需联网' : '完全离线可用' },
    ],
  }
})

function syncFromStore() {
  modelMode.value = setting.modelMode
  apiBaseUrl.value = setting.apiBaseUrl ?? ''
  apiModel.value = setting.apiModel
  depth.value = setting.depth
  mixRate.value = setting.languageMixRate
  selfName.value = setting.selfName
  userName.value = setting.userName
  hotkey.value = setting.hotkey
}

onMounted(async () => {
  if (!setting.loaded) await setting.loadConfig()
  syncFromStore()
  // P3：确保陪伴记录已加载（防 App.vue 未加载完成）
  if (!milestone.loaded) await milestone.load().catch(() => {})
  autostart.value = setting.autostart
  try {
    const res = await getAutostart()
    autostart.value = res.enabled
  } catch { /* 忽略 */ }
  // 检测管理员权限
  try { adminState.value = await isAdmin() } catch { adminState.value = false }
  // 检测本地 Ollama
  await detectLocalAI()
  // 检测离线语义检索模型（方案3）
  await checkVectorModel()
})

// 以管理员权限重启
async function requestAdmin() {
  if (!confirm('将以管理员权限重启应用（会弹出 UAC 提示）。重启后可正常使用：电源模式切换、深度清理内存等特殊工具。')) return
  try {
    await restartAsAdmin()
    alert('已请求管理员权限启动。请在弹出的 UAC 窗口点击「是」，然后关闭当前窗口，使用新的管理员窗口。')
  } catch (e) {
    alert(`启动失败：${e}`)
  }
}

// 切换「始终以管理员运行」（用户自选，持久化）
async function toggleRunAsAdmin() {
  setting.runAsAdmin = !setting.runAsAdmin
  await setting.update({ run_as_admin: setting.runAsAdmin })
  generalMsg.value = setting.runAsAdmin
    ? '✓ 已开启：下次启动将以管理员身份运行（会弹一次 UAC 确认）'
    : '已关闭：将以普通权限启动'
}

// —— 通用 ——
async function toggleAutostart() {
  try {
    await setAutostart(autostart.value)
    await setting.update({ autostart: autostart.value })
    autostartMsg.value = autostart.value ? '✓ 已开启开机自启动' : '✓ 已关闭开机自启动'
  } catch (e) {
    autostartMsg.value = `✗ ${e}`
  }
  setTimeout(() => (autostartMsg.value = ''), 4000)
}
async function saveHotkey() {
  hotkeyMsg.value = ''
  try {
    const res = await registerHotkey(hotkey.value)
    await setting.update({ hotkey: hotkey.value })
    hotkeyMsg.value = res.registered ? `✓ 快捷键已生效：${res.accelerator}` : '✗ 注册失败'
  } catch (e) {
    hotkeyMsg.value = `✗ ${e}`
  }
  setTimeout(() => (hotkeyMsg.value = ''), 4000)
}
async function doReset() {
  if (!confirm('确定恢复所有设置为默认值吗？（保留主密码与已加密密钥）')) return
  await setting.resetAll()
  syncFromStore()
  generalMsg.value = '✓ 已重置为默认设置'
}
async function doExportConfig() {
  const res = await setting.exportToFile()
  exportPath.value = res.success ? `✓ 已导出：${res.path}` : `✗ ${res.error}`
}
async function doImportConfig() {
  const p = prompt('请输入要导入的 JSON 配置文件完整路径：')
  if (!p) return
  try {
    await setting.importFromFile(p)
    syncFromStore()
    generalMsg.value = `✓ 已从 ${p} 导入`
  } catch (e) {
    generalMsg.value = `✗ 导入失败：${e}`
  }
}
async function saveDataPath() {
  await setting.update({ data_path: setting.dataPath })
  generalMsg.value = '✓ 数据路径已更新（记忆将保存到新路径）'
}

// —— 模型 ——
async function saveModel() {
  try {
    await setting.update({
      model_mode: modelMode.value,
      api_base_url: apiBaseUrl.value.trim() || null,
      api_model: apiModel.value.trim() || 'gpt-3.5-turbo',
      depth: depth.value,
    })
    generalMsg.value = '✓ 模型设置已保存'
  } catch (e) {
    generalMsg.value = `✗ ${e}`
  }
}
async function saveApiKey() {
  try {
    await setting.saveApiKey(apiKeyInput.value)
    apiKeyInput.value = ''
    generalMsg.value = setting.unlocked ? '✓ API 密钥已加密保存' : '✓ API 密钥已保存（明文，建议设置主密码加密）'
  } catch (e) {
    generalMsg.value = `✗ ${e}`
  }
}
async function testConnection() {
  if (!apiBaseUrl.value.trim()) {
    testMsg.value = '⚠️ 请先填写 API 地址'
    return
  }
  testing.value = true
  testMsg.value = '⏳ 正在测试连接…'
  try {
    const res = await testApiConnection(apiBaseUrl.value.trim(), apiKeyInput.value.trim())
    testMsg.value = res.success ? `✅ ${res.message}` : `⚠️ ${res.message}`
  } catch (e) {
    testMsg.value = `✗ 测试失败：${e}`
  } finally {
    testing.value = false
  }
}
// 打开外部链接（复用后端 open_url，用系统默认浏览器）
function openExternal(url: string) {
  openUrl(url).catch((e) => alert(`打开链接失败：${e}`))
}
// —— 主密码 ——
async function doSetupMaster() {
  if (!masterPwd.value) return
  if (masterPwd.value !== masterPwd2.value) {
    cryptoMsg.value = '✗ 两次输入不一致'
    return
  }
  try {
    await setting.setupMasterPassword(masterPwd.value)
    masterPwd.value = ''
    masterPwd2.value = ''
    cryptoMsg.value = '✓ 主密码已设置并解锁'
  } catch (e) {
    cryptoMsg.value = `✗ ${e}`
  }
}
async function doUnlock() {
  try {
    await setting.unlockVault(unlockPwd.value)
    unlockPwd.value = ''
    cryptoMsg.value = '✓ 已解锁'
  } catch (e) {
    cryptoMsg.value = `✗ ${e}`
  }
}

// —— 个性化 ——
async function savePersona() {
  await setting.update({
    language_mix_rate: mixRate.value,
    self_name: selfName.value,
    user_name: userName.value,
  })
  generalMsg.value = '✓ 个性化设置已保存'
}

// —— 悬浮球设置 ——
async function saveBallSettings() {
  await setting.update({
    floating_ball_mode: setting.floatingBallMode,
    floating_ball_size: setting.floatingBallSize,
    floating_ball_opacity: setting.floatingBallOpacity,
    floating_ball_breathing: setting.floatingBallBreathing,
    floating_ball_flash: setting.floatingBallFlash,
  })
  generalMsg.value = '✓ 悬浮球设置已保存'
}
async function resetBallPosition() {
  await setting.update({ floating_ball_position: [0, 0] })
  generalMsg.value = '✓ 位置已重置（下次启动生效）'
}

// —— 外观自定义 ——
async function saveUiCustom() {
  await setting.update({
    accent_color: setting.accentColor || null,
    danger_color: setting.dangerColor || null,
    bg_color: setting.bgColor || null,
    bg_image: setting.bgImage || null,
    bubble_user_color: setting.bubbleUserColor || null,
    bubble_suzu_color: setting.bubbleSuzuColor || null,
    avatar_suzu: setting.avatarSuzu || null,
    avatar_user: setting.avatarUser || null,
    ui_radius: setting.uiRadius ?? null,
  })
  generalMsg.value = '✓ 外观自定义已保存'
  // P3：第一次自定义外观里程碑（幂等）
  milestone.record('first_custom', '第一次自定义外观').catch(() => {})
}

// —— 自定义主题组合 ——
const presetName = ref('')
async function savePreset() {
  const name = presetName.value.trim()
  if (!name) {
    generalMsg.value = '⚠️ 请先给这套风格起个名字'
    return
  }
  await setting.saveThemePreset(name)
  generalMsg.value = `✓ 已保存主题组合「${name}」，可一键切换`
  presetName.value = ''
}

// —— 软件内选择图片（背景图 / 头像）——
const bgFileInput = ref<HTMLInputElement | null>(null)
const avatarFileInput = ref<HTMLInputElement | null>(null)
const userAvatarFileInput = ref<HTMLInputElement | null>(null)
function fileToDataUrl(f: File): Promise<string> {
  return new Promise((res, rej) => {
    const r = new FileReader()
    r.onload = () => res(r.result as string)
    r.onerror = () => rej(new Error('读取图片失败'))
    r.readAsDataURL(f)
  })
}
function pickImage(kind: 'bg' | 'avatar' | 'user') {
  const el = kind === 'bg' ? bgFileInput.value : kind === 'avatar' ? avatarFileInput.value : userAvatarFileInput.value
  el?.click()
}
async function onBgFile(e: Event) {
  const input = e.target as HTMLInputElement
  const f = input.files?.[0]
  if (!f) return
  try {
    const path = await saveUiImage(await fileToDataUrl(f), 'bg')
    setting.bgImage = path
    generalMsg.value = '背景图已选择，点击「应用自定义外观」生效'
  } catch (err) {
    generalMsg.value = `选择失败：${err}`
  } finally {
    input.value = '' // 重置，允许下次选同一文件也触发
  }
}
async function onAvatarFile(e: Event) {
  const input = e.target as HTMLInputElement
  const f = input.files?.[0]
  if (!f) return
  try {
    const path = await saveUiImage(await fileToDataUrl(f), 'avatar')
    setting.avatarSuzu = path
    generalMsg.value = '头像已选择，点击「应用自定义外观」生效'
  } catch (err) {
    generalMsg.value = `选择失败：${err}`
  } finally {
    input.value = ''
  }
}
async function onUserAvatarFile(e: Event) {
  const input = e.target as HTMLInputElement
  const f = input.files?.[0]
  if (!f) return
  try {
    const path = await saveUiImage(await fileToDataUrl(f), 'user')
    setting.avatarUser = path
    generalMsg.value = '你的头像已选择，点击「应用自定义外观」生效'
  } catch (err) {
    generalMsg.value = `选择失败：${err}`
  } finally {
    input.value = ''
  }
}
// —— Emoji 显示模式（默认关闭）——
async function setEmojiMode(m: 'off' | 'partial' | 'all') {
  setting.emojiMode = m
  await setting.update({ emoji_mode: m })
  generalMsg.value = `Emoji 显示已设为：${m === 'off' ? '关闭' : m === 'partial' ? '局部' : '全部'}`
}
// —— AI 调用工具箱（默认关闭）——
async function toggleAiToolbox() {
  setting.aiToolbox = !setting.aiToolbox
  await setting.update({ ai_toolbox: setting.aiToolbox })
  generalMsg.value = setting.aiToolbox ? '✓ 已开启：铃可直接执行工具箱工具' : '已关闭：铃不调用工具箱'
}
</script>

<template>
  <div class="setting-view">
    <h3 class="page-title">⚙️ 设置</h3>

    <!-- 标签导航 -->
    <div class="tabs">
      <div v-for="t in tabs" :key="t.key" class="tab" :class="{ active: activeTab === t.key }" @click="activeTab = t.key">
        <TheIcon :name="t.icon" :size="14" class="tab-icon" />
        {{ t.label }}
      </div>
    </div>

    <div class="tab-content">
      <!-- ============ 通用 ============ -->
      <div v-if="activeTab === 'general'">
        <!-- P3：与铃的日记（陪伴记录）置顶显示 -->
        <section class="card diary-card">
          <div class="card-title">📖 与铃的日记</div>
          <template v-if="milestone.days > 0">
            <p class="diary-days">
              <span class="diary-num">{{ milestone.days }}</span>
              <span class="diary-days-label">天</span>
              <span class="diary-sub">与铃相遇{{ milestone.firstDate ? '于 ' + milestone.firstDate : '' }}</span>
            </p>
            <!-- 每日日记（1-3 天一组，折叠显示摘要行） -->
            <div class="diary-list">
              <div
                v-for="(g, gi) in diaryGroups"
                :key="g.end"
                class="diary-entry"
                :class="{ open: diaryOpen === gi }"
              >
                <div class="diary-row" @click="toggleDiary(gi)">
                  <span class="diary-date">{{ g.end }}</span>
                  <span class="diary-summary">{{ g.summary }}</span>
                  <span class="diary-arrow">{{ diaryOpen === gi ? '▴' : '▾' }}</span>
                </div>
                <div v-if="diaryOpen === gi" class="diary-body">{{ g.body }}</div>
              </div>
              <div v-if="diaryGroups.length === 0" class="diary-empty">
                第一天，故事从一句"你好"开始～
              </div>
            </div>
            <!-- 里程碑（纪念章，折叠） -->
            <div v-if="milestone.items.length > 0" class="diary-badges">
              <div class="badges-head" @click="showBadges = !showBadges">
                <span>🎖️ 纪念章（{{ milestone.items.length }}）</span>
                <span>{{ showBadges ? '▴' : '▾' }}</span>
              </div>
              <div v-if="showBadges" class="badges-body">
                <div v-for="m in milestone.items" :key="m.key" class="diary-item">
                  <span class="diary-check">✓</span>
                  <span class="diary-label">{{ m.label }}</span>
                  <span class="diary-date">{{ m.date }}</span>
                </div>
              </div>
            </div>
          </template>
          <p v-else class="diary-empty">铃正在等你开启第一段对话…</p>
        </section>

        <!-- 能力面板（大项目）：当前模型与能力矩阵 -->
        <section class="card">
          <div class="card-title">🪄 铃的能力</div>
          <p class="cap-mode">
            <span class="cap-mode-tag">{{ abilityMatrix.modeLabel }}</span>
            <span class="cap-mode-model">{{ abilityMatrix.modelName }}</span>
          </p>
          <div class="ability-list">
            <div v-for="r in abilityMatrix.rows" :key="r.name" class="ability-row">
              <span class="ability-name">{{ r.name }}</span>
              <span class="ability-state" :class="r.ok ? 'ok' : r.unknown ? 'unknown' : 'no'">
                {{ r.ok ? '✓' : r.unknown ? '?' : '✕' }}
              </span>
              <span class="ability-detail">{{ r.detail }}</span>
            </div>
          </div>
        </section>

        <section class="card">
          <div class="card-title">外观</div>
          <div class="row">
            <span class="label">主题</span>
            <button class="btn ghost" :class="{ on: setting.theme === 'dark' }" @click="setting.setTheme('dark')">深色</button>
            <button class="btn ghost" :class="{ on: setting.theme === 'light' }" @click="setting.setTheme('light')">亮色</button>
          </div>
          <div class="row theme-row">
            <span class="label">风格</span>
            <button class="btn ghost" :class="{ on: setting.theme === 'win10' }" @click="setting.setTheme('win10')">Win10</button>
            <button class="btn ghost" :class="{ on: setting.theme === 'edge' }" @click="setting.setTheme('edge')">微软浏览器</button>
            <button class="btn ghost" :class="{ on: setting.theme === 'minimal' }" @click="setting.setTheme('minimal')">极简文字</button>
            <button class="btn ghost" :class="{ on: setting.theme === 'ios-flat' }" @click="setting.setTheme('ios-flat')">iOS 扁平</button>
            <button class="btn ghost" :class="{ on: setting.theme === 'ios-glass' }" @click="setting.setTheme('ios-glass')">iOS 毛玻璃</button>
          </div>
          <div class="row theme-row">
            <span class="label">Emoji</span>
            <button class="btn ghost" :class="{ on: setting.emojiMode === 'off' }" @click="setEmojiMode('off')">关闭</button>
            <button class="btn ghost" :class="{ on: setting.emojiMode === 'partial' }" @click="setEmojiMode('partial')">局部</button>
            <button class="btn ghost" :class="{ on: setting.emojiMode === 'all' }" @click="setEmojiMode('all')">全部</button>
          </div>
          <div class="ui-custom">
            <div class="field">
              <label>主色</label>
              <div class="row">
                <input v-model="setting.accentColor" class="input" placeholder="#ff7a94" style="flex:1" />
                <input type="color" v-model="setting.accentColor" class="color-swatch" />
              </div>
              <p class="hint" style="margin-top:2px">设置主色后，铃的气泡/危险按钮自动跟随（可单独覆盖）</p>
            </div>
            <div class="field">
              <label>危险色（删除/错误按钮）</label>
              <div class="row">
                <input v-model="setting.dangerColor" class="input" placeholder="留空则跟随主色" style="flex:1" />
                <input type="color" v-model="setting.dangerColor" class="color-swatch" />
              </div>
            </div>
            <div class="field">
              <label>背景色</label>
              <div class="row">
                <input v-model="setting.bgColor" class="input" placeholder="#1d1b1f" style="flex:1" />
                <input type="color" v-model="setting.bgColor" class="color-swatch" />
              </div>
            </div>
            <div class="field">
              <label>背景图（选填）</label>
              <div class="row">
                <input v-model="setting.bgImage" class="input long" placeholder="C:\Users\...\bg.jpg" style="flex:1" />
                <button class="btn ghost" @click="pickImage('bg')">选择图片</button>
              </div>
              <input ref="bgFileInput" type="file" accept="image/*" style="display:none" @change="onBgFile" />
            </div>
            <div class="field">
              <label>铃的头像（emoji / 文字 / 图片）</label>
              <div class="row">
                <input v-model="setting.avatarSuzu" class="input" placeholder="铃" style="flex:1" />
                <button class="btn ghost" @click="pickImage('avatar')">选头像图</button>
              </div>
              <input ref="avatarFileInput" type="file" accept="image/*" style="display:none" @change="onAvatarFile" />
            </div>
            <div class="field">
              <label>你的头像（选填）</label>
              <div class="row">
                <input v-model="setting.avatarUser" class="input" placeholder="（留空则不显示）" style="flex:1" />
                <button class="btn ghost" @click="pickImage('user')">选头像图</button>
              </div>
              <input ref="userAvatarFileInput" type="file" accept="image/*" style="display:none" @change="onUserAvatarFile" />
            </div>
            <div class="field">
              <label>圆角：{{ setting.uiRadius ?? 12 }}px</label>
              <input v-model.number="setting.uiRadius" type="range" min="0" max="24" class="range" />
            </div>
            <div class="field">
              <label>你的气泡颜色</label>
              <div class="row">
                <input v-model="setting.bubbleUserColor" class="input" placeholder="#2d2d2d" style="flex:1" />
                <input type="color" v-model="setting.bubbleUserColor" class="color-swatch" />
              </div>
            </div>
            <div class="field">
              <label>铃的气泡颜色</label>
              <div class="row">
                <input v-model="setting.bubbleSuzuColor" class="input" placeholder="#3a3438" style="flex:1" />
                <input type="color" v-model="setting.bubbleSuzuColor" class="color-swatch" />
              </div>
            </div>
            <button class="btn primary" @click="saveUiCustom">应用自定义外观</button>
            <div class="theme-presets">
              <div class="row">
                <input v-model="presetName" class="input" placeholder="给这套风格起个名字" style="flex:1" />
                <button class="btn primary" @click="savePreset">保存组合</button>
              </div>
              <p class="hint">保存后一键切换你的专属风格，无需复制粘贴</p>
              <div v-if="setting.uiThemes && setting.uiThemes.length" class="preset-list">
                <div v-for="t in setting.uiThemes" :key="t.name" class="preset-item">
                  <span class="preset-name">{{ t.name }}</span>
                  <button class="btn ghost" @click="setting.switchThemePreset(t.name)">切换</button>
                  <button class="btn ghost" @click="setting.deleteThemePreset(t.name)">删除</button>
                </div>
              </div>
            </div>
          </div>
        </section>

        <section class="card">
          <div class="card-title">管理员权限</div>
          <p class="hint">
            当前状态：{{ adminState === null ? '检测中…' : adminState ? '✅ 已以管理员权限运行' : '普通权限（电源模式切换 / 深度清理内存等特殊工具需管理员）' }}
          </p>
          <label class="switch-wrap">
            <input type="checkbox" :checked="setting.runAsAdmin" @change="toggleRunAsAdmin" class="switch" />
            <span class="label">启动时始终以管理员运行（自动提权）</span>
          </label>
          <p class="hint">开启后每次启动自动提权（弹一次 UAC 确认），适合常需要管理员工具的深度用户；无需可关闭。</p>
          <label class="switch-wrap" style="margin-top:10px">
            <input type="checkbox" :checked="setting.aiToolbox" @change="toggleAiToolbox" class="switch" />
            <span class="label">允许 AI 调用工具箱工具</span>
          </label>
          <p class="hint">开启后，铃可直接执行工具箱工具（如清理内存、ping、截图、查 IP 等），默认关闭。</p>
          <div v-if="adminState === false" class="row">
            <button class="btn primary" @click="requestAdmin">以管理员权限重启</button>
          </div>
        </section>

        <section class="card">
          <div class="card-title">开机自启动</div>
          <label class="switch-wrap">
            <input v-model="autostart" type="checkbox" class="switch" @change="toggleAutostart" />
            <span class="label">{{ autostart ? '已开启' : '已关闭' }}</span>
          </label>
          <div v-if="autostartMsg" class="msg">{{ autostartMsg }}</div>
        </section>

        <section class="card">
          <div class="card-title">全局快捷键</div>
          <div class="row">
            <input v-model="hotkey" class="input" placeholder="Ctrl+Alt+L" />
            <button class="btn primary" @click="saveHotkey">应用</button>
          </div>
          <div v-if="hotkeyMsg" class="msg">{{ hotkeyMsg }}</div>
        </section>

        <section class="card">
          <div class="card-title">数据路径</div>
          <div class="row">
            <input v-model="setting.dataPath" class="input long" placeholder="记忆存储路径" />
            <button class="btn primary" @click="saveDataPath">保存</button>
          </div>
          <p class="hint">当前：{{ setting.dataPath || '（未设置，默认文档/铃记忆体）' }}</p>
        </section>

        <section class="card">
          <div class="card-title">配置备份</div>
          <div class="row">
            <button class="btn ghost" @click="doExportConfig">导出配置 JSON</button>
            <button class="btn ghost" @click="doImportConfig">导入配置</button>
          </div>
          <div v-if="exportPath" class="msg">{{ exportPath }}</div>
        </section>

        <section class="card">
          <div class="card-title">重置</div>
          <button class="btn danger" @click="doReset">恢复默认设置</button>
        </section>

        <section class="card">
          <div class="card-title">工具箱</div>
          <button class="btn primary" @click="showToolbox = !showToolbox">
            {{ showToolbox ? '收起工具箱' : '打开工具箱' }}
          </button>
        </section>
      </div>

      <!-- ============ 模型 ============ -->
      <div v-else-if="activeTab === 'model'">
        <!-- 快速接入向导（3 步） -->
        <section class="card">
          <div class="card-title">快速接入向导</div>
          <div class="steps">
            <span class="step" :class="{ on: modelMode !== 'script' }">① 选运行模式</span>
            <span class="arrow">→</span>
            <span class="step" :class="{ on: modelMode === 'api' && !!apiBaseUrl }">② 填地址与模型</span>
            <span class="arrow">→</span>
            <span class="step" :class="{ on: testMsg.includes('✅') }">③ 测试连接</span>
          </div>
          <p class="hint">
            {{ modelMode === 'script' ? '当前：脚本模式（开箱即用，预设回复库，无需联网）' :
               modelMode === 'api' ? '当前：API 模式（接入 OpenAI 兼容大模型，如 DeepSeek / Qwen / GPT）' :
               '当前：本地模式（需自行安装 Ollama 并下载模型）' }}
          </p>
        </section>

        <!-- 快速选择（P1：人话分层，点卡片自动填推荐配置） -->
        <section class="card">
          <div class="card-title">选择铃的运行方式</div>
          <div class="quick-modes">
            <div class="qmode" :class="{ sel: modelMode === 'api' && apiBaseUrl.includes('deepseek') }" @click="pickQuick('deepseek')">
              <div class="qmode-icon">☁️</div>
              <div class="qmode-name">DeepSeek<span class="qmode-tag">推荐</span></div>
              <div class="qmode-desc">云端智能，性价比高，中文好</div>
            </div>
            <div class="qmode" :class="{ sel: modelMode === 'local' }" @click="pickQuick('local')">
              <div class="qmode-icon">💻</div>
              <div class="qmode-name">本地 AI</div>
              <div class="qmode-desc">模型跑在自己电脑，离线可用</div>
            </div>
            <div class="qmode" :class="{ sel: modelMode === 'script' }" @click="pickQuick('script')">
              <div class="qmode-icon">📴</div>
              <div class="qmode-name">离线模式</div>
              <div class="qmode-desc">零配置，内置回复库，即开即用</div>
            </div>
          </div>
          <p class="hint">点击卡片会自动切换模式并填入推荐配置；下方「运行模式」可手调高级参数。</p>
          <p class="hint" style="margin-top:6px;color:var(--text-secondary)">
            💡 OpenAI 兼容：支持任意 OpenAI 兼容服务（中转站 / Qwen / 混元等）。在下方「运行模式」填 API 地址即可；DeepSeek 卡片已填好官方示例，无需联网的海外 OpenAI 官方（需翻墙+付费 key）不做默认推荐。
          </p>
        </section>

        <section class="card">
          <div class="card-title">运行模式<span class="card-sub">高级参数</span></div>
          <div class="modes">
            <div class="mode" :class="{ sel: modelMode === 'script' }" @click="modelMode = 'script'">离线</div>
            <div class="mode" :class="{ sel: modelMode === 'api' }" @click="modelMode = 'api'">云端</div>
            <div class="mode" :class="{ sel: modelMode === 'local' }" @click="modelMode = 'local'">本地 AI</div>
          </div>
          <template v-if="modelMode === 'api'">
            <div class="field">
              <label>API 地址（OpenAI 兼容，带/不带 /v1 均可）</label>
              <input v-model="apiBaseUrl" class="input long" placeholder="https://api.deepseek.com" />
            </div>
            <div class="field">
              <label>模型名（可选预设，也可手填）</label>
              <input v-model="apiModel" class="input long" list="model-presets" placeholder="deepseek-chat / qwen-plus / gpt-4o-mini…" />
              <datalist id="model-presets">
                <option v-for="m in MODEL_PRESETS" :key="m" :value="m" />
              </datalist>
            </div>
            <div class="row">
              <button class="btn ghost" :disabled="testing" @click="testConnection">
                {{ testing ? '⏳ 测试中…' : '🔌 测试连接' }}
              </button>
              <button class="btn ghost" @click="openExternal('https://platform.deepseek.com')">🔑 获取 DeepSeek API Key</button>
            </div>
            <div v-if="testMsg" class="msg">{{ testMsg }}</div>
          </template>
          <template v-if="modelMode !== 'script'">
            <div class="field">
              <label>回复速度</label>
              <input v-model.number="depth" type="range" min="1" max="4" step="1" class="range" />
              <div class="depth-labels">
                <span>⚡ 快速</span><span>⚖️ 平衡</span><span>🧠 深入</span><span>🔥 认真思考</span>
              </div>
              <span class="label">当前：{{ DEPTH_LABEL[depth as keyof typeof DEPTH_LABEL] ?? depth }}</span>
            </div>
            <button class="btn primary" @click="saveModel">保存模型设置</button>
            <p class="hint key-guide">
              <span class="key-guide-arrow">⬇</span> API 密钥在下方「API 密钥」卡片填写，填完回到这里点保存
            </p>
          </template>
        </section>

        <!-- 一键本地部署 AI -->
        <section class="card">
          <div class="card-title">一键本地部署 AI（Ollama）</div>
          <p class="hint">
            {{ !ollamaChecked ? '检测中…' : ollama.installed ? `✅ Ollama 已安装（${ollama.models.length} 个模型）` : '❌ 未检测到 Ollama，需先安装' }}
            <button v-if="ollamaChecked" class="btn ghost" style="margin-left:8px;padding:2px 10px" @click="detectLocalAI">重新检测</button>
          </p>
          <template v-if="ollamaChecked && ollama.installed">
            <p v-if="ollama.models.length" class="hint">已装模型：{{ ollama.models.join('、') }}</p>
            <p v-else class="hint">还没有模型，下拉一个开始用：</p>
            <p v-if="gpuVram.length" class="hint">🖥️ 显卡：{{ gpuVram.map((g) => `${g.name}（${(g.vram_mb / 1024).toFixed(1)}GB）`).join('、') }}</p>
            <div class="row">
              <input v-model="pullModelName" class="input long" placeholder="qwen2.5:3b" />
              <button class="btn primary" @click="doPullModel">⬇️ 一键拉取模型</button>
            </div>
            <div v-if="localAiMsg" class="msg">{{ localAiMsg }}</div>

            <!-- 显存不足弹窗（含「我偏不」强制继续） -->
            <div v-if="vramWarn" class="modal-mask" @click.self="vramWarn = false">
              <div class="modal vram-modal">
                <div class="modal-title">🖥️ 显存不足提醒</div>
                <p class="modal-body">{{ vramWarnText }}</p>
                <p class="modal-tip">建议改用轻量模型（qwen2.5:3b 或更小），或切换 API 模式更流畅。</p>
                <div class="modal-actions">
                  <button class="btn ghost" @click="vramWarn = false">改用 API 模式</button>
                  <button class="btn primary" @click="vramIgnore">我偏不，老子电脑很牛逼 🔥</button>
                </div>
              </div>
            </div>
            <div class="row" style="margin-top:10px">
              <input v-model="modelsPath" class="input long" placeholder="模型存储路径（可选，如 D:\ollama-models）" />
              <button class="btn ghost" @click="doSaveModelsPath">保存路径</button>
            </div>
            <div v-if="pathMsg" class="msg">{{ pathMsg }}</div>
          </template>
          <template v-else-if="ollamaChecked && !ollama.installed">
            <div class="row">
              <button class="btn primary" @click="openExternal('https://ollama.com/download')">🌐 打开 Ollama 官网下载</button>
            </div>
            <p class="hint" style="margin-top:6px">安装 Ollama 后回到本页点「重新检测」，再一键拉取模型。推荐 <b>qwen2.5:3b</b>（轻量）或 <b>qwen2.5:7b</b>（均衡）。</p>
          </template>
        </section>

        <!-- 离线语义检索模型（方案3） -->
        <section class="card">
          <div class="card-title">离线语义检索模型（bge）</div>
          <p class="hint">离线模式下让「铃」更聪明地选回复（语义匹配，比关键词更懂你）。模型约 91MB，完整版内置；轻量版可一键检索安装。</p>
          <div v-if="vectorModelStatus" class="hint">
            {{ vectorModelStatus.available ? '✅ ' + vectorModelStatus.message : '❌ ' + vectorModelStatus.message }}
          </div>
          <div class="row" style="margin-top:8px">
            <button class="btn ghost" :disabled="vectorScanning" @click="scanVectorModels">
              {{ vectorScanning ? '⏳ 检索中…' : '🔍 检索模型' }}
            </button>
          </div>
          <div v-if="vectorCandidates.length" class="card" style="margin-top:8px;background:var(--input-bg)">
            <div v-for="c in vectorCandidates" :key="c.path" class="row" style="margin-top:6px">
              <span class="hint" style="flex:1">{{ c.filename }}（{{ (c.size_mb).toFixed(1) }}MB）</span>
              <button v-if="c.exists_in_target" class="btn ghost" disabled>已安装</button>
              <button v-else class="btn primary" :disabled="vectorInstalling" @click="doInstallVectorModel(c.path)">
                {{ vectorInstalling ? '安装中…' : '一键安装' }}
              </button>
            </div>
          </div>
          <div v-if="vectorMsg" class="msg">{{ vectorMsg }}</div>
        </section>

        <section class="card">
          <div class="card-title">API 密钥</div>
          <p class="hint">
            主密码状态：{{ setting.hasMasterPassword ? (setting.unlocked ? '已设置 · 已解锁 ✅' : '已设置 · 未解锁') : '未设置 ⚠️（密钥将明文存储，建议设置主密码加密）' }}
          </p>
          <div class="field">
            <label>{{ setting.unlocked ? '新密钥（加密存储，主密码保护）' : '新密钥（当前明文存储，不设主密码也可用）' }}</label>
            <div class="row">
              <input v-model="apiKeyInput" type="password" class="input long" placeholder="sk-..." />
              <button class="btn primary" @click="saveApiKey">{{ setting.unlocked ? '加密保存' : '保存密钥' }}</button>
            </div>
          </div>

          <div class="field" style="margin-top: 12px">
            <label>主密码</label>
            <template v-if="!setting.hasMasterPassword">
              <div class="row">
                <input v-model="masterPwd" type="password" class="input" placeholder="设置主密码" />
                <input v-model="masterPwd2" type="password" class="input" placeholder="确认主密码" />
                <button class="btn primary" @click="doSetupMaster">设置</button>
              </div>
            </template>
            <template v-else-if="!setting.unlocked">
              <div class="row">
                <input v-model="unlockPwd" type="password" class="input" placeholder="输入主密码解锁" />
                <button class="btn primary" @click="doUnlock">解锁</button>
              </div>
            </template>
            <template v-else>
              <div class="row">
                <input v-model="masterPwd" type="password" class="input" placeholder="新主密码（修改）" />
                <input v-model="masterPwd2" type="password" class="input" placeholder="确认新主密码" />
                <button class="btn ghost" @click="doSetupMaster">修改</button>
              </div>
            </template>
          </div>
          <div v-if="cryptoMsg" class="msg">{{ cryptoMsg }}</div>
        </section>
      </div>

      <!-- ============ 记忆 ============ -->
      <div v-else-if="activeTab === 'memory'">
        <section class="card">
          <div class="card-title">记忆存储</div>
          <p class="hint">记忆文件保存在：{{ setting.dataPath }}</p>
          <p class="hint">记忆集管理与详情请使用主界面「记忆」面板（由 AI-4 实现）。</p>
          <div class="row">
            <button class="btn ghost" @click="doExportConfig">导出记忆配置</button>
            <button class="btn ghost" @click="doImportConfig">导入记忆配置</button>
          </div>
        </section>
      </div>

      <!-- ============ 监测 ============ -->
      <div v-else-if="activeTab === 'monitor'">
        <MonitorSettings />
      </div>

      <!-- ============ 形象互联（VBIL） ============ -->
      <div v-else-if="activeTab === 'vbil'">
        <VBILSettings />
      </div>

      <!-- ============ 悬浮球 ============ -->
      <div v-else-if="activeTab === 'ball'">
        <section class="card">
          <div class="card-title">悬浮球</div>
          <div class="field">
            <label>显示模式</label>
            <select v-model="setting.floatingBallMode" @change="saveBallSettings">
              <option value="avatar">头像</option>
              <option value="simple">纯文字</option>
              <option value="live2d">Live2D（实验）</option>
            </select>
          </div>
          <div class="field">
            <label>大小</label>
            <div class="size-btns">
              <button v-for="s in [120, 160, 200, 260]" :key="s" class="size-btn" :class="{ active: setting.floatingBallSize === s }" @click="setting.floatingBallSize = s; saveBallSettings()">{{ s }}</button>
            </div>
          </div>
          <div class="field">
            <label>透明度（{{ (setting.floatingBallOpacity * 100).toFixed(0) }}%）</label>
            <input v-model.number="setting.floatingBallOpacity" type="range" min="0.1" max="1" step="0.05" class="range" @change="saveBallSettings" />
          </div>
          <div class="row">
            <label class="switch-wrap">
              <span class="label">呼吸动画</span>
              <input type="checkbox" class="switch" v-model="setting.floatingBallBreathing" @change="saveBallSettings" />
            </label>
            <label class="switch-wrap">
              <span class="label">消息闪烁</span>
              <input type="checkbox" class="switch" v-model="setting.floatingBallFlash" @change="saveBallSettings" />
            </label>
          </div>
          <div class="actions">
            <button class="btn ghost" @click="resetBallPosition">重置位置</button>
          </div>
        </section>
      </div>

      <!-- ============ 个性化 ============ -->
      <div v-else-if="activeTab === 'persona'">
        <section class="card">
          <div class="card-title">个性化</div>
          <div class="field">
            <label>日语修饰词浓度（0-30）</label>
            <input v-model.number="mixRate" type="range" min="0" max="30" class="range" />
            <span class="label">当前：{{ mixRate }}（{{ mixRate < 5 ? '低' : mixRate < 15 ? '中' : '高' }}）</span>
          </div>
          <div class="row">
            <div class="field">
              <label>自称</label>
              <input v-model="selfName" class="input" />
            </div>
            <div class="field">
              <label>对您的称呼</label>
              <input v-model="userName" class="input" />
            </div>
          </div>
          <button class="btn primary" @click="savePersona">保存个性化</button>
        </section>
      </div>

      <!-- ============ 插件 ============ -->
      <div v-else-if="activeTab === 'plugin'">
        <PluginManager />
      </div>

      <!-- ============ 同步 ============ -->
      <div v-else-if="activeTab === 'sync'">
        <SyncPanel />
      </div>

      <!-- ============ 快捷指令（AI-9） ============ -->
      <div v-else-if="activeTab === 'quick'">
        <QuickCommandPanel />
      </div>

      <!-- ============ 赞助 ============ -->
      <div v-else-if="activeTab === 'sponsor'">
        <section class="card">
          <div class="card-title">赞助</div>
          <p class="hint">支持铃·记忆体开源项目。赞助渠道、目标进度、赞助者名单在此展示（预留）。</p>
          <div class="row">
            <button class="btn primary" @click="openExternal('https://github.com/Zang7311')">访问 GitHub 主页</button>
            <button class="btn ghost" @click="openExternal('https://github.com/Zang7311/-Memoria')">项目仓库</button>
          </div>
        </section>
      </div>

      <!-- ============ 诊断 ============ -->
      <div v-else-if="activeTab === 'diag'">
        <DiagnosticView />
      </div>
    </div>

    <div v-if="generalMsg" class="msg global">{{ generalMsg }}</div>

    <!-- 工具箱悬浮窗（AI-6） -->
    <ToolboxPanel v-if="showToolbox" @close="showToolbox = false" />
  </div>
</template>

<style scoped>
.setting-view {
  padding: 20px 24px;
  max-width: 820px;
  color: var(--text-main, #eee6e7);
  overflow-y: auto;
  height: 100%;
  box-sizing: border-box;
}
.page-title { margin: 0 0 16px; font-size: var(--fs-18); }
.tabs { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 16px; }
.tab {
  padding: 7px 13px; border-radius: 20px; cursor: pointer; font-size: var(--fs-13);
  background: rgba(128, 128, 128, 0.12); color: var(--text-secondary);
  transition: all 0.15s;
}
.tab.active { background: var(--accent, #ff7a94); color: var(--text-user); }
.tab-icon { margin-right: 3px; }
.tab-content { display: flex; flex-direction: column; gap: 4px; }
.card {
  background: var(--bg-bar, rgba(34, 32, 36, 0.85));
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  border-radius: 14px; padding: 16px; margin-bottom: 12px;
}
.card-title { font-weight: 600; font-size: var(--fs-14); margin-bottom: 10px; }
.hint { font-size: var(--fs-12); color: var(--text-secondary); margin: 0 0 8px; line-height: 1.6; }
.row { display: flex; align-items: center; gap: 10px; margin-bottom: 8px; flex-wrap: wrap; }
.label { font-size: var(--fs-13); color: var(--text-secondary); }
.field { display: flex; flex-direction: column; gap: 4px; margin-bottom: 10px; }
.field label { font-size: var(--fs-12); color: var(--text-secondary); }
.input {
  padding: 7px 10px; border-radius: 8px; border: 1px solid var(--border);
  background: var(--input-bg); color: var(--text-main); font-size: var(--fs-13);
}
.input.long { flex: 1; min-width: 220px; }
.range { width: 100%; accent-color: var(--accent, #ff7a94); }
.size-btns { display: flex; gap: 8px; margin-top: 6px; }
.size-btn {
  flex: 1;
  padding: 6px 0;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--input-bg);
  color: var(--text-main);
  font-size: var(--fs-13);
  cursor: pointer;
  transition: all 0.15s;
}
.size-btn.active {
  background: var(--accent, #ff7a94);
  color: var(--text-user);
  border-color: transparent;
}
.depth-labels {
  display: flex;
  justify-content: space-between;
  font-size: var(--fs-10);
  color: var(--text-secondary);
  margin-top: 2px;
}
.modes { display: flex; gap: 8px; margin-bottom: 12px; }
/* —— 快速选择（P1）样式 —— */
.quick-modes {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  margin-bottom: 8px;
}
.qmode {
  border: 2px solid var(--border, rgba(128, 128, 128, 0.25));
  border-radius: 12px;
  padding: 10px 12px;
  cursor: pointer;
  transition: all 0.15s;
  text-align: left;
  background: var(--input-bg, transparent);
}
.qmode:hover { border-color: var(--accent, #ff7a94); }
.qmode.sel { border-color: var(--accent, #ff7a94); background: rgba(255, 122, 148, 0.08); }
.qmode-icon { font-size: var(--fs-18); }
.qmode-name { font-weight: 700; font-size: var(--fs-13); margin: 2px 0; }
.qmode-tag {
  display: inline-block; margin-left: 6px; padding: 0 6px; border-radius: 7px;
  font-size: var(--fs-10); font-weight: 600; background: var(--accent, #ff7a94); color: #fff;
  vertical-align: 1px;
}
.qmode-desc { font-size: var(--fs-10); color: var(--text-secondary); }
.card-sub { font-size: var(--fs-10); color: var(--text-secondary); font-weight: 400; margin-left: 6px; }
/* 密钥位置引导（P1）：小字 + 向下箭头 */
.key-guide {
  margin-top: 8px;
  font-size: var(--fs-11);
  color: var(--text-secondary);
  display: flex;
  align-items: center;
  gap: 4px;
}
/* —— P3：与铃的日记样式 —— */
.diary-card { background: linear-gradient(160deg, rgba(255, 122, 148, 0.06), rgba(201, 228, 255, 0.06)); }
.diary-days { display: flex; align-items: baseline; gap: 6px; margin: 6px 0 10px; }
.diary-num { font-size: var(--fs-30); font-weight: 800; color: var(--accent, #ff7a94); line-height: 1; }
.diary-days-label { font-size: var(--fs-14); color: var(--text-main); font-weight: 600; }
.diary-sub { font-size: var(--fs-11); color: var(--text-secondary); }
.diary-list { display: flex; flex-direction: column; gap: 4px; }
/* —— 能力面板（大项目）样式 —— */
.cap-mode {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}
.cap-mode-tag {
  font-size: var(--fs-11);
  font-weight: 600;
  padding: 2px 10px;
  border-radius: 10px;
  background: var(--accent, #ff7a94);
  color: #fff;
}
.cap-mode-model {
  font-size: var(--fs-11);
  color: var(--text-secondary);
  font-family: var(--font-mono);
  word-break: break-all;
}
.ability-list { display: flex; flex-direction: column; gap: 2px; }
.ability-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 6px;
  border-radius: 6px;
  font-size: var(--fs-11);
}
.ability-row:hover { background: rgba(128, 128, 128, 0.08); }
.ability-name { width: 70px; color: var(--text-main); flex-shrink: 0; }
.ability-state {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: var(--fs-10);
  font-weight: 700;
  flex-shrink: 0;
}
.ability-state.ok { background: rgba(76, 175, 80, 0.15); color: var(--success, #4caf50); }
.ability-state.no { background: rgba(217, 83, 79, 0.15); color: var(--danger, #d9534f); }
.ability-state.unknown { background: rgba(240, 173, 78, 0.15); color: var(--warning, #f0ad4e); }
.ability-detail { color: var(--text-secondary); flex: 1; }
/* —— 每日日记条目（折叠） —— */
.diary-entry {
  border-radius: 10px;
  background: rgba(128, 128, 128, 0.08);
  overflow: hidden;
}
.diary-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  cursor: pointer;
  font-size: var(--fs-12);
  user-select: none;
}
.diary-row:hover { background: rgba(128, 128, 128, 0.08); }
.diary-date { font-weight: 700; color: var(--accent, #ff7a94); flex-shrink: 0; }
.diary-summary {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-main);
}
.diary-arrow { font-size: var(--fs-10); color: var(--text-secondary); flex-shrink: 0; }
.diary-body {
  padding: 6px 10px 8px;
  font-size: var(--fs-11);
  color: var(--text-secondary);
  line-height: 1.7;
  border-top: 1px dashed var(--border, rgba(128, 128, 128, 0.2));
}
/* —— 纪念章（折叠） —— */
.diary-badges { margin-top: 8px; border-top: 1px dashed var(--border, rgba(128, 128, 128, 0.25)); padding-top: 6px; }
.badges-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
  font-size: var(--fs-11);
  color: var(--text-secondary);
  user-select: none;
  padding: 2px 4px;
}
.badges-body { margin-top: 4px; display: flex; flex-direction: column; gap: 4px; }
.diary-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 10px;
  border-radius: 8px;
  background: rgba(128, 128, 128, 0.08);
  font-size: var(--fs-12);
}
.diary-check { color: var(--success, #4caf50); font-weight: 700; }
.diary-label { flex: 1; color: var(--text-main); }
.diary-date { font-size: var(--fs-10); color: var(--text-secondary); }
.diary-empty { font-size: var(--fs-12); color: var(--text-secondary); padding: 8px 0; }
.key-guide-arrow {
  color: var(--accent, #ff7a94);
  font-size: var(--fs-14);
  animation: key-bounce 1.6s ease-in-out infinite;
}
@keyframes key-bounce {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(3px); }
}
.steps { display: flex; align-items: center; gap: 6px; margin-bottom: 8px; flex-wrap: wrap; }
.step { font-size: var(--fs-12); padding: 3px 9px; border-radius: 12px; background: rgba(128, 128, 128, 0.14); color: var(--text-secondary); }
.step.on { background: var(--accent, #ff7a94); color: var(--text-user); }
.arrow { color: var(--text-secondary); font-size: var(--fs-12); }
.mode {
  padding: 8px 16px; border-radius: 10px; cursor: pointer; border: 1px solid var(--border);
  font-size: var(--fs-13); background: transparent;
}
.mode.sel { border-color: var(--accent); background: var(--accent); color: var(--text-user); }
.vram-modal { width: 360px; text-align: left; }
.modal-body { font-size: var(--fs-13); line-height: 1.6; margin: 0 0 8px; }
.modal-tip { font-size: var(--fs-12); color: var(--text-secondary, #999); margin: 0 0 14px; line-height: 1.6; }
.btn { padding: 6px 14px; border-radius: 8px; border: none; cursor: pointer; font-size: var(--fs-13); }
.btn.primary { background: var(--accent, #ff7a94); color: var(--text-user); }
.btn.ghost { background: rgba(128, 128, 128, 0.18); color: var(--text-main); }
.btn.on { border: 1px solid var(--accent); color: var(--accent); }
.theme-row { flex-wrap: wrap; gap: 6px; }
.ui-custom { display: flex; flex-direction: column; gap: 10px; margin-top: 14px; padding-top: 14px; border-top: 1px dashed var(--border, rgba(128,128,128,0.25)); }
.color-swatch { width: 42px; height: 34px; padding: 0; border: 1px solid var(--border, rgba(128,128,128,0.3)); border-radius: 6px; background: transparent; cursor: pointer; }
.theme-presets { margin-top: 14px; padding-top: 14px; border-top: 1px dashed var(--border, rgba(128,128,128,0.25)); display: flex; flex-direction: column; gap: 8px; }
.preset-list { display: flex; flex-direction: column; gap: 6px; max-height: 200px; overflow-y: auto; }
.preset-item { display: flex; align-items: center; gap: 8px; background: rgba(128,128,128,0.1); padding: 6px 10px; border-radius: 8px; }
.preset-name { flex: 1; font-size: var(--fs-13); color: var(--text-main); }
.btn.danger { background: var(--danger-bg); color: var(--danger, #ff6b6b); }
.btn:disabled { opacity: 0.5; cursor: not-allowed; }
.switch-wrap { display: flex; align-items: center; gap: 8px; }
.switch { width: 40px; height: 20px; accent-color: var(--accent, #ff7a94); }
.msg { font-size: var(--fs-12); margin-top: 6px; color: var(--accent, #ff7a94); }
.msg.global { margin-top: 12px; }
</style>
