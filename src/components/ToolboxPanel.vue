<!-- 《铃·记忆体》工具箱面板：抽屉内流式布局，工具网格自适应列数，弹窗统一用令牌 -->
<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useDesktopStore } from '../stores/desktopStore'
import { useSettingStore } from '../stores/settingStore'
import { useMilestoneStore } from '../stores/milestoneStore'
import { assetUrl, checkDependency, decodeQrcode, deleteForgedTool, generateQrcode, getMemories, listForgedTools, ocrImage, openUrl, setForgedToolEnabled } from '../utils/tauri'
import type { ForgedTool, ToolboxItem } from '../types'
import TheIcon from './TheIcon.vue'
import AppButton from './ui/AppButton.vue'
import AppTabs from './ui/AppTabs.vue'
import AppStatusDot from './ui/AppStatusDot.vue'
import AppInput from './ui/AppInput.vue'
import PixelArtPanel from './PixelArtPanel.vue'
import RegexTester from './RegexTester.vue'
import { confirmDialog } from '../utils/dialog'
import { toast } from '../utils/toast'

const desktop = useDesktopStore()
const setting = useSettingStore()
const milestone = useMilestoneStore()
const forgedTools = ref<ForgedTool[]>([])
const expandedForgedId = ref<string | null>(null)

// 关闭由抽屉标签条处理；仍声明 close，避免父组件的监听落到根元素上
defineEmits<{ (e: 'close'): void }>()

// —— 内置工具 id → 图标名（只用仓库里的 SVG 图标，不用 emoji）——
const TOOL_ICONS: Record<string, string> = {
  explorer: 'file', 'open-file': 'file', 'open-app': 'search', 'open-cmd': 'command', 'open-powershell': 'command',
  calc: 'calculator', notepad: 'edit', 'batch-rename': 'edit', taskmgr: 'monitor', bsod: 'monitor', vm: 'monitor',
  lock: 'lock', 'wifi-pwd': 'lock', encrypt: 'lock', decrypt: 'lock', xor: 'lock',
  'shutdown-1h': 'power', 'cancel-shutdown': 'power', 'power-saver': 'power', 'power-balanced': 'power', 'power-high': 'power',
  'clean-temp': 'clean', 'clean-memory': 'clean', 'empty-recycle-bin': 'delete', shred: 'delete', 'format-disk': 'delete',
  screenshot: 'screenshot', network: 'network', 'lan-ip': 'network', ping: 'network', speedtest: 'network', 'port-list': 'network',
  'ip-lookup': 'search', regex: 'search', ocr: 'search', 'qrcode-decode': 'search',
  'cpu-test': 'diagnostic', benchmark: 'diagnostic', clicker: 'floating', 'pixel-art': 'customize',
  'clipboard-history': 'file', alarm: 'music', ffmpeg: 'music', fortune: 'sponsor',
  compress: 'save', uncompress: 'save', 'qrcode-gen': 'save',
  'install-python': 'plugin', 'install-jdk': 'plugin',
}
function toolIcon(id: string) { return TOOL_ICONS[id] ?? 'toolbox' }

const executingId = ref<string | null>(null)
const feedback = ref<{ ok: boolean; text: string } | null>(null)
// —— D 批次：执行中"铃正在努力"动态提示 ——
const executingText = ref<string | null>(null)
const phaseText = ref<string | null>(null) // 真实阶段文本（有阶段输出的工具用）
const phaseDots = ref('')
let phaseTimer: ReturnType<typeof setInterval> | undefined
// —— 记忆×工具联动（概率事件，非彩蛋）：中等概率触发，覆盖常用工具 ——
const memoryEcho = ref<string | null>(null)
// 工具 id → (记忆关键词正则, 提起文案)；执行成功时 35% 概率检查记忆命中
const TOOL_MEMORY_RULES: { id: string; kw: RegExp; text: string }[] = [
  { id: 'clean-memory', kw: /卡|卡顿|慢|内存|清理|释放/, text: '（铃探头看了看）主人之前说电脑有点卡，这次铃帮你清理得干干净净啦～' },
  { id: 'screenshot', kw: /截图|截屏|录屏|屏幕/, text: '（铃眨眨眼）主人上次也截过图呢，这次截到什么好东西啦？' },
  { id: 'volume', kw: /音量|声音|太吵|太小声|静音/, text: '（铃竖起耳朵）音量调好啦～主人之前说声音不舒服，铃记着呢！' },
  { id: 'music', kw: /音乐|歌|唱歌|听歌|乐队/, text: '（铃轻轻哼了一声）主人上次聊到喜欢的歌，铃还记在记忆里哦～' },
  { id: 'network', kw: /网速|网络|wifi|宽带|断网|连接/, text: '（铃看了看信号）网络弄好啦！主人上次说网卡，铃这次帮你盯紧啦～' },
  { id: 'lock', kw: /锁屏|隐私|离开|安全/, text: '（铃认真点头）锁好啦！主人的隐私最重要，铃会守着的～' },
  { id: 'calculator', kw: /数学|计算|算数|数字/, text: '（铃掰了掰手指）算好了！主人之前也问过类似的计算，铃还记得呢～' },
]
// 记忆×工具联动检查：35% 概率 + 记忆关键词命中 → 显示铃的提起
async function checkToolMemoryEcho(itemId: string) {
  memoryEcho.value = null
  try {
    const rule = TOOL_MEMORY_RULES.find((r) => r.id === itemId)
    if (!rule) return
    // 中等概率（35%）
    const nanos = Date.now()
    if (nanos % 100 > 35) return
    const resp = await getMemories({ limit: 100 })
    const hit = resp.memories.some((m) => m.role === 'user' && rule.kw.test(m.content))
    if (hit) memoryEcho.value = rule.text
  } catch {
    /* 静默 */
  }
}
// —— 格式化硬盘预填盘符（双确认后跳过通用输入框）——
const formatDiskInput = ref<string | null>(null)
// —— P2：错误恢复动作（失败弹窗的"下一步"建议）——
const errorActions = ref<{ label: string; url?: string; kind: 'url' | 'hint' | 'admin' }[]>([])
function buildErrorActions(title: string, text: string): { label: string; url?: string; kind: 'url' | 'hint' | 'admin' }[] {
  const t = (title + ' ' + text).toLowerCase()
  const acts: { label: string; url?: string; kind: 'url' | 'hint' | 'admin' }[] = []
  // 常见命令缺失
  if (/\b(ffmpeg|python|javac?|node|git|winget|ollama|tesseract|python3)\b/.test(t) && /(not recognized|not found|找不到|无法识别|未找到|no such|command not)/.test(t)) {
    acts.push({ label: '查看安装教程', url: 'https://www.bing.com/search?q=Windows+安装+FFmpeg+教程', kind: 'url' })
    acts.push({ label: '回工具箱重新检查', kind: 'hint' })
  }
  // 权限不足
  if (/(access denied|denied|拒绝访问|权限不足|requires elevation|administrator)/.test(t)) {
    acts.push({ label: '以管理员身份运行重试', kind: 'admin' })
  }
  // 文件/路径不存在
  if (/(no such file|does not exist|找不到文件|路径不存在|无法找到)/.test(t)) {
    acts.push({ label: '检查输入的路径是否正确', kind: 'hint' })
  }
  // 网络失败
  if (/(timed? ?out|connection refused|网络|无法连接|failed to connect|离线)/.test(t)) {
    acts.push({ label: '检查网络或代理后重试', kind: 'hint' })
  }
  return acts
}
function doErrorAction(a: { label: string; url?: string; kind: 'url' | 'hint' | 'admin' }) {
  if (a.kind === 'url' && a.url) {
    openGuideUrl(a.url)
  } else if (a.kind === 'admin') {
    showOutput.value = false
    toast('请关闭本软件，右键「铃·记忆体.exe」→「以管理员身份运行」，再重试此工具。')
  } else {
    showOutput.value = false
  }
}
// —— 像素画板（批次3，前端组件）——
const showPixelArt = ref(false)
// —— 正则测试器（批次4，前端组件）——
const showRegex = ref(false)
// —— 依赖缺失引导（ffmpeg/Ollama/OCR语言包等缺失时弹窗提示下载方法）——
const showDepGuide = ref(false)
const depGuide = ref<{ id?: string; name: string; hint: string; install: string; url: string | null } | null>(null)
// —— 输出弹窗（执行有输出的工具时弹出完整内容，避免视觉盲区）——
const showOutput = ref(false)
const outputContent = ref('')
const outputTitle = ref('执行输出')

