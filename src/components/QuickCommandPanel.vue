<!-- 《铃·记忆体》快捷指令管理面板（AI-9）
     新增/编辑/删除指令（名称 + 步骤列表 + 可选铃说句话），列表展示 + 一键执行 -->
<script setup lang="ts">
import { onMounted, ref } from 'vue'
import type { QuickCommand, QuickCommandStep } from '../types'
import { useQuickCommandStore } from '../stores/quickCommandStore'

const store = useQuickCommandStore()

// —— 编辑表单 ——
const name = ref('')
const stepsText = ref('')
const say = ref('')
const editingId = ref<string | null>(null)

// —— 执行反馈 ——
const runMsg = ref('')
const runResults = ref<string[]>([])
const runSay = ref('')
const running = ref(false)

// —— 危险操作清单（不可逆/系统级，执行前需用户确认）——
const DANGEROUS_TOOLS = new Set(['shutdown-1h', 'lock', 'shred', 'bsod', 'empty-recycle-bin', 'cancel-shutdown'])

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
}

function editCmd(cmd: QuickCommand) {
  editingId.value = cmd.id
  name.value = cmd.name
  stepsText.value = stepsToText(cmd.steps)
  say.value = cmd.say ?? ''
}

async function saveCmd() {
  if (!name.value.trim()) {
    alert('请先填写指令名称')
    return
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
      <div class="field">
        <label>步骤列表（每行一个动作）</label>
        <textarea v-model="stepsText" class="input area" rows="5" placeholder="power-balanced&#10;clean-temp&#10;volume=20&#10;music"></textarea>
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
.qc-name { font-size: 14px; font-weight: 600; }
.qc-steps { font-size: 12px; color: var(--text-secondary); margin-top: 2px; word-break: break-all; }
.qc-say { font-size: 12px; color: var(--accent, #ff7a94); margin-top: 2px; }
.qc-actions { display: flex; gap: 6px; flex-shrink: 0; }
.qc-results { margin: 6px 0; padding-left: 18px; font-size: 12px; color: var(--text-secondary); line-height: 1.7; }
.qc-say-result { font-size: 13px; color: var(--accent, #ff7a94); margin-top: 6px; }
</style>
