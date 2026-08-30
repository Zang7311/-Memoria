<!-- 《铃·记忆体》工具箱面板（AI-6 任务 7 / 4.2）
     九宫格布局：图标 + 名称；点击执行系统命令；底部添加工具；长按/右键删除 -->
<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useDesktopStore } from '../stores/desktopStore'
import { useSettingStore } from '../stores/settingStore'
import { useMilestoneStore } from '../stores/milestoneStore'
import { assetUrl, checkDependency, decodeQrcode, generateQrcode, ocrImage, openUrl } from '../utils/tauri'
import type { ToolboxItem } from '../types'
import PixelArtPanel from './PixelArtPanel.vue'
import RegexTester from './RegexTester.vue'

const desktop = useDesktopStore()
const setting = useSettingStore()
const milestone = useMilestoneStore()

// 关闭工具箱（父组件监听后隐藏）
const emit = defineEmits<{ (e: 'close'): void }>()

const executingId = ref<string | null>(null)
const feedback = ref<{ ok: boolean; text: string } | null>(null)
// —— D 批次：执行中"铃正在努力"动态提示 ——
const executingText = ref<string | null>(null)
const phaseText = ref<string | null>(null) // 真实阶段文本（有阶段输出的工具用）
const phaseDots = ref('')
let phaseTimer: ReturnType<typeof setInterval> | undefined
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
    alert('请关闭本软件，右键「铃·记忆体.exe」→「以管理员身份运行」，再重试此工具。')
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
// —— moon10：二维码预览图片 URL（生成成功后展示）——
const qrImageUrl = ref<string | null>(null)

// —— 添加弹窗 ——
const showEditor = ref(false)
const editingItem = ref<ToolboxItem>({ id: '', name: '', icon: '🔧', command: '', enabled: true })
const editorError = ref('')

// —— 删除确认 ——
const showDeleteConfirm = ref(false)
const pendingDeleteId = ref('')

onMounted(() => {
  desktop.loadToolboxItems()
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
      feedback.value = { ok: true, text: `✓ ${st.name} 已就绪，请重新点击工具执行` }
    } else {
      depGuide.value = { id: st.id, name: st.name, hint: `此功能需要安装 ${st.name}`, install: st.install, url: st.url }
    }
  } catch {
    /* 忽略 */
  }
}