// —— 工具参数输入：内嵌表单（替代原生 prompt）——
const showInputPrompt = ref(false)
const inputLabel = ref('')
const inputPlaceholder = ref('')
const inputValue = ref('')
let inputResolve: ((value: string | null) => void) | null = null
function askInput(label: string, placeholder = '', initial = ''): Promise<string | null> {
  inputLabel.value = label
  inputPlaceholder.value = placeholder
  inputValue.value = initial
  showInputPrompt.value = true
  return new Promise<string | null>((resolve) => {
    inputResolve = resolve
  })
}
function finishInputPrompt(value: string | null) {
  showInputPrompt.value = false
  const done = inputResolve
  inputResolve = null
  done?.(value)
}
// —— moon10：二维码预览图片 URL（生成成功后展示）——
const qrImageUrl = ref<string | null>(null)

// —— 添加/编辑弹窗 ——
const showEditor = ref(false)
const editingItem = ref<ToolboxItem>({ id: '', name: '', icon: '', command: '', enabled: true })
const editorError = ref('')
// —— v0.6 组合工具（模块化，模仿快捷指令）——
const editorKind = ref<'cmd' | 'combo'>('cmd')
const SYS_MODULES: { tool: string; label: string; needInput?: boolean }[] = [
  { tool: 'volume', label: '设置音量', needInput: true },
  { tool: 'music', label: '播放音乐' },
  { tool: 'power-balanced', label: '电源：均衡' },
  { tool: 'power-high', label: '电源：高性能' },
  { tool: 'power-saver', label: '电源：节能' },
]
const pickedToolId = ref('')
/** 步骤 → 展示名（系统动作 / 工具箱工具） */
function stepName(tool: string): string {
  const sys = SYS_MODULES.find((m) => m.tool === tool)
  if (sys) return sys.label
  const it = desktop.toolboxItems.find((i) => i.id === tool)
  return it ? it.name : tool
}
/** 编辑器打开统一初始化 */
function initEditor(item: ToolboxItem, kind: 'cmd' | 'combo') {
  editingItem.value = { ...item, steps: item.steps ? [...item.steps] : [] }
  editorKind.value = kind
  editorError.value = ''
  pickedToolId.value = ''
  showEditor.value = true
}
/** 添加步骤：系统模块 */
function addSysStep(m: { tool: string; label: string; needInput?: boolean }) {
  const steps = editingItem.value.steps ?? (editingItem.value.steps = [])
  steps.push({ tool: m.tool, input: m.needInput ? '' : null })
}
/** 添加步骤：已有工具箱工具 */
function addToolStep() {
  const id = pickedToolId.value
  if (!id) return
  const steps = editingItem.value.steps ?? (editingItem.value.steps = [])
  if (steps.some((s) => s.tool === id)) return // 同工具不重复
  steps.push({ tool: id, input: null })
}
function moveStep(i: number, dir: -1 | 1) {
  const steps = editingItem.value.steps ?? []
  const j = i + dir
  if (j < 0 || j >= steps.length) return
  ;[steps[i], steps[j]] = [steps[j], steps[i]]
}
function removeStep(i: number) {
  editingItem.value.steps?.splice(i, 1)
}

// —— 删除确认 ——
const showDeleteConfirm = ref(false)
const pendingDeleteId = ref('')

