<!-- 《铃·记忆体》快捷指令管理面板（AI-9）
     新增/编辑/删除指令（名称 + 步骤列表 + 可选铃说句话），列表展示 + 一键执行 -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import type { QuickCommand, QuickCommandStep } from '../types'
import { useQuickCommandStore } from '../stores/quickCommandStore'
import { useDesktopStore } from '../stores/desktopStore'

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
const SYS_MODULES: { tool: string; name: string; icon: string; input_label?: string; placeholder?: string }[] = [
  { tool: 'volume', name: '音量', icon: '🔊', input_label: '音量数值（0-100）', placeholder: '20' },
  { tool: 'music', name: '音乐', icon: '🎵', input_label: '音乐文件路径（可选）', placeholder: 'C:\\music\\song.mp3' },
  { tool: 'power-balanced', name: '电源·平衡', icon: '🔋' },
  { tool: 'power-high', name: '电源·高性能', icon: '⚡' },
  { tool: 'power-saver', name: '电源·省电', icon: '🍃' },
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
    list.push({ ...s, cat: 'qc' })
  }
  for (const c of MODULE_CATEGORIES) {
    for (const id of c.ids) {
      const t = toolboxModule(id)
      if (t) list.push({ tool: t.id, name: t.name, icon: t.icon || '🔧', input_label: t.input_label ?? undefined, placeholder: t.input_placeholder ?? undefined, cat: c.key })
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
    alert('请先填写指令名称')
    return
  }
  // 模块模式下，先从模块列表同步步骤文本（保存统一走 steps）
  if (editMode.value === 'module') {
    stepsText.value = stepsToText(moduleSteps.value)
  }
  const steps = parseSteps(stepsText.value)
  if (steps.length === 0) {
    alert('请至少填写一个步骤（每行一个动作）')
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
  if (!confirm('确定删除这条快捷指令吗？')) return
  await store.remove(id)
}

async function runCmd(cmd: QuickCommand) {
  // 危险操作确认
  const dangerous = cmd.steps.filter((s) => DANGEROUS_TOOLS.has(s.tool)).map((s) => s.tool)
  if (dangerous.length > 0) {
    const ok = confirm(`该指令包含危险操作（${dangerous.join('、')}），可能影响系统或无法恢复，是否继续？`)
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
  <div>
    <section class="card">
      <div class="card-title">快捷指令</div>
      <p class="hint">
        把一串动作组合成一条指令（如「晚安模式」：设电源平衡 → 清临时文件 → 音量 20% → 启动音乐 → 铃说晚安）。在聊天里直接说指令名即可触发。
        每行一个动作，格式「工具名」或「工具名=输入」。可用系统工具：volume（0-100）、music、power-balanced / power-high / power-saver，也可填任意工具箱工具 id（如 clean-temp、clean-memory）。
      </p>

      <!-- 编辑区 -->
      <div class="field">
        <label>指令名称（聊天里说这个名字触发）</label>
        <input v-model="name" class="input" placeholder="如：晚安模式" />
      </div>

      <!-- 编辑模式切换：手敲 / 模块 -->
      <div class="mode-switch">
        <button class="btn ghost mode-btn" :class="{ on: editMode === 'text' }" @click="switchMode('text')">✍️ 手敲</button>
        <button class="btn ghost mode-btn" :class="{ on: editMode === 'module' }" @click="switchMode('module')">🧩 模块</button>
      </div>

      <!-- 手敲模式：文本步骤 -->
      <div v-if="editMode === 'text'" class="field">
        <label>步骤列表（每行一个动作）</label>
        <textarea v-model="stepsText" class="input area" rows="5" placeholder="power-balanced&#10;clean-temp&#10;volume=20&#10;music"></textarea>
      </div>

      <!-- 模块模式：模块库 + 已选步骤 -->
      <div v-else class="module-editor">
        <label class="field-label">模块库（点击或拖拽添加，可用系统命令与工具箱工具）</label>
        <div class="mod-lib">
          <div
            v-for="cat in MODULE_CATEGORIES"
            :key="cat.key"
            class="mod-cat"
          >
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
                <span class="mod-icon">{{ m.icon }}</span>
                <span class="mod-name">{{ m.name }}</span>
                <span class="mod-add">+</span>
              </div>
            </div>
          </div>
          <p v-if="library.length === 0" class="hint">工具箱未加载，请先打开工具箱或稍后重试。</p>
        </div>

        <label class="field-label">已选步骤（{{ moduleSteps.length }}）—— 拖入/点击添加，拖动排序，可填参数</label>
        <div
          class="mod-steps"
          @dragover.prevent
          @drop="onDropOnList"
        >
          <div
            v-for="(s, i) in moduleSteps"
            :key="i"
            class="mod-step"
            draggable="true"
            :class="{ over: dragOverIdx === i }"
            @dragstart="onStepDragStart(i)"
            @dragover.prevent
            @drop="onStepDrop(i)"
          >
            <span class="mod-step-idx">{{ i + 1 }}</span>
            <span class="mod-step-tool">{{ s.tool }}</span>
            <input
              v-model="s.input"
              class="input mod-step-input"
              :placeholder="s.input ? '' : '（可选参数）'"
            />
            <button class="btn ghost step-btn" title="上移" @click="moveModule(i, -1)">↑</button>
            <button class="btn ghost step-btn" title="下移" @click="moveModule(i, 1)">↓</button>
            <button class="btn ghost step-btn del" title="删除" @click="removeModule(i)">✕</button>
          </div>
          <p v-if="moduleSteps.length === 0" class="hint">还没有步骤，从上方模块库点一下或拖一个过来吧～</p>
        </div>
      </div>

      <div class="field">
        <label>执行完铃说的话（可选）</label>
        <input v-model="say" class="input" placeholder="如：晚安，主人～" />
      </div>
      <div class="row">
        <button class="btn primary" @click="saveCmd">{{ editingId ? '保存修改' : '新增指令' }}</button>
        <button class="btn ghost" @click="newCmd">清空</button>
      </div>
    </section>

    <!-- 列表区 -->
    <section class="card">
      <div class="card-title">已保存的指令</div>
      <p v-if="store.commands.length === 0" class="hint">还没有快捷指令，先在上方创建一个吧。</p>
      <div v-for="c in store.commands" :key="c.id" class="qc-item">
        <div class="qc-main">
          <div class="qc-name">{{ c.name }}</div>
          <div class="qc-steps">{{ c.steps.map((s) => (s.input ? `${s.tool}(${s.input})` : s.tool)).join(' → ') }}</div>
          <div v-if="c.say" class="qc-say">铃：{{ c.say }}</div>
        </div>
        <div class="qc-actions">
          <button class="btn primary" :disabled="running" @click="runCmd(c)">执行</button>
          <button class="btn ghost" @click="editCmd(c)">编辑</button>
          <button class="btn danger" @click="delCmd(c.id)">删除</button>
        </div>
      </div>
    </section>

    <!-- 执行反馈 -->
    <section v-if="runMsg" class="card">
      <div class="card-title">执行结果</div>
      <p class="hint">{{ runMsg }}</p>
      <ul v-if="runResults.length" class="qc-results">
        <li v-for="(r, i) in runResults" :key="i">{{ r }}</li>
      </ul>
      <p v-if="runSay" class="qc-say-result">{{ runSay }}</p>
    </section>
  </div>
</template>

<style scoped>
/* 统一继承全局字体，避免浏览器默认字体差异 */
section, .card-title, label, .input, .hint, .btn, p, ul, li {
  font-family: inherit;
}
/* 与全局其他面板（SyncPanel/PluginManager 等）保持一致的卡片样式 */
.card {
  background: var(--bg-bar, rgba(34, 32, 36, 0.85));
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  border-radius: 14px;
  padding: 14px 16px;
  margin-bottom: 10px;
}
.card-title { font-weight: 600; font-size: var(--fs-14); margin-bottom: 8px; }
.hint { font-size: var(--fs-12); color: var(--text-secondary); margin: 0 0 8px; line-height: 1.6; }
.row { display: flex; align-items: center; gap: 10px; margin-bottom: 8px; flex-wrap: wrap; }
.field { display: flex; flex-direction: column; gap: 4px; margin-bottom: 10px; }
.field label { font-size: var(--fs-12); color: var(--text-secondary); }
.input {
  padding: 7px 10px; border-radius: 8px; border: 1px solid var(--border);
  background: var(--input-bg); color: var(--text-main); font-size: var(--fs-13);
}
.btn { padding: 6px 14px; border-radius: 8px; border: none; cursor: pointer; font-size: var(--fs-13); }
.btn.primary { background: var(--accent, #ff7a94); color: var(--text-user); }
.btn.ghost { background: rgba(128, 128, 128, 0.18); color: var(--text-main); }
.btn.danger { background: var(--danger-bg); color: var(--danger); }
.btn:disabled { opacity: 0.5; cursor: not-allowed; }
.area {
  resize: vertical;
  min-height: 90px;
  font-family: inherit;
  line-height: 1.6;
}
.qc-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px;
  border-radius: 10px;
  background: rgba(128, 128, 128, 0.1);
  margin-bottom: 8px;
}
.qc-main { flex: 1; min-width: 0; }
.qc-name { font-size: var(--fs-14); font-weight: 600; }
.qc-steps { font-size: var(--fs-12); color: var(--text-secondary); margin-top: 2px; word-break: break-all; }
.qc-say { font-size: var(--fs-12); color: var(--accent, #ff7a94); margin-top: 2px; }
.qc-actions { display: flex; gap: 6px; flex-shrink: 0; }
.qc-results { margin: 6px 0; padding-left: 18px; font-size: var(--fs-12); color: var(--text-secondary); line-height: 1.7; }

/* —— moon12-3 模块化编辑样式 —— */
.mode-switch { display: flex; gap: 8px; margin-bottom: 10px; }
.mode-btn { padding: 5px 14px; border-radius: 8px; }
.mode-btn.on { border: 1px solid var(--accent); color: var(--accent); background: var(--accent-bg, rgba(255, 122, 148, 0.12)); }
.field-label { font-size: var(--fs-12); color: var(--text-secondary); display: block; margin-bottom: 6px; }
.module-editor { margin-bottom: 10px; }
.mod-lib {
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  border-radius: 10px;
  padding: 10px;
  background: rgba(128, 128, 128, 0.06);
  margin-bottom: 12px;
  max-height: 260px;
  overflow-y: auto;
}
.mod-cat { margin-bottom: 8px; }
.mod-cat:last-child { margin-bottom: 0; }
.mod-cat-title { font-size: var(--fs-11); color: var(--text-secondary); margin-bottom: 4px; font-weight: 600; }
.mod-cat-items { display: flex; flex-wrap: wrap; gap: 6px; }
.mod-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 4px 10px;
  border-radius: 14px;
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  background: var(--bg-bar, rgba(34, 32, 36, 0.85));
  cursor: grab;
  font-size: var(--fs-12);
  color: var(--text-main);
  transition: all 0.15s;
  user-select: none;
}
.mod-chip:hover { border-color: var(--accent); color: var(--accent); }
.mod-chip:active { cursor: grabbing; }
.mod-icon { font-size: var(--fs-13); }
.mod-add { color: var(--accent); font-weight: 700; opacity: 0.7; }
.mod-steps {
  border: 1px dashed var(--border, rgba(255, 255, 255, 0.2));
  border-radius: 10px;
  padding: 8px;
  min-height: 60px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.mod-step {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 8px;
  background: rgba(128, 128, 128, 0.1);
  cursor: grab;
}
.mod-step.over { border: 1px solid var(--accent); background: var(--accent-bg, rgba(255, 122, 148, 0.12)); }
.mod-step-idx {
  width: 20px; height: 20px;
  border-radius: 50%;
  background: var(--accent, #ff7a94);
  color: var(--text-user);
  font-size: var(--fs-11);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.mod-step-tool { font-weight: 600; font-size: var(--fs-12); min-width: 90px; }
.mod-step-input { flex: 1; min-width: 120px; padding: 4px 8px; font-size: var(--fs-12); }
.step-btn { padding: 2px 8px; font-size: var(--fs-12); flex-shrink: 0; }
.step-btn.del { color: var(--danger); }
.qc-say-result { font-size: var(--fs-13); color: var(--accent, #ff7a94); margin-top: 6px; }
</style>
