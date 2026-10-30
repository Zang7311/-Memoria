<!-- 《铃·记忆体》快捷指令管理面板（设置 → 指令）
     新增/编辑/删除指令（名称 + 步骤列表 + 可选铃说句话），列表展示 + 一键执行
     皮肤：全部走 --ui-* 令牌；图标走 TheIcon，不再用 emoji -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import type { QuickCommand, QuickCommandStep } from '../types'
import { useQuickCommandStore } from '../stores/quickCommandStore'
import { useDesktopStore } from '../stores/desktopStore'
import { confirmDialog } from '../utils/dialog'
import { toastErr } from '../utils/toast'
import TheIcon from './TheIcon.vue'
import AppButton from './ui/AppButton.vue'
import AppCard from './ui/AppCard.vue'
import AppEmpty from './ui/AppEmpty.vue'
import AppInput from './ui/AppInput.vue'

const store = useQuickCommandStore()
const desktop = useDesktopStore()

// —— 编辑表单 ——
const name = ref('')
const stepsText = ref('')
const say = ref('')
const editingId = ref<string | null>(null)
// —— moon12-3 模块化编辑：text=手敲 / module=拖拽模块 ——
const editMode = ref<'text' | 'module'>('text')
// 模块模式：已选步骤（tool + 参数输入）
const moduleSteps = ref<QuickCommandStep[]>([])
// 拖拽相关
const dragToolId = ref<string | null>(null)
const dragOverIdx = ref<number | null>(null)

// —— 执行反馈 ——
const runMsg = ref('')
const runResults = ref<string[]>([])
const runSay = ref('')
const running = ref(false)

// —— 危险操作清单（不可逆/系统级，执行前需用户确认）——
const DANGEROUS_TOOLS = new Set(['shutdown-1h', 'lock', 'shred', 'bsod', 'empty-recycle-bin', 'cancel-shutdown', 'format-disk'])

// —— 模块库：系统命令（快捷指令专用） + 工具箱工具 ——
const SYS_MODULES: { tool: string; name: string; input_label?: string; placeholder?: string }[] = [
  { tool: 'volume', name: '音量', input_label: '音量数值（0-100）', placeholder: '20' },
  { tool: 'music', name: '音乐', input_label: '音乐文件路径（可选）', placeholder: 'C:\\music\\song.mp3' },
  { tool: 'power-balanced', name: '电源·平衡' },
  { tool: 'power-high', name: '电源·高性能' },
  { tool: 'power-saver', name: '电源·省电' },
]

// 模块分类（工具箱工具按 id 归类，便于浏览）
const MODULE_CATEGORIES: { key: string; label: string; ids: string[] }[] = [
  { key: 'qc', label: '快捷指令', ids: [] }, // 系统命令（SYS_MODULES）归入此类
  { key: 'app', label: '应用', ids: ['explorer', 'calc', 'notepad', 'taskmgr', 'open-cmd', 'open-powershell', 'vm', 'alarm', 'open-file', 'open-app'] },
  { key: 'sys', label: '系统', ids: ['lock', 'shutdown-1h', 'cancel-shutdown', 'bsod', 'empty-recycle-bin', 'clipboard-history', 'screenshot', 'format-disk', 'install-python', 'install-jdk'] },
  { key: 'power', label: '电源/清理', ids: ['power-saver', 'power-balanced', 'power-high', 'clean-temp', 'clean-memory'] },
  { key: 'net', label: '网络', ids: ['network', 'port-list', 'lan-ip', 'wifi-pwd', 'ping', 'ip-lookup', 'speedtest'] },
  { key: 'tool', label: '工具', ids: ['fortune', 'cpu-test', 'benchmark', 'encrypt', 'decrypt', 'shred', 'compress', 'uncompress', 'clicker', 'ffmpeg', 'xor', 'pixel-art', 'regex', 'batch-rename', 'ocr', 'qrcode-gen', 'qrcode-decode'] },
]