async function loadForgedTools() {
  try { forgedTools.value = await listForgedTools() } catch (error) { feedback.value = { ok: false, text: String(error) } }
}
function forgedCreatedAt(timestamp: number) { return new Date(timestamp * 1000).toLocaleString('zh-CN') }
async function toggleForgedTool(tool: ForgedTool) {
  try { await setForgedToolEnabled(tool.id, !tool.enabled); tool.enabled = !tool.enabled } catch (error) { feedback.value = { ok: false, text: String(error) } }
}
async function copyForgedCode(tool: ForgedTool) {
  try { await navigator.clipboard.writeText(tool.code); feedback.value = { ok: true, text: '源码已复制' } } catch (error) { feedback.value = { ok: false, text: String(error) } }
}
async function removeForgedTool(tool: ForgedTool) {
  const ok = await confirmDialog('确定删除这个自制工具吗？删除后无法恢复。', { danger: true, confirmText: '删除' })
  if (!ok) return
  try { await deleteForgedTool(tool.id); forgedTools.value = forgedTools.value.filter((item) => item.id !== tool.id) } catch (error) { feedback.value = { ok: false, text: String(error) } }
}

onMounted(() => {
  desktop.loadToolboxItems()
  loadForgedTools()
})

// —— 依赖缺失检测：输出含依赖关键词 → 返回安装引导 ——
function detectDependency(text: string) {
  if (!text) return null
  const t = text.toLowerCase()
  if (/ffmpeg/.test(t)) return { name: 'ffmpeg（视频转码）', hint: '用于视频格式转换（转 MP4 等）', install: 'PowerShell 管理员运行：winget install ffmpeg', url: 'https://ffmpeg.org/download.html' }
  if (/ollama/.test(t)) return { name: 'Ollama（本地 AI）', hint: '用于本地离线对话', install: 'PowerShell 管理员运行：winget install Ollama.Ollama', url: 'https://ollama.com/download' }
  if (/ocr engine|language pack|not available/.test(t)) return { name: 'OCR 语言包', hint: 'Windows 内置 OCR 需要系统语言包', install: '设置 → 时间和语言 → 语言 → 添加语言 → 勾选「光学字符识别」', url: null }
  return null
}
function openGuideUrl(url: string) {
  openUrl(url).catch(() => window.open(url, '_blank'))
}
// —— 重新检查依赖（装好后点此重试，已装则关闭引导）——
async function recheckDep() {
  const id = depGuide.value?.id
  if (!id) return
  try {
    const st = await checkDependency(id)
    if (st.installed) {
      showDepGuide.value = false
      depGuide.value = null
      feedback.value = { ok: true, text: `${st.name} 已就绪，请重新点击工具执行` }
    } else {
      depGuide.value = { id: st.id, name: st.name, hint: `此功能需要安装 ${st.name}`, install: st.install, url: st.url }
    }
  } catch {
    /* 忽略 */
  }
}