async function runItem(item: ToolboxItem) {
  if (executingId.value) return
  // 格式化硬盘：毁灭性操作，双确认（输入盘符 + 确认不可恢复）
  if (item.id === 'format-disk') {
    const drv = prompt('⚠️ 格式化硬盘（不可恢复！）\n\n输入要格式化的盘符（单个字母，如 D）：', 'D')
    if (drv === null) return
    const ok = confirm(`⚠️ 再次确认：将格式化 ${drv.trim().replace(/:$/, '')}: 盘\n该操作会删除磁盘上所有数据，且无法恢复！\n\n确定继续吗？`)
    if (!ok) return
    // 直接执行（命令内部读 TOOLBOX_INPUT），不再弹通用输入框
    formatDiskInput.value = drv.trim().replace(/:$/, '')
    item = { ...item, needs_input: true }
  }
  // WiFi 密码工具：提前声明（仅解自己的 + 不上传）
  if (item.id === 'wifi-pwd') {
    const ok = confirm(
      '⚠️ WiFi 密码查看声明：\n\n· 仅查看本机已保存的 WiFi 密码（你自己的网络）\n· 所有处理均在本机完成，不会上传任何数据\n\n是否继续？'
    )
    if (!ok) return
  }
  // XOR 破解声明（仅限授权数据）
  if (item.id === 'xor') {
    const ok = confirm(
      '⚠️ XOR 破解声明：\n\n· 仅用于破解你拥有或获授权测试的加密数据\n· 请勿用于非法破解他人数据\n\n是否继续？'
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
    const v = window.prompt(item.input_label || '请输入要生成二维码的内容：', item.input_placeholder || '')
    if (v === null) return
    executingId.value = item.id
    feedback.value = null
    qrImageUrl.value = null
    try {
      const path = await generateQrcode(v)
      feedback.value = { ok: true, text: '✓ 二维码已生成' }
      outputTitle.value = '📱 二维码生成成功'
      outputContent.value = `已保存到：\n${path}`
      qrImageUrl.value = assetUrl(path)
      showOutput.value = true
    } catch (e) {
      outputTitle.value = '✗ 二维码生成失败'
      outputContent.value = String(e)
      showOutput.value = true
    } finally {
      executingId.value = null
    }
    return
  }
  // 二维码识别：输入图片路径，返回解码内容
  if (item.id === 'qrcode-decode') {
    const v = window.prompt(item.input_label || '请输入包含二维码的图片完整路径：', item.input_placeholder || '')
    if (v === null) return
    executingId.value = item.id
    feedback.value = null
    try {
      const content = await decodeQrcode(v)
      feedback.value = { ok: true, text: '✓ 二维码识别成功' }
      outputTitle.value = '✅ 二维码识别结果'
      outputContent.value = content
      showOutput.value = true
    } catch (e) {
      outputTitle.value = '✗ 二维码识别失败'
      outputContent.value = String(e)
      showOutput.value = true
    } finally {
      executingId.value = null
    }
    return
  }
  // OCR 文字识别（moon11）：优先 Windows OCR，失败自动降级 Tesseract
  if (item.id === 'ocr') {
    const v = window.prompt(item.input_label || '请输入要识别的图片完整路径：', item.input_placeholder || '')
    if (v === null) return
    executingId.value = item.id
    feedback.value = null
    try {
      const r = await ocrImage(v)
      feedback.value = { ok: true, text: '✓ 识别成功' }
      outputTitle.value = r.engine === 'tesseract' ? '🔤 Tesseract OCR 识别结果' : '👁️ Windows OCR 识别结果'
      outputContent.value = r.text
      showOutput.value = true
    } catch (e) {
      outputTitle.value = '✗ OCR 识别失败'
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
      const v = window.prompt(item.input_label || `请输入「${item.name}」所需参数：`, item.input_placeholder || '')
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
    feedback.value = { ok: false, text: `✗ ${item.name} 没有完成` }
    // 失败也弹窗显示完整原因
    outputTitle.value = `✗ ${item.name} · 失败`
    outputContent.value = result.error
    // P2：错误恢复——按错误类型生成"下一步"动作
    errorActions.value = buildErrorActions(item.name, result.error)
    showOutput.value = true
  } else if (result?.output) {
    feedback.value = { ok: true, text: `✓ ${item.name} 完成啦（点查看输出）` }
    // 有输出则弹独立窗口完整展示，避免顶部小区域视觉盲区
    outputTitle.value = `📄 ${item.name} · 输出`
    outputContent.value = result.output
    errorActions.value = []
    showOutput.value = true
  } else {
    feedback.value = { ok: true, text: `✓ ${item.name} 完成啦` }
    errorActions.value = []
  }
  // P3：第一次使用工具箱里程碑（幂等）+ 每日日记工具计数
  if (!result?.error) {
    milestone.record('first_toolbox', '第一次使用铃的工具箱').catch(() => {})
    milestone.recordTool(item.name).catch(() => {})
  }
  // 反馈 6 秒后消失（开始语+完成语周期，不影响弹窗）
  setTimeout(() => (feedback.value = null), 6000)
}

// —— 添加 ——
function openAdd() {
  editingItem.value = { id: `user_${Date.now()}`, name: '', icon: '🔧', command: '', enabled: true }
  editorError.value = ''
  showEditor.value = true
}

async function saveItem() {
  const it = editingItem.value
  if (!it.name.trim()) {
    editorError.value = '请输入工具名称'
    return
  }
  if (!it.command.trim()) {
    editorError.value = '请输入要执行的命令'
    return
  }
  try {
    await desktop.addOrUpdateToolboxItem({ ...it, icon: it.icon.trim() || '🔧' })
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
  <div class="toolbox-panel">
    <!-- 像素画板（批次3，全屏遮罩） -->
    <PixelArtPanel v-if="showPixelArt" @close="showPixelArt = false" />
    <!-- 正则测试器（批次4，全屏遮罩） -->
    <RegexTester v-if="showRegex" @close="showRegex = false" />
    <div class="panel-header">
      <span class="panel-title">铃的工具箱<span class="title-sub">（{{ desktop.toolboxItems.length }} 个工具）</span></span>
      <div class="header-btns">
        <button class="add-btn" title="添加工具" @click="openAdd">＋</button>
        <button class="close-btn" title="关闭工具箱" @click="emit('close')">✕</button>
      </div>
    </div>

    <!-- 执行反馈 -->
    <div v-if="feedback" class="feedback" :class="feedback.ok ? 'ok' : 'err'">
      {{ feedback.text }}
    </div>
    <!-- D 批次：执行中动态提示（铃正在努力…） -->
    <div v-if="executingText" class="executing-hint">
      <span class="exec-spinner">⏳</span>
      <span class="exec-text">{{ executingText }}</span>
      <span v-if="phaseText" class="exec-phase">{{ phaseText }}</span>
    </div>

    <!-- 九宫格 -->
    <div class="grid">
      <div
        v-for="item in desktop.toolboxItems"
        :key="item.id"
        class="cell"
        :class="{ running: executingId === item.id, disabled: !item.enabled }"
        @click="runItem(item)"
        @contextmenu.prevent="requestDelete(item)"
      >
        <span v-if="setting.emojiMode !== 'off'" class="cell-icon">{{ item.icon }}</span>
        <span class="cell-name">{{ item.name }}</span>
        <span v-if="executingId === item.id" class="spinner">⏳</span>
      </div>
      <div v-if="desktop.toolboxLoading" class="cell loading-cell">加载中…</div>
    </div>

    <div class="panel-footer">清理内存＝释放所有进程工作集＋清系统缓存（管理员模式更强）· 右键工具可删除（仅自定义）</div>

    <!-- 输出弹窗：完整显示命令输出（端口/清理等），可滚动，无视觉盲区 -->
    <div v-if="showOutput" class="modal-mask" @click.self="showOutput = false">
      <div class="modal output-modal">
        <div class="modal-title">{{ outputTitle }}</div>
        <img v-if="qrImageUrl && outputTitle.includes('二维码生成')" :src="qrImageUrl" class="qr-preview" alt="二维码预览" />
        <pre class="output-pre">{{ outputContent }}</pre>
        <!-- P2：错误恢复动作（仅失败时显示） -->
        <div v-if="errorActions.length > 0" class="error-actions">
          <span class="error-actions-title">💡 可以试试：</span>
          <button v-for="(a, i) in errorActions" :key="i" class="btn action-btn" @click="doErrorAction(a)">
            {{ a.label }}
          </button>
        </div>
        <div class="modal-actions">
          <button class="btn confirm" @click="showOutput = false">关闭</button>
        </div>
      </div>
    </div>

    <!-- 添加/编辑弹窗 -->
    <!-- 依赖缺失引导弹窗 -->
    <div v-if="showDepGuide" class="modal-mask" @click.self="showDepGuide = false">
      <div class="modal-box dep-box">
        <h4 class="dep-title">缺少依赖：{{ depGuide?.name }}</h4>
        <p class="dep-hint">{{ depGuide?.hint }}</p>
        <p class="dep-install">{{ depGuide?.install }}</p>
        <div class="modal-actions">
          <button v-if="depGuide?.url" class="btn confirm" @click="openGuideUrl(depGuide.url)">打开下载页</button>
          <button v-if="depGuide?.id" class="btn" @click="recheckDep">重新检查</button>
          <button class="btn cancel" @click="showDepGuide = false">知道了</button>
        </div>
      </div>
    </div>

    <div v-if="showEditor" class="modal-mask" @click.self="showEditor = false">
      <div class="modal">
        <div class="modal-title">{{ editingItem.id.startsWith('user_') && !editingItem.name ? '添加工具' : '编辑工具' }}</div>
        <label class="field">
          <span>名称</span>
          <input v-model="editingItem.name" placeholder="如：打开我的世界" />
        </label>
        <label class="field">
          <span>图标（emoji）</span>
          <input v-model="editingItem.icon" placeholder="如：🎮" />
        </label>
        <label class="field">
          <span>命令</span>
          <textarea v-model="editingItem.command" rows="3" placeholder="如：D:\game\mc.exe 或 notepad" />
        </label>
        <div v-if="editorError" class="editor-error">{{ editorError }}</div>
        <div class="modal-actions">
          <button class="btn cancel" @click="showEditor = false">取消</button>
          <button class="btn confirm" @click="saveItem">保存</button>
        </div>
      </div>
    </div>

    <!-- 删除确认 -->
    <div v-if="showDeleteConfirm" class="modal-mask" @click.self="showDeleteConfirm = false">
      <div class="modal small">
        <div class="modal-title">删除这个工具？</div>
        <div class="modal-actions">
          <button class="btn cancel" @click="showDeleteConfirm = false">取消</button>
          <button class="btn danger" @click="confirmDelete">删除</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.toolbox-panel {
  position: fixed;
  right: 16px;
  bottom: 100px;
  width: 300px;
  max-height: 82vh;
  display: flex;
  flex-direction: column;
  background: var(--bg-bar, rgba(30, 28, 32, 0.92));
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  border-radius: 14px;
  padding: 12px;
  backdrop-filter: blur(10px);
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.35);
  z-index: 500;
  color: var(--text-main, #eee6e7);
}
.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
  flex-shrink: 0;
}
.header-btns { display: flex; gap: 6px; }
.panel-title {
  font-weight: 600;
  font-size: var(--fs-14);
}
.title-sub { font-size: var(--fs-11); color: var(--text-secondary); font-weight: 400; margin-left: 4px; }
.add-btn {
  width: 26px;
  height: 26px;
  border-radius: 8px;
  border: none;
  background: var(--accent, #ff7a94);
  color: var(--text-user);
  font-size: var(--fs-16);
  cursor: pointer;
}
.close-btn {
  width: 26px;
  height: 26px;
  border-radius: 8px;
  border: none;
  background: rgba(128, 128, 128, 0.2);
  color: var(--text-main, #eee6e7);
  font-size: var(--fs-14);
  line-height: 1;
  cursor: pointer;
}
.close-btn:hover { background: var(--danger-bg); color: var(--danger); }
.feedback {
  flex-shrink: 0; /* 防止被九宫格挤压，保证文字完整显示 */
  min-height: 32px;
  display: flex;
  align-items: center;
  margin-bottom: 8px;
  padding: 6px 10px;
  border-radius: 8px;
  font-size: var(--fs-12);
  word-break: break-all;
  max-height: 80px;
  overflow-y: auto;
}
.feedback.ok { background: var(--success-bg); color: var(--success); }
.feedback.err { background: var(--danger-bg); color: var(--danger); }
/* —— D 批次：执行中"铃正在努力"动态提示 —— */
.executing-hint {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  padding: 7px 10px;
  border-radius: 10px;
  background: rgba(255, 122, 148, 0.08);
  border: 1px solid rgba(255, 122, 148, 0.2);
  font-size: var(--fs-12);
  color: var(--text-main);
  min-height: 32px;
}
.exec-spinner {
  display: inline-block;
  animation: exec-rotate 1s linear infinite;
  font-size: var(--fs-13);
}
.exec-text { color: var(--accent, #ff7a94); font-weight: 500; }
.exec-phase { color: var(--text-secondary); font-size: var(--fs-11); }
@keyframes exec-rotate {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
.grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
  flex: 1;
  overflow-y: auto;
  padding-right: 2px;
  min-height: 0;
}
.cell {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 14px 6px;
  min-height: 82px;
  border-radius: 12px;
  background: var(--input-bg, #2a272b);
  cursor: pointer;
  transition: transform 0.12s ease, background 0.12s ease;
  position: relative;
}
.cell:hover {
  transform: translateY(-2px);
  background: rgba(255, 138, 171, 0.18);
}
.cell.running { opacity: 0.6; }
.cell.disabled { opacity: 0.4; }
.cell-icon {
  font-size: var(--fs-32);
  line-height: 1;
}
.cell-name {
  font-size: var(--fs-13);
  text-align: center;
  word-break: break-all;
}
.spinner {
  position: absolute;
  top: 4px;
  right: 6px;
  font-size: var(--fs-12);
}
.loading-cell {
  color: var(--text-secondary, #9a9294);
  font-size: var(--fs-12);
}
.panel-footer {
  margin-top: 10px;
  text-align: center;
  font-size: var(--fs-11);
  color: var(--text-secondary, #9a9294);
}
.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 600;
  padding: 16px;
  box-sizing: border-box;
}
.modal {
  width: 320px;
  max-height: 88vh;
  overflow-y: auto;
  box-sizing: border-box;
  background: var(--bg-bar, #262328);
  border: 1px solid var(--border, rgba(255, 255, 255, 0.15));
  border-radius: 14px;
  padding: 18px;
}
.modal.small { width: 240px; }
.output-modal { width: 560px; max-width: 90vw; }
/* P2：错误恢复动作栏 */
.error-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 12px;
  padding: 8px 10px;
  border-radius: 8px;
  background: rgba(255, 152, 0, 0.08);
  border: 1px solid rgba(255, 152, 0, 0.25);
}
.error-actions-title {
  font-size: var(--fs-12);
  color: var(--warning, #f0ad4e);
  font-weight: 600;
}
.action-btn {
  font-size: var(--fs-11);
  padding: 3px 10px;
  border-radius: 12px;
  border: 1px solid var(--border, rgba(128, 128, 128, 0.3));
  background: transparent;
  color: var(--text-main);
  cursor: pointer;
  transition: all 0.15s;
}
.action-btn:hover { border-color: var(--accent, #ff7a94); color: var(--accent, #ff7a94); }
.qr-preview {
  display: block;
  margin: 0 auto 12px;
  width: 200px;
  height: 200px;
  border-radius: 8px;
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  image-rendering: pixelated;
  background: #ffffff; /* 输出预览区固定白色 */
}
.output-pre {
  margin: 0 0 12px;
  padding: 10px;
  max-height: 60vh;
  overflow: auto;
  background: var(--input-bg, #1e1c20);
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  border-radius: 8px;
  font-family: Consolas, 'Courier New', monospace;
  font-size: var(--fs-12);
  line-height: 1.6;
  color: var(--text-main, #eee6e7);
  white-space: pre-wrap;
  word-break: break-all;
}
.modal-title {
  font-weight: 600;
  margin-bottom: 14px;
}
.field {
  display: block;
  margin-bottom: 12px;
}
.field span {
  display: block;
  font-size: var(--fs-12);
  color: var(--text-secondary, #9a9294);
  margin-bottom: 4px;
}
.field input,
.field textarea {
  width: 100%;
  box-sizing: border-box;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid var(--border, rgba(255, 255, 255, 0.15));
  background: var(--input-bg, #2a272b);
  color: var(--text-main, #eee6e7);
  font-size: var(--fs-13);
  resize: vertical;
}
.editor-error {
  color: var(--danger);
  font-size: var(--fs-12);
  margin-bottom: 10px;
}
.dep-box { text-align: left; }
.dep-title { margin: 0 0 10px; color: var(--text-main, #eee); font-size: var(--fs-16); }
.dep-hint { margin: 0 0 8px; color: var(--text-secondary, #aaa); font-size: var(--fs-13); }
.dep-install {
  margin: 0 0 14px;
  padding: 10px 12px;
  border-radius: 8px;
  background: rgba(128, 128, 128, 0.12);
  color: var(--text-main, #eee);
  font-family: 'Cascadia Mono', Consolas, monospace;
  font-size: var(--fs-12);
  word-break: break-all;
  white-space: pre-wrap;
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}
.btn {
  padding: 6px 16px;
  border-radius: 8px;
  border: none;
  cursor: pointer;
  font-size: var(--fs-13);
}
.btn.cancel { background: rgba(128, 128, 128, 0.2); color: var(--text-main); }
.btn.confirm { background: var(--accent, #ff7a94); color: var(--text-user); }
.btn.danger { background: var(--danger); color: var(--text-user); }
</style>