// 工具 id → TheIcon 图标名（不再用 emoji；未知 id 兜底 command）
const TOOL_ICONS: Record<string, string> = {
  // 快捷指令专用系统命令
  volume: 'music',
  music: 'music',
  'power-saver': 'power',
  'power-balanced': 'power',
  'power-high': 'power',
  // 应用
  explorer: 'file',
  'open-file': 'file',
  'open-app': 'file',
  calc: 'calculator',
  notepad: 'edit',
  taskmgr: 'monitor',
  vm: 'monitor',
  'open-cmd': 'command',
  'open-powershell': 'command',
  // 系统
  lock: 'lock',
  'shutdown-1h': 'power',
  'cancel-shutdown': 'power',
  bsod: 'diagnostic',
  'empty-recycle-bin': 'clean',
  'clipboard-history': 'file',
  screenshot: 'screenshot',
  'format-disk': 'delete',
  'install-python': 'toolbox',
  'install-jdk': 'toolbox',
  // 电源 / 清理
  'clean-temp': 'clean',
  'clean-memory': 'memory',
  // 网络
  network: 'network',
  'port-list': 'network',
  'lan-ip': 'network',
  'wifi-pwd': 'network',
  ping: 'network',
  'ip-lookup': 'network',
  speedtest: 'network',
  // 工具
  fortune: 'chat',
  'cpu-test': 'diagnostic',
  benchmark: 'diagnostic',
  encrypt: 'lock',
  decrypt: 'lock',
  shred: 'delete',
  compress: 'file',
  uncompress: 'file',
  clicker: 'send',
  ffmpeg: 'music',
  'pixel-art': 'customize',
  regex: 'search',
  'batch-rename': 'edit',
  ocr: 'search',
  'qrcode-decode': 'search',
}
function toolIcon(id: string): string {
  return TOOL_ICONS[id] ?? 'command'
}

// 从工具箱加载工具模块（挂载时）
onMounted(async () => {
  store.load()
  await desktop.loadToolboxItems()
})
// 工具箱工具转模块（按 id 查）
function toolboxModule(id: string) {
  return desktop.toolboxItems.find((t) => t.id === id)
}
// 模块库完整列表（系统命令 + 工具箱工具，按分类）
const library = computed(() => {
  const list: { tool: string; name: string; icon: string; input_label?: string; placeholder?: string; cat: string }[] = []
  // 快捷指令专用系统命令（不在工具箱里）
  for (const s of SYS_MODULES) {
    list.push({ ...s, icon: toolIcon(s.tool), cat: 'qc' })
  }
  for (const c of MODULE_CATEGORIES) {
    for (const id of c.ids) {
      const t = toolboxModule(id)
      if (t) list.push({ tool: t.id, name: t.name, icon: toolIcon(t.id), input_label: t.input_label ?? undefined, placeholder: t.input_placeholder ?? undefined, cat: c.key })
    }
  }
  return list
})

// 步骤文本解析：每行一条动作，格式「工具名」或「工具名=输入」
function parseSteps(text: string): QuickCommandStep[] {
  return text
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l.length > 0)
    .map((line) => {
      const idx = line.search(/[=:,]/)
      if (idx === -1) return { tool: line, input: null }
      return { tool: line.slice(0, idx).trim(), input: line.slice(idx + 1).trim() }
    })
    .filter((s) => s.tool.length > 0)
}

// 步骤对象转文本（编辑回显）
function stepsToText(steps: QuickCommandStep[]): string {
  return steps.map((s) => (s.input ? `${s.tool}=${s.input}` : s.tool)).join('\n')
}

function newCmd() {
  editingId.value = null
  name.value = ''
  stepsText.value = ''
  say.value = ''
  moduleSteps.value = []
  editMode.value = 'text'
}

function editCmd(cmd: QuickCommand) {
  editingId.value = cmd.id
  name.value = cmd.name
  stepsText.value = stepsToText(cmd.steps)
  say.value = cmd.say ?? ''
  moduleSteps.value = cmd.steps.map((s) => ({ tool: s.tool, input: s.input ?? null }))
  editMode.value = 'text'
}