async function runItem(item: ToolboxItem) {
  if (executingId.value) return
  // 格式化硬盘：毁灭性操作，双确认（内嵌表单输入盘符 + 确认不可恢复）
  if (item.id === 'format-disk') {
    const drv = await askInput('格式化硬盘（不可恢复）：输入要格式化的盘符，单个字母', 'D', 'D')
    if (drv === null) return
    const ok = await confirmDialog(
      `注意：再次确认，将格式化 ${drv.trim().replace(/:$/, '')}: 盘\n该操作会删除磁盘上所有数据，且无法恢复！\n\n确定继续吗？`,
      { danger: true, confirmText: '继续格式化' },
    )
    if (!ok) return
    // 直接执行（命令内部读 TOOLBOX_INPUT），不再弹通用输入框
    formatDiskInput.value = drv.trim().replace(/:$/, '')
    item = { ...item, needs_input: true }
  }
  // WiFi 密码工具：提前声明（仅解自己的 + 不上传）
  if (item.id === 'wifi-pwd') {
    const ok = await confirmDialog(
      '看 WiFi 密码前先确认：\n\n· 仅查看本机已保存的 WiFi 密码（你自己的网络）\n· 所有处理均在本机完成，不会上传任何数据\n\n是否继续？',
      { confirmText: '继续' },
    )
    if (!ok) return
  }
  // XOR 破解声明（仅限授权数据）
  if (item.id === 'xor') {
    const ok = await confirmDialog(
      'XOR 破解前先确认：\n\n· 仅用于破解你拥有或获授权测试的加密数据\n· 请勿用于非法破解他人数据\n\n是否继续？',
      { confirmText: '继续' },
    )
    if (!ok) return
  }
  // 像素画板：前端交互组件，不执行命令
  if (item.id === 'pixel-art') {
    showPixelArt.value = true
    return
  }
  // 正则测试器：前端交互组件
  if (item.id === 'regex') {
    showRegex.value = true
    return
  }
  // 二维码生成：调用 Rust 命令，弹窗展示图片预览 + 保存路径
  if (item.id === 'qrcode-gen') {
    const v = await askInput(item.input_label || '请输入要生成二维码的内容：', item.input_placeholder || '')
    if (v === null) return
    executingId.value = item.id
    feedback.value = null
    qrImageUrl.value = null
    try {
      const path = await generateQrcode(v)
      feedback.value = { ok: true, text: '二维码已生成' }
      outputTitle.value = '二维码生成成功'
      outputContent.value = `已保存到：\n${path}`
      qrImageUrl.value = assetUrl(path)
      showOutput.value = true
    } catch (e) {
      outputTitle.value = '二维码生成失败'
      outputContent.value = String(e)
      showOutput.value = true
    } finally {
      executingId.value = null
    }
    return
  }
  // 二维码识别：输入图片路径，返回解码内容
  if (item.id === 'qrcode-decode') {
    const v = await askInput(item.input_label || '请输入包含二维码的图片完整路径：', item.input_placeholder || '')
    if (v === null) return
    executingId.value = item.id
    feedback.value = null
    try {
      const content = await decodeQrcode(v)
      feedback.value = { ok: true, text: '二维码识别成功' }
      outputTitle.value = '二维码识别结果'
      outputContent.value = content
      showOutput.value = true
    } catch (e) {
      outputTitle.value = '二维码识别失败'
      outputContent.value = String(e)
      showOutput.value = true
    } finally {
      executingId.value = null
    }
    return
  }
  // OCR 文字识别（moon11）：优先 Windows OCR，失败自动降级 Tesseract
  if (item.id === 'ocr') {
    const v = await askInput(item.input_label || '请输入要识别的图片完整路径：', item.input_placeholder || '')
    if (v === null) return
    executingId.value = item.id
    feedback.value = null
    try {
      const r = await ocrImage(v)
      feedback.value = { ok: true, text: '识别成功' }
      outputTitle.value = r.engine === 'tesseract' ? 'OCR 识别结果（Tesseract）' : 'OCR 识别结果（Windows）'
      outputContent.value = r.text
      showOutput.value = true
    } catch (e) {
      outputTitle.value = 'OCR 识别失败'
      outputContent.value = String(e)
      showOutput.value = true
    } finally {
      executingId.value = null
    }
    return
  }
  // 需要输入参数的工具：先弹输入框（格式化硬盘已双确认预填，跳过）
  let input: string | undefined
  if (item.needs_input) {
    if (formatDiskInput.value !== null) {
      input = formatDiskInput.value
      formatDiskInput.value = null
    } else {
      const v = await askInput(item.input_label || `请输入「${item.name}」所需参数：`, item.input_placeholder || '')
      if (v === null) return // 用户取消
      input = v
    }
  }
  executingId.value = item.id
  // —— 过程感（P1 + D）：开始语 → 执行中"铃正在努力"动态提示 ——
  feedback.value = { ok: true, text: `铃：好的，帮你执行「${item.name}」～` }
  // 执行中提示（真实阶段数据暂缺时用动态努力语；有阶段输出的工具走 toolPhase）
  phaseText.value = null
  executingText.value = `铃正在努力${phaseDots.value}`
  phaseTimer = setInterval(() => {
    phaseDots.value = phaseDots.value.length >= 3 ? '' : phaseDots.value + '。'
    if (executingText.value) executingText.value = `铃正在努力${phaseDots.value}`
  }, 500)
  // —— 统一依赖检查（工具声明 dependencies，未装则弹引导后返回）——
  for (const depId of item.dependencies || []) {
    try {
      const st = await checkDependency(depId)
      if (st && !st.installed) {
        depGuide.value = { id: st.id, name: st.name, hint: `此功能需要安装 ${st.name}`, install: st.install, url: st.url }
        showDepGuide.value = true
        executingId.value = null
        return
      }
    } catch {
      /* 未知依赖跳过 */
    }
  }
  const result = await desktop.executeToolboxItem(item.id, input)
  executingId.value = null
  // 清理执行中动态提示
  if (phaseTimer) { clearInterval(phaseTimer); phaseTimer = undefined }
  executingText.value = null
  phaseText.value = null
  phaseDots.value = ''
  // 依赖缺失检测：输出含依赖关键词 → 弹安装引导（自动提示下载方法）
  const dep = detectDependency((result?.output || '') + ' ' + (result?.error || ''))
  if (dep) {
    depGuide.value = dep
    showDepGuide.value = true
  }
  if (result?.error) {
    feedback.value = { ok: false, text: `「${item.name}」没有完成` }
    // 失败也弹窗显示完整原因
    outputTitle.value = `${item.name} · 失败`
    outputContent.value = result.error
    // P2：错误恢复——按错误类型生成"下一步"动作
    errorActions.value = buildErrorActions(item.name, result.error)
    showOutput.value = true
  } else if (result?.output) {
    feedback.value = { ok: true, text: `「${item.name}」完成啦（点查看输出）` }
    // 有输出则弹独立窗口完整展示，避免顶部小区域视觉盲区
    outputTitle.value = `${item.name} · 输出`
    outputContent.value = result.output
    errorActions.value = []
    showOutput.value = true
  } else {
    feedback.value = { ok: true, text: `「${item.name}」完成啦` }
    errorActions.value = []
  }
  // P3：第一次使用工具箱里程碑（幂等）+ 每日日记工具计数
  if (!result?.error) {
    milestone.record('first_toolbox', '第一次使用铃的工具箱').catch(() => {})
    milestone.recordTool(item.name).catch(() => {})
    // 记忆×工具联动（中等概率概率事件）：35% 概率提起相关记忆
    checkToolMemoryEcho(item.id)
  }
  // 反馈 6 秒后消失（开始语+完成语周期，不影响弹窗）
  setTimeout(() => (feedback.value = null), 6000)
}

// —— 添加 ——
function openAdd() {
  initEditor({ id: `user_${Date.now()}`, name: '', icon: '', command: '', enabled: true }, 'cmd')
}

/** 添加组合工具（模块化多步） */
function openAddCombo() {
  initEditor({ id: `user_${Date.now()}`, name: '', icon: '', command: '', enabled: true, steps: [] }, 'combo')
}

/** 双击自定义工具 → 编辑（内置 preset 不可编辑） */
function editItem(item: ToolboxItem) {
  if (!item.id.startsWith('user_')) {
    feedback.value = { ok: false, text: '内置工具不可编辑，可点「命令工具」创建自己的工具' }
    return
  }
  initEditor(item, (item.steps?.length ?? 0) > 0 ? 'combo' : 'cmd')
}

async function saveItem() {
  const it = editingItem.value
  if (!it.name.trim()) {
    editorError.value = '请输入工具名称'
    return
  }
  if (editorKind.value === 'combo') {
    // 组合工具：至少一步；command 仅作展示占位（执行由后端按 steps 分发）
    const steps = (it.steps ?? []).filter((s) => s.tool)
    if (steps.length === 0) {
      editorError.value = '请至少添加一步动作'
      return
    }
    it.steps = steps
    it.command = `【组合工具 · ${steps.length} 步】`
  } else {
    if (!it.command.trim()) {
      editorError.value = '请输入要执行的命令'
      return
    }
    it.steps = []
  }
  try {
    await desktop.addOrUpdateToolboxItem(it)
    showEditor.value = false
  } catch (e) {
    editorError.value = String(e)
  }
}

// —— 删除（仅用户自定义条目，预设不可删）——
function requestDelete(item: ToolboxItem) {
  if (!item.id.startsWith('user_')) {
    feedback.value = { ok: false, text: '预设工具不可删除' }
    setTimeout(() => (feedback.value = null), 3000)
    return
  }
  pendingDeleteId.value = item.id
  showDeleteConfirm.value = true
}

async function confirmDelete() {
  await desktop.removeToolboxItem(pendingDeleteId.value)
  showDeleteConfirm.value = false
}
</script>