// —— 模块模式操作 ——
function switchMode(mode: 'text' | 'module') {
  // 切换时同步数据：手敲文本 ↔ 模块列表
  if (mode === 'module') {
    // 文本 → 模块（已有模块保留，合并去重）
    for (const s of parseSteps(stepsText.value)) {
      if (!moduleSteps.value.some((m) => m.tool === s.tool && m.input === s.input)) {
        moduleSteps.value.push(s)
      }
    }
  } else {
    stepsText.value = stepsToText(moduleSteps.value)
  }
  editMode.value = mode
}
function addModule(tool: string) {
  moduleSteps.value.push({ tool, input: null })
}
function removeModule(idx: number) {
  moduleSteps.value.splice(idx, 1)
}
function moveModule(idx: number, dir: -1 | 1) {
  const j = idx + dir
  if (j < 0 || j >= moduleSteps.value.length) return
  const arr = moduleSteps.value
  ;[arr[idx], arr[j]] = [arr[j], arr[idx]]
}
// 拖拽：从模块库拖入列表
function onDragStart(tool: string) {
  dragToolId.value = tool
}
function onDropOnList() {
  if (dragToolId.value) {
    addModule(dragToolId.value)
    dragToolId.value = null
  }
  dragOverIdx.value = null
}
// 步骤内拖拽排序（简化为上下移，另有拖拽插入）
function onStepDragStart(idx: number) {
  dragOverIdx.value = idx
}
function onStepDrop(idx: number) {
  const from = dragOverIdx.value
  if (from !== null && from !== idx) {
    const arr = moduleSteps.value
    const [item] = arr.splice(from, 1)
    arr.splice(idx, 0, item)
  }
  dragOverIdx.value = null
}