<template>
  <div class="toolbox-panel" role="region" aria-label="工具箱">
    <!-- 像素画板 / 正则测试器（独立遮罩组件，自身就是 fixed 层） -->
    <PixelArtPanel v-if="showPixelArt" @close="showPixelArt = false" />
    <RegexTester v-if="showRegex" @close="showRegex = false" />

    <!-- 轻量操作条：标题与关闭已在抽屉标签条上，这里只留新增按钮 -->
    <div class="panel-bar">
      <AppButton size="sm" @click="openAdd">
        <TheIcon name="add" :size="14" />命令工具
      </AppButton>
      <AppButton size="sm" @click="openAddCombo">
        <TheIcon name="customize" :size="14" />组合工具
      </AppButton>
      <span class="bar-meta">{{ desktop.toolboxItems.length }} 个工具</span>
    </div>

    <div class="panel-body">
      <!-- 执行反馈 -->
      <div v-if="feedback" class="feedback" :class="feedback.ok ? 'ok' : 'err'" role="status">
        <AppStatusDot :state="feedback.ok ? 'ok' : 'err'" />
        <span class="feedback-text">{{ feedback.text }}</span>
      </div>
      <!-- D 批次：执行中动态提示（铃正在努力…） -->
      <div v-if="executingText" class="executing-hint" role="status">
        <span class="spin-ring" aria-hidden="true" />
        <span class="exec-text">{{ executingText }}</span>
        <span v-if="phaseText" class="exec-phase">{{ phaseText }}</span>
      </div>
      <!-- 记忆×工具联动（概率事件）：铃提起相关记忆 -->
      <div v-if="memoryEcho" class="memory-echo">
        <AppStatusDot state="ok" />
        <span class="echo-text">{{ memoryEcho }}</span>
        <button type="button" class="icon-btn" title="收起" @click="memoryEcho = null">
          <TheIcon name="close" :size="12" />
        </button>
      </div>

      <!-- 工具网格：列数随抽屉宽度自适应，380px 下不横向滚动 -->
      <div class="grid">
        <button
          v-for="item in desktop.toolboxItems"
          :key="item.id"
          type="button"
          class="cell"
          :class="{ running: executingId === item.id, disabled: !item.enabled, combo: (item.steps?.length ?? 0) > 0 }"
          :title="item.id.startsWith('user_') ? '左键执行 · 双击编辑 · 右键删除' : '左键执行'"
          @click="runItem(item)"
          @dblclick="editItem(item)"
          @contextmenu.prevent="requestDelete(item)"
        >
          <span class="cell-icon">
            <span v-if="executingId === item.id" class="spin-ring" aria-hidden="true" />
            <TheIcon v-else :name="toolIcon(item.id)" :size="18" />
          </span>
          <span class="cell-name">{{ item.name }}</span>
          <span v-if="(item.steps?.length ?? 0) > 0" class="cell-badge">×{{ item.steps!.length }}</span>
        </button>
        <div v-if="desktop.toolboxLoading" class="cell loading-cell">加载中…</div>
      </div>

      <!-- 自制工具 -->
      <section class="forged-section">
        <div class="forged-title">自制工具</div>
        <p v-if="!setting.agentAllowToolForge" class="forged-hint">需在设置里授权 Agent 自己编写小工具</p>
        <div v-if="forgedTools.length === 0" class="forged-empty">暂无自制工具</div>
        <article v-for="tool in forgedTools" :key="tool.id" class="forged-card">
          <div class="forged-card-head">
            <strong class="forged-name">{{ tool.name }}</strong>
            <label class="forged-toggle">
              <input type="checkbox" :checked="tool.enabled" @change="toggleForgedTool(tool)" />启用
            </label>
          </div>
          <div class="forged-description">{{ tool.description }}</div>
          <div class="forged-meta">{{ tool.language }} · {{ forgedCreatedAt(tool.created_at) }} · 调用 {{ tool.call_count }} 次</div>
          <div class="forged-actions">
            <AppButton size="sm" @click="expandedForgedId = expandedForgedId === tool.id ? null : tool.id">
              {{ expandedForgedId === tool.id ? '收起源码' : '查看源码' }}
            </AppButton>
            <AppButton v-if="expandedForgedId === tool.id" size="sm" @click="copyForgedCode(tool)">复制源码</AppButton>
            <AppButton size="sm" variant="text" @click="removeForgedTool(tool)">删除</AppButton>
          </div>
          <pre v-if="expandedForgedId === tool.id" class="forged-source">{{ tool.code }}</pre>
          <div v-if="tool.last_error" class="forged-error">最近失败：{{ tool.last_error }}</div>
        </article>
      </section>

      <p class="panel-footer">清理内存＝释放所有进程工作集＋清系统缓存（管理员模式更强）· 右键工具可删除（仅自定义）</p>
    </div>

    <!-- 弹窗：固定遮罩层，不进抽屉内容区，避免被抽屉宽度裁切 -->
    <!-- 工具参数输入（内嵌表单，替代原生 prompt） -->
    <div v-if="showInputPrompt" class="modal-mask" @click.self="finishInputPrompt(null)">
      <div class="modal" role="dialog" aria-modal="true">
        <div class="modal-title">{{ inputLabel }}</div>
        <AppInput
          :model-value="inputValue"
          :placeholder="inputPlaceholder"
          @update:model-value="inputValue = $event"
        />
        <div class="modal-actions">
          <AppButton @click="finishInputPrompt(null)">取消</AppButton>
          <AppButton variant="primary" @click="finishInputPrompt(inputValue)">确定</AppButton>
        </div>
      </div>
    </div>

    <!-- 输出弹窗：完整显示命令输出（端口/清理等），可滚动，无视觉盲区 -->
    <div v-if="showOutput" class="modal-mask" @click.self="showOutput = false">
      <div class="modal output-modal" role="dialog" aria-modal="true" :aria-label="outputTitle">
        <div class="modal-title">{{ outputTitle }}</div>
        <img v-if="qrImageUrl && outputTitle.includes('二维码生成')" :src="qrImageUrl" class="qr-preview" alt="二维码预览" />
        <pre class="output-pre">{{ outputContent }}</pre>
        <!-- P2：错误恢复动作（仅失败时显示） -->
        <div v-if="errorActions.length > 0" class="error-actions">
          <span class="error-actions-title">可以试试：</span>
          <button v-for="(a, i) in errorActions" :key="i" type="button" class="action-btn" @click="doErrorAction(a)">
            {{ a.label }}
          </button>
        </div>
        <div class="modal-actions">
          <AppButton @click="showOutput = false">关闭</AppButton>
        </div>
      </div>
    </div>

    <!-- 依赖缺失引导弹窗 -->
    <div v-if="showDepGuide" class="modal-mask" @click.self="showDepGuide = false">
      <div class="modal dep-box" role="dialog" aria-modal="true">
        <div class="modal-title">缺少依赖：{{ depGuide?.name }}</div>
        <p class="dep-hint">{{ depGuide?.hint }}</p>
        <p class="dep-install">{{ depGuide?.install }}</p>
        <div class="modal-actions">
          <AppButton v-if="depGuide?.url" variant="primary" @click="openGuideUrl(depGuide.url)">打开下载页</AppButton>
          <AppButton v-if="depGuide?.id" @click="recheckDep">重新检查</AppButton>
          <AppButton variant="text" @click="showDepGuide = false">知道了</AppButton>
        </div>
      </div>
    </div>

    <!-- 添加/编辑弹窗 -->
    <div v-if="showEditor" class="modal-mask" @click.self="showEditor = false">
      <div class="modal editor-modal" role="dialog" aria-modal="true">
        <div class="modal-title">{{ editingItem.id.startsWith('user_') && !editingItem.name ? '添加工具' : '编辑工具' }}</div>
        <!-- 类型切换 -->
        <AppTabs
          :items="[{ id: 'cmd', label: '命令工具' }, { id: 'combo', label: '组合工具' }]"
          :model-value="editorKind"
          @update:modelValue="editorKind = $event === 'combo' ? 'combo' : 'cmd'"
        />
        <label class="field">
          <span>名称</span>
          <input v-model="editingItem.name" placeholder="如：晚安模式 / 打开我的世界" />
        </label>

        <!-- 命令模式 -->
        <template v-if="editorKind === 'cmd'">
          <label class="field">
            <span>命令</span>
            <textarea v-model="editingItem.command" rows="3" placeholder="如：D:\game\mc.exe 或 notepad" />
          </label>
        </template>

        <!-- 组合模式：模块库 + 步骤编排 -->
        <template v-else>
          <div class="field">
            <span>动作模块（点击追加为一步）</span>
            <div class="module-chips">
              <AppButton v-for="m in SYS_MODULES" :key="m.tool" size="sm" @click="addSysStep(m)">{{ m.label }}</AppButton>
            </div>
            <div class="tool-pick">
              <select v-model="pickedToolId" class="pick-select">
                <option value="">从工具箱选一个已有工具</option>
                <option v-for="it in desktop.toolboxItems.filter((i) => i.id !== editingItem.id)" :key="it.id" :value="it.id">{{ it.name }}</option>
              </select>
              <AppButton size="sm" :disabled="!pickedToolId" @click="addToolStep">追加</AppButton>
            </div>
          </div>
          <div class="field">
            <span>执行顺序（{{ (editingItem.steps ?? []).length }} 步）</span>
            <div v-if="(editingItem.steps ?? []).length === 0" class="step-empty">还没有步骤，从上方模块添加</div>
            <div v-for="(s, i) in editingItem.steps ?? []" :key="i" class="step-row">
              <span class="step-idx">{{ i + 1 }}</span>
              <span class="step-name">{{ stepName(s.tool) }}</span>
              <input v-if="s.tool === 'volume'" v-model="s.input" class="step-input" placeholder="音量 0-100" />
              <div class="step-ops">
                <button type="button" class="op" :disabled="i === 0" @click="moveStep(i, -1)">上</button>
                <button type="button" class="op" :disabled="i === (editingItem.steps ?? []).length - 1" @click="moveStep(i, 1)">下</button>
                <button type="button" class="op op-del" @click="removeStep(i)">删</button>
              </div>
            </div>
          </div>
          <div class="combo-hint">组合工具执行时按顺序运行每一步，可混搭系统动作与工具箱工具</div>
        </template>

        <div v-if="editorError" class="editor-error">{{ editorError }}</div>
        <div class="modal-actions">
          <AppButton variant="text" @click="showEditor = false">取消</AppButton>
          <AppButton variant="primary" @click="saveItem">保存</AppButton>
        </div>
      </div>
    </div>

    <!-- 删除确认 -->
    <div v-if="showDeleteConfirm" class="modal-mask" @click.self="showDeleteConfirm = false">
      <div class="modal modal-small" role="dialog" aria-modal="true">
        <div class="modal-title">删除这个工具？</div>
        <div class="modal-actions">
          <AppButton variant="text" @click="showDeleteConfirm = false">取消</AppButton>
          <button type="button" class="btn-danger" @click="confirmDelete">删除</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 流式布局：占满抽屉内容区，滚动交给内容区自己 */
.toolbox-panel {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
  color: var(--ui-text);
  font-size: var(--ui-fs-13);
}

/* 轻量操作条 */
.panel-bar {
  flex: none;
  flex-wrap: wrap;
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-bottom: 1px solid var(--ui-border);
}
.bar-meta {
  margin-left: auto;
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
}

/* 内容区：唯一滚动容器 */
.panel-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-3);
}

/* —— 反馈 / 执行中 / 记忆联动 —— */
.feedback {
  display: flex;
  align-items: flex-start;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-radius: var(--ui-r-sm);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  min-height: 28px;
}
.feedback-text {
  min-width: 0;
  overflow-wrap: anywhere;
}
.feedback.ok {
  background: color-mix(in srgb, var(--ui-ok) 10%, transparent);
  color: var(--ui-text);
  border-left: 3px solid var(--ui-ok);
}
.feedback.err {
  background: color-mix(in srgb, var(--ui-err) 10%, transparent);
  color: var(--ui-text);
  border-left: 3px solid var(--ui-err);
}
.executing-hint {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-radius: var(--ui-r-sm);
  background: var(--ui-accent-soft);
  border: 1px solid var(--ui-accent-line);
  font-size: var(--ui-fs-12);
  min-height: 28px;
}
.exec-text {
  color: var(--ui-accent);
  font-weight: 500;
}
.exec-phase {
  color: var(--ui-text-3);
  font-size: var(--ui-fs-11);
}
.memory-echo {
  display: flex;
  align-items: flex-start;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-radius: var(--ui-r-sm);
  background: color-mix(in srgb, var(--ui-ok) 8%, transparent);
  border: 1px solid color-mix(in srgb, var(--ui-ok) 28%, transparent);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
}
.echo-text {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
}

/* 转圈：替代原来的文字符号 */
.spin-ring {
  flex: none;
  width: 12px;
  height: 12px;
  border-radius: var(--ui-r-pill);
  border: 2px solid var(--ui-accent-soft);
  border-top-color: var(--ui-accent);
  animation: tb-spin 700ms linear infinite;
}
@keyframes tb-spin {
  to { transform: rotate(360deg); }
}

/* 小图标按钮 */
.icon-btn {
  flex: none;
  display: inline-grid;
  place-items: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: 0;
  border-radius: var(--ui-r-sm);
  background: transparent;
  color: var(--ui-text-2);
}
.icon-btn:hover {
  background: var(--ui-hover);
  color: var(--ui-text);
}

/* —— 工具网格：自适应列数 —— */
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
  gap: var(--ui-sp-2);
}
.cell {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--ui-sp-1);
  min-height: 66px;
  padding: var(--ui-sp-2) var(--ui-sp-1);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-md);
  background: var(--ui-raised);
  color: var(--ui-text);
  font-size: var(--ui-fs-12);
  transition: background var(--ui-dur-1) var(--ui-ease), border-color var(--ui-dur-1) var(--ui-ease);
}
.cell:hover {
  background: var(--ui-hover);
  border-color: var(--ui-border-strong);
}
.cell.combo {
  border-color: var(--ui-accent-line);
}
.cell.running,
.cell.disabled {
  opacity: 0.55;
}
.cell-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 18px;
  color: var(--ui-text-2);
}
.cell-name {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  line-height: 1.4;
  text-align: center;
  overflow-wrap: anywhere;
}
.cell-badge {
  position: absolute;
  top: 3px;
  right: 3px;
  padding: 0 var(--ui-sp-1);
  border-radius: var(--ui-r-pill);
  background: var(--ui-accent-soft);
  color: var(--ui-accent);
  font-size: var(--ui-fs-11);
  line-height: 14px;
}
.loading-cell {
  justify-content: center;
  color: var(--ui-text-3);
  cursor: default;
}

/* —— 自制工具 —— */
.forged-section {
  margin-top: var(--ui-sp-2);
  padding-top: var(--ui-sp-2);
  border-top: 1px solid var(--ui-border);
}
.forged-title {
  font-size: var(--ui-fs-13);
  font-weight: 600;
  margin-bottom: var(--ui-sp-1);
}
.forged-hint,
.forged-empty,
.forged-meta {
  color: var(--ui-text-3);
  font-size: var(--ui-fs-11);
  line-height: var(--ui-line);
}
.forged-card {
  padding: var(--ui-sp-2) 0;
  border-top: 1px solid var(--ui-border);
}
.forged-card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--ui-sp-2);
}
.forged-name {
  min-width: 0;
  overflow-wrap: anywhere;
}
.forged-description {
  margin-top: var(--ui-sp-1);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  overflow-wrap: anywhere;
}
.forged-meta {
  margin-top: var(--ui-sp-1);
}
.forged-toggle {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-1);
  flex: none;
  font-size: var(--ui-fs-11);
  color: var(--ui-text-2);
}
.forged-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  margin-top: var(--ui-sp-2);
}
.forged-source {
  max-height: 180px;
  overflow: auto;
  margin: var(--ui-sp-2) 0 0;
  padding: var(--ui-sp-2);
  white-space: pre-wrap;
  user-select: text;
  background: var(--ui-bg);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  font-family: Consolas, 'Courier New', monospace;
  font-size: var(--ui-fs-11);
  line-height: var(--ui-line);
  color: var(--ui-text);
}
.forged-error {
  margin-top: var(--ui-sp-1);
  color: var(--ui-err);
  font-size: var(--ui-fs-11);
  overflow-wrap: anywhere;
}
.panel-footer {
  margin: var(--ui-sp-2) 0 0;
  text-align: center;
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
}