async function saveCmd() {
  if (!name.value.trim()) {
    toastErr('请先填写指令名称')
    return
  }
  // 模块模式下，先从模块列表同步步骤文本（保存统一走 steps）
  if (editMode.value === 'module') {
    stepsText.value = stepsToText(moduleSteps.value)
  }
  const steps = parseSteps(stepsText.value)
  if (steps.length === 0) {
    toastErr('请至少填写一个步骤（每行一个动作）')
    return
  }
  const cmd: QuickCommand = {
    id: editingId.value ?? `qc_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
    name: name.value.trim(),
    steps,
    say: say.value.trim() ? say.value.trim() : null,
  }
  await store.save(cmd)
  newCmd()
}

async function delCmd(id: string) {
  if (!(await confirmDialog('确定删除这条快捷指令吗？', { danger: true, confirmText: '删除' }))) return
  await store.remove(id)
}

async function runCmd(cmd: QuickCommand) {
  // 危险操作确认
  const dangerous = cmd.steps.filter((s) => DANGEROUS_TOOLS.has(s.tool)).map((s) => s.tool)
  if (dangerous.length > 0) {
    const ok = await confirmDialog(
      `该指令包含危险操作（${dangerous.join('、')}），可能影响系统或无法恢复，是否继续？`,
      { danger: true, confirmText: '继续执行' },
    )
    if (!ok) return
  }
  running.value = true
  runMsg.value = `正在执行「${cmd.name}」…`
  runResults.value = []
  runSay.value = ''
  try {
    const res = await store.execute(cmd.id)
    if (!res) {
      runMsg.value = '执行失败（IPC 调用出错）'
    } else if (res.error) {
      runMsg.value = `执行出错：${res.error}`
    } else {
      runMsg.value = `「${cmd.name}」执行完成`
      runResults.value = res.results
      runSay.value = res.say ?? ''
    }
  } catch (e) {
    runMsg.value = `执行出错：${e}`
  } finally {
    running.value = false
  }
}

onMounted(() => {
  store.load()
})
</script>

<template>
  <div class="qc-page">
    <!-- 编辑区 -->
    <AppCard title="快捷指令" hint="把多个动作串成一条">
      <p class="hint">把一串动作组合成一条指令（如「晚安模式」：设电源平衡 → 清临时文件 → 音量 20% → 启动音乐 → 铃说晚安）。在聊天里直接说指令名即可触发。</p>
      <p class="hint">每行一个动作，格式「工具名」或「工具名=输入」。可用系统工具：volume（0-100）、music、power-balanced / power-high / power-saver，也可填任意工具箱工具 id（如 clean-temp、clean-memory）。</p>

      <div class="field">
        <label class="field-label" for="qc-name">指令名称（聊天里说这个名字触发）</label>
        <AppInput id="qc-name" v-model="name" placeholder="如：晚安模式" />
      </div>

      <!-- 编辑模式切换：手敲 / 模块 -->
      <div class="mode-switch" role="group" aria-label="编辑模式">
        <AppButton size="sm" :variant="editMode === 'text' ? 'ghost' : 'text'" @click="switchMode('text')">
          <TheIcon name="edit" :size="14" />手敲步骤
        </AppButton>
        <AppButton size="sm" :variant="editMode === 'module' ? 'ghost' : 'text'" @click="switchMode('module')">
          <TheIcon name="customize" :size="14" />模块拼接
        </AppButton>
      </div>

      <!-- 手敲模式：文本步骤 -->
      <div v-if="editMode === 'text'" class="field">
        <label class="field-label" for="qc-steps">步骤列表（每行一个动作）</label>
        <textarea
          id="qc-steps"
          v-model="stepsText"
          class="qc-textarea"
          rows="5"
          placeholder="power-balanced&#10;clean-temp&#10;volume=20&#10;music"
        />
      </div>

      <!-- 模块模式：模块库 + 已选步骤 -->
      <div v-else class="module-editor">
        <p class="field-label">模块库（点击或拖拽添加，可用系统命令与工具箱工具）</p>
        <div class="mod-lib">
          <div v-for="cat in MODULE_CATEGORIES" :key="cat.key" class="mod-cat">
            <div class="mod-cat-title">{{ cat.label }}</div>
            <div class="mod-cat-items">
              <div
                v-for="m in library.filter((x) => x.cat === cat.key)"
                :key="m.tool"
                class="mod-chip"
                draggable="true"
                :title="`${m.name}${m.input_label ? '（需输入：' + m.input_label + '）' : ''}`"
                @click="addModule(m.tool)"
                @dragstart="onDragStart(m.tool)"
              >
                <TheIcon :name="m.icon" :size="13" />
                <span class="mod-name">{{ m.name }}</span>
                <span class="mod-add"><TheIcon name="add" :size="12" /></span>
              </div>
            </div>
          </div>
          <p v-if="library.length === 0" class="hint">工具箱未加载，请先打开工具箱或稍后重试。</p>
        </div>

        <p class="field-label">已选步骤（{{ moduleSteps.length }}）—— 拖入/点击添加，拖动排序，可填参数</p>
        <div class="mod-steps" @dragover.prevent @drop="onDropOnList">
          <div
            v-for="(s, i) in moduleSteps"
            :key="i"
            class="mod-step"
            :class="{ over: dragOverIdx === i }"
            draggable="true"
            @dragstart="onStepDragStart(i)"
            @dragover.prevent
            @drop="onStepDrop(i)"
          >
            <span class="mod-step-idx">{{ i + 1 }}</span>
            <span class="mod-step-tool">{{ s.tool }}</span>
            <input
              v-model="s.input"
              class="mod-step-input"
              :placeholder="s.input ? '' : '（可选参数）'"
            />
            <button class="step-btn" type="button" title="上移这一步" @click="moveModule(i, -1)">上移</button>
            <button class="step-btn" type="button" title="下移这一步" @click="moveModule(i, 1)">下移</button>
            <button class="step-btn danger" type="button" title="删除这一步" @click="removeModule(i)">删除</button>
          </div>
          <p v-if="moduleSteps.length === 0" class="hint center">还没有步骤，从上方模块库点一下，或拖一个过来。</p>
        </div>
      </div>

      <div class="field">
        <label class="field-label" for="qc-say">执行完铃说的话（可选）</label>
        <AppInput id="qc-say" v-model="say" placeholder="如：晚安，主人～" />
      </div>

      <div class="form-actions">
        <AppButton variant="primary" @click="saveCmd">{{ editingId ? '保存修改' : '新增指令' }}</AppButton>
        <AppButton variant="text" @click="newCmd">清空</AppButton>
      </div>
    </AppCard>

    <!-- 列表区 -->
    <AppCard title="已保存的指令" :hint="store.commands.length ? `${store.commands.length} 条` : ''">
      <AppEmpty
        v-if="store.commands.length === 0"
        text="还没有快捷指令"
        hint="在上方填好名称与步骤，保存后就会出现在这里。"
      />
      <div v-for="c in store.commands" :key="c.id" class="qc-item">
        <div class="qc-main">
          <div class="qc-name">{{ c.name }}</div>
          <div class="qc-steps">{{ c.steps.map((s) => (s.input ? `${s.tool}(${s.input})` : s.tool)).join(' → ') }}</div>
          <div v-if="c.say" class="qc-say">铃：{{ c.say }}</div>
        </div>
        <div class="qc-actions">
          <AppButton size="sm" :disabled="running" @click="runCmd(c)">
            <TheIcon name="send" :size="13" />执行
          </AppButton>
          <AppButton size="sm" variant="text" @click="editCmd(c)">编辑</AppButton>
          <AppButton size="sm" variant="text" class="danger" @click="delCmd(c.id)">删除</AppButton>
        </div>
      </div>
    </AppCard>

    <!-- 执行反馈 -->
    <AppCard v-if="runMsg" title="执行结果">
      <p class="hint run-msg" role="status">{{ runMsg }}</p>
      <ul v-if="runResults.length" class="qc-results">
        <li v-for="(r, i) in runResults" :key="i">{{ r }}</li>
      </ul>
      <p v-if="runSay" class="qc-say-result">铃：{{ runSay }}</p>
    </AppCard>
  </div>
</template>

<style scoped>
/* 设置页内容区里的整列：纵向排列、不产生横向滚动 */
.qc-page {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-3);
  min-width: 0;
  font-size: var(--ui-fs-13);
  color: var(--ui-text);
}

.hint {
  margin: 0 0 var(--ui-sp-2);
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
  line-height: var(--ui-line);
  overflow-wrap: anywhere;
}
.hint.center {
  text-align: center;
  color: var(--ui-text-3);
}
.hint:last-child {
  margin-bottom: 0;
}

/* —— 表单 —— */
.field {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
  margin-bottom: var(--ui-sp-2);
  min-width: 0;
}
.field-label {
  display: block;
  margin: 0;
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
  line-height: var(--ui-line);
  overflow-wrap: anywhere;
}
.qc-textarea {
  display: block;
  width: 100%;
  min-width: 0;
  min-height: 96px;
  padding: var(--ui-sp-2);
  background: var(--ui-raised);
  color: var(--ui-text);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-sm);
  font-family: var(--font-mono);
  font-size: var(--ui-fs-13);
  line-height: var(--ui-line);
  resize: vertical;
}
.qc-textarea:focus {
  border-color: var(--ui-accent);
}
.qc-textarea::placeholder {
  color: var(--ui-text-3);
}
.form-actions {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  margin-top: var(--ui-sp-3);
  flex-wrap: wrap;
}

/* —— 模式切换 —— */
.mode-switch {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-1);
  margin-bottom: var(--ui-sp-3);
  flex-wrap: wrap;
}

/* —— 模块库 —— */
.module-editor {
  margin-bottom: var(--ui-sp-2);
  min-width: 0;
}
.mod-lib {
  max-height: 280px;
  overflow-y: auto;
  margin-bottom: var(--ui-sp-3);
  padding: var(--ui-sp-2);
  background: var(--ui-bg);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-md);
}
.mod-cat + .mod-cat {
  margin-top: var(--ui-sp-2);
}
.mod-cat-title {
  margin-bottom: var(--ui-sp-1);
  font-size: var(--ui-fs-11);
  font-weight: 600;
  color: var(--ui-text-3);
}
.mod-cat-items {
  display: flex;
  flex-wrap: wrap;
  gap: var(--ui-sp-1);
}
.mod-chip {
  display: inline-flex;
  align-items: center;
  gap: var(--ui-sp-1);
  max-width: 100%;
  padding: var(--ui-sp-1) var(--ui-sp-2);
  background: var(--ui-raised);
  color: var(--ui-text);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-pill);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  cursor: grab;
  user-select: none;
  transition: border-color var(--ui-dur-1) var(--ui-ease), color var(--ui-dur-1) var(--ui-ease),
    background var(--ui-dur-1) var(--ui-ease);
}
.mod-chip:hover {
  background: var(--ui-hover);
  border-color: var(--ui-accent-line);
  color: var(--ui-accent);
}
.mod-chip:active {
  cursor: grabbing;
}
.mod-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mod-add {
  display: inline-flex;
  align-items: center;
  color: var(--ui-accent);
  opacity: 0.75;
}

/* —— 已选步骤（可拖入 / 拖动排序）—— */
.mod-steps {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
  min-height: 60px;
  padding: var(--ui-sp-2);
  background: var(--ui-bg);
  border: 1px dashed var(--ui-border-strong);
  border-radius: var(--ui-r-md);
}
.mod-step {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  min-width: 0;
  padding: var(--ui-sp-1) var(--ui-sp-2);
  background: var(--ui-raised);
  border: 1px solid transparent;
  border-radius: var(--ui-r-sm);
  cursor: grab;
}
.mod-step.over {
  border-color: var(--ui-accent-line);
  background: var(--ui-accent-soft);
}
.mod-step-idx {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  width: var(--ui-sp-5);
  height: var(--ui-sp-5);
  border-radius: var(--ui-r-pill);
  background: var(--ui-accent);
  color: var(--ui-on-accent);
  font-size: var(--ui-fs-11);
  line-height: 1;
}
.mod-step-tool {
  flex: none;
  max-width: 100%;
  font-size: var(--ui-fs-12);
  font-weight: 600;
  color: var(--ui-text);
  overflow-wrap: anywhere;
}
.mod-step-input {
  flex: 1 1 120px;
  min-width: 0;
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-2);
  background: var(--ui-bg);
  color: var(--ui-text);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-sm);
  font-size: var(--ui-fs-12);
}
.mod-step-input:focus {
  border-color: var(--ui-accent);
}
.mod-step-input::placeholder {
  color: var(--ui-text-3);
}
.step-btn {
  flex: none;
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-2);
  background: var(--ui-raised);
  color: var(--ui-text-2);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  font-size: var(--ui-fs-11);
  transition: background var(--ui-dur-1) var(--ui-ease), color var(--ui-dur-1) var(--ui-ease);
}
.step-btn:hover {
  background: var(--ui-hover);
  color: var(--ui-text);
}
.step-btn.danger {
  color: var(--ui-err);
}
.step-btn.danger:hover {
  background: color-mix(in srgb, var(--ui-err) 14%, transparent);
  color: var(--ui-err);
}

/* —— 已保存的指令 —— */
.qc-item {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  min-width: 0;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  background: var(--ui-bg);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-md);
}
.qc-item + .qc-item {
  margin-top: var(--ui-sp-2);
}
.qc-main {
  flex: 1 1 200px;
  min-width: 0;
}
.qc-name {
  font-size: var(--ui-fs-14);
  font-weight: 600;
  color: var(--ui-text);
  overflow-wrap: anywhere;
}
.qc-steps {
  margin-top: var(--ui-sp-1);
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
  line-height: var(--ui-line);
  overflow-wrap: anywhere;
}
.qc-say {
  margin-top: var(--ui-sp-1);
  font-size: var(--ui-fs-12);
  color: var(--ui-accent);
  overflow-wrap: anywhere;
}
.qc-actions {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-1);
  flex: none;
  flex-wrap: wrap;
}
/* 覆盖 AppButton 的文字色：删除是危险操作 */
.qc-actions :deep(.app-btn.danger) {
  color: var(--ui-err);
}
.qc-actions :deep(.app-btn.danger:hover:not(:disabled)) {
  color: var(--ui-err);
  background: color-mix(in srgb, var(--ui-err) 14%, transparent);
}

/* —— 执行结果 —— */
.run-msg {
  margin-bottom: 0;
}
.qc-results {
  margin: var(--ui-sp-2) 0 0;
  padding-left: var(--ui-sp-5);
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
  line-height: var(--ui-line);
}
.qc-results li {
  overflow-wrap: anywhere;
}
.qc-say-result {
  margin: var(--ui-sp-2) 0 0;
  font-size: var(--ui-fs-13);
  color: var(--ui-accent);
  line-height: var(--ui-line);
  overflow-wrap: anywhere;
}
</style>