/* —— 弹窗（传送到 body）—— */
.modal-mask {
  position: fixed;
  inset: 0;
  z-index: 600;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--ui-sp-4);
  background: color-mix(in srgb, var(--ui-bg) 78%, transparent);
}
.modal {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-2);
  width: min(360px, 92vw);
  max-height: 88vh;
  overflow-y: auto;
  padding: var(--ui-sp-4);
  background: var(--ui-raised);
  color: var(--ui-text);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-lg);
  box-shadow: var(--ui-shadow);
}
.modal.output-modal {
  width: min(560px, 92vw);
}
.modal.editor-modal {
  width: min(400px, 92vw);
}
.modal.modal-small {
  width: min(260px, 88vw);
}
.modal-title {
  font-size: var(--ui-fs-14);
  font-weight: 600;
  overflow-wrap: anywhere;
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--ui-sp-2);
  margin-top: var(--ui-sp-2);
}
.qr-preview {
  display: block;
  margin: 0 auto;
  width: 200px;
  height: 200px;
  border-radius: var(--ui-r-sm);
  border: 1px solid var(--ui-border);
  background: var(--ui-bg);
}
.output-pre {
  margin: 0;
  padding: var(--ui-sp-3);
  max-height: 60vh;
  overflow: auto;
  background: var(--ui-bg);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  font-family: Consolas, 'Courier New', monospace;
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text);
  white-space: pre-wrap;
  word-break: break-all;
}

/* P2：错误恢复动作栏 */
.error-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-radius: var(--ui-r-sm);
  background: color-mix(in srgb, var(--ui-warn) 10%, transparent);
  border: 1px solid color-mix(in srgb, var(--ui-warn) 28%, transparent);
}
.error-actions-title {
  font-size: var(--ui-fs-12);
  color: var(--ui-warn);
  font-weight: 600;
}
.action-btn {
  padding: 0 var(--ui-sp-3);
  height: 26px;
  border-radius: var(--ui-r-sm);
  border: 1px solid var(--ui-border-strong);
  background: transparent;
  color: var(--ui-text);
  font-size: var(--ui-fs-11);
  transition: border-color var(--ui-dur-1) var(--ui-ease), color var(--ui-dur-1) var(--ui-ease);
}
.action-btn:hover {
  border-color: var(--ui-accent-line);
  color: var(--ui-accent);
}

/* 依赖引导 */
.dep-hint {
  margin: 0;
  font-size: var(--ui-fs-13);
  color: var(--ui-text-2);
  line-height: var(--ui-line);
}
.dep-install {
  margin: 0;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-radius: var(--ui-r-sm);
  background: var(--ui-active);
  color: var(--ui-text);
  font-family: Consolas, 'Courier New', monospace;
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  white-space: pre-wrap;
  word-break: break-all;
}

/* 编辑器表单 */
.field {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
  min-width: 0;
}
.field > span {
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
}
.field input,
.field textarea {
  width: 100%;
  min-width: 0;
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-2);
  background: var(--ui-bg);
  color: var(--ui-text);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-sm);
  font-size: var(--ui-fs-13);
}
.field textarea {
  height: auto;
  padding: var(--ui-sp-2);
  line-height: var(--ui-line);
  resize: vertical;
}
.field input:focus,
.field textarea:focus,
.pick-select:focus,
.step-input:focus {
  border-color: var(--ui-accent);
}
.module-chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--ui-sp-1);
}
.tool-pick {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  margin-top: var(--ui-sp-1);
}
.pick-select {
  flex: 1;
  min-width: 0;
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-1);
  border-radius: var(--ui-r-sm);
  border: 1px solid var(--ui-border-strong);
  background: var(--ui-bg);
  color: var(--ui-text);
  font-size: var(--ui-fs-12);
}
.step-empty {
  padding: var(--ui-sp-3);
  border: 1px dashed var(--ui-border);
  border-radius: var(--ui-r-sm);
  font-size: var(--ui-fs-12);
  color: var(--ui-text-3);
  text-align: center;
}
.step-row {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-1);
  padding: var(--ui-sp-1) var(--ui-sp-2);
  margin-top: var(--ui-sp-1);
  border-radius: var(--ui-r-sm);
  background: var(--ui-hover);
  font-size: var(--ui-fs-12);
}
.step-idx {
  flex: none;
  display: inline-grid;
  place-items: center;
  width: 18px;
  height: 18px;
  border-radius: var(--ui-r-pill);
  background: var(--ui-accent-soft);
  color: var(--ui-accent);
  font-size: var(--ui-fs-11);
}
.step-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.step-input {
  width: 76px;
  flex: none;
  height: 24px;
  padding: 0 var(--ui-sp-1);
  border-radius: var(--ui-r-sm);
  border: 1px solid var(--ui-border-strong);
  background: var(--ui-bg);
  color: var(--ui-text);
  font-size: var(--ui-fs-11);
}
.step-ops {
  display: flex;
  flex: none;
  gap: 2px;
}
.op {
  width: 22px;
  height: 22px;
  padding: 0;
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  background: transparent;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-11);
  line-height: 1;
}
.op:hover:not(:disabled) {
  background: var(--ui-active);
  color: var(--ui-text);
}
.op:disabled {
  opacity: 0.4;
}
.op-del:hover:not(:disabled) {
  color: var(--ui-err);
}
.combo-hint {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
}
.editor-error {
  color: var(--ui-err);
  font-size: var(--ui-fs-12);
  margin-top: var(--ui-sp-1);
}
.btn-danger {
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-3);
  border-radius: var(--ui-r-sm);
  border: 1px solid var(--ui-err);
  background: color-mix(in srgb, var(--ui-err) 14%, transparent);
  color: var(--ui-err);
  font-size: var(--ui-fs-13);
}
.btn-danger:hover {
  background: color-mix(in srgb, var(--ui-err) 22%, transparent);
}
</style>
