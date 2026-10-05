<script setup lang="ts">
import { ref } from 'vue'
import { useGoalStore } from '../stores/goalStore'
import type { AgentGoal } from '../types/goals'

const goals = useGoalStore()
const emit = defineEmits<{ close: [] }>()
const title = ref('')
const description = ref('')
const maxRuns = ref(20)
const maxSteps = ref(30)
const maxSeconds = ref(600)
const creating = ref(false)
const submitting = ref(false)
const statusLabels = { active: '进行中', paused: '已暂停', done: '已完成', blocked: '需你决定', cancelled: '已取消' }
const stepLabels = { pending: '待开始', doing: '进行中', done: '已完成', failed: '失败' }
const outcomeLabels = { ok: '成功', partial: '部分完成', failed: '失败', blocked: '需要决定' }
function time(at: number) { return new Date(at * 1000).toLocaleString('zh-CN') }
async function create() {
  submitting.value = true
  try {
    await goals.create(title.value, description.value, { max_runs: maxRuns.value, max_seconds_per_run: maxSeconds.value, max_steps_per_run: maxSteps.value })
    if (!goals.error) { title.value = ''; description.value = ''; creating.value = false }
  } finally { submitting.value = false }
}
async function toggle(goal: AgentGoal, event: Event) {
  const input = event.target as HTMLInputElement
  await goals.settings(goal, input.checked, goal.auto_interval_secs)
  input.checked = goals.goals.find(current => current.id === goal.id)?.auto_advance ?? goal.auto_advance
}
function interval(goal: AgentGoal, event: Event) {
  goals.settings(goal, goal.auto_advance, Math.round(Number((event.target as HTMLInputElement).value) * 60))
}
function budget(goal: AgentGoal, event: Event) {
  goals.settings(goal, goal.auto_advance, goal.auto_interval_secs, { ...goal.budget, max_runs: Number((event.target as HTMLInputElement).value) })
}
function stepBudget(goal: AgentGoal, event: Event) {
  goals.settings(goal, goal.auto_advance, goal.auto_interval_secs, { ...goal.budget, max_steps_per_run: Number((event.target as HTMLInputElement).value) })
}
async function remove(goal: AgentGoal) {
  if (confirm('确定删除目标「' + goal.title + '」及其全部推进记录？此操作不可恢复。')) await goals.remove(goal.id)
}
</script>

<template>
  <aside class="goals-panel" aria-label="目标面板">
    <header><h2>目标</h2><button @click="creating = !creating">新建</button><button @click="emit('close')">关闭</button></header>
    <p class="hint">自动推进默认关闭，仅在应用进程运行时推进。每次约消耗 1 次 API 调用，工具、规划与自检可能增加调用，按当前配置模型计费。退出应用即停止。</p>
    <p v-if="goals.error" class="blocked" role="alert">{{ goals.error }}</p>
    <p v-if="goals.feedback" role="status">{{ goals.feedback }}</p>
    <form v-if="creating" @submit.prevent="create">
      <label>标题<input v-model="title" required maxlength="200" /></label>
      <label>详细说明<textarea v-model="description" required rows="3" /></label>
      <label>推进次数上限<input v-model.number="maxRuns" type="number" min="1" max="4294967295" required /></label>
      <label>单次步数上限<input v-model.number="maxSteps" type="number" min="1" max="4294967295" required /></label>
      <label>单次超时（秒）<input v-model.number="maxSeconds" type="number" min="1" max="4294967295" required /></label>
      <button type="submit" :disabled="submitting">建立目标</button>
    </form>
    <p v-if="!goals.goals.length">暂无目标。新建一个目标后，可逐次推进并保留进度。</p>
    <article v-for="goal in goals.goals" :key="goal.id">
      <h3>{{ goal.title }} <span>{{ statusLabels[goal.status] }}</span></h3>
      <p>{{ goal.progress }}</p>
      <p>下一步：{{ goal.next_action || '无' }}</p>
      <p class="hint">推进 {{ goal.used.runs }}/{{ goal.budget.max_runs }} 次 · 已用 {{ goal.used.total_seconds }} 秒 · 模型调用 {{ goal.used.calls ?? 0 }} 次 · 更新 {{ time(goal.updated_at) }}</p>
      <p v-if="goal.blocked_reason" class="blocked" role="alert">需要你决定：{{ goal.blocked_reason }}</p>
      <p v-if="goals.runningId === goal.id" role="status">{{ goals.automatic ? '正在自动推进' : '正在推进' }}：{{ goal.title }}</p>
      <label class="auto-toggle"><input type="checkbox" :checked="goal.auto_advance" :disabled="goal.status !== 'active'" @change="toggle(goal, $event)" />自动推进（每 {{ goal.auto_interval_secs / 60 }} 分钟一次）</label>
      <label>间隔（分钟）<input type="number" :value="goal.auto_interval_secs / 60" min="0.0166666667" step="any" @change="interval(goal, $event)" /></label>
      <label>推进次数上限<input type="number" :value="goal.budget.max_runs" min="1" step="1" @change="budget(goal, $event)" /></label>
      <label>单次步数上限<input type="number" :value="goal.budget.max_steps_per_run ?? 30" min="1" max="4294967295" step="1" @change="stepBudget(goal, $event)" /></label>
      <p v-if="goal.auto_advance" class="auto-state">自动推进中 · 每 {{ goal.auto_interval_secs / 60 }} 分钟 · 已自动推进 {{ goal.auto_runs }} 次</p>
      <div class="actions">
        <button :disabled="goal.status !== 'active' || !!goals.runningId" @click="goals.advance(goal.id)">推进一次</button>
        <button :disabled="goal.status !== 'active'" @click="goals.setStatus(goal.id, 'paused')">暂停</button>
        <button :disabled="!['paused', 'blocked'].includes(goal.status) || goals.runningId === goal.id" @click="goals.setStatus(goal.id, 'active')">继续</button>
        <button :disabled="['done', 'cancelled'].includes(goal.status)" @click="goals.setStatus(goal.id, 'cancelled')">取消</button>
        <button :disabled="goals.runningId === goal.id" @click="remove(goal)">删除</button>
      </div>
      <details><summary>步骤与推进记录</summary>
        <p>{{ goal.description }}</p>
        <ol><li v-for="(step, index) in goal.steps" :key="index">{{ stepLabels[step.status] }}：{{ step.text }}</li></ol>
        <ul><li v-for="(checkpoint, index) in goal.checkpoints" :key="index">{{ time(checkpoint.at) }} · {{ outcomeLabels[checkpoint.outcome] }} · {{ checkpoint.summary }}</li></ul>
      </details>
    </article>
  </aside>
</template>

<style scoped>
.goals-panel { position: fixed; right: 16px; top: 64px; bottom: 24px; width: min(480px, calc(100vw - 32px)); z-index: 100; overflow-y: auto; padding: 20px; box-sizing: border-box; background: var(--bg-main); color: var(--text-main); border: 1px solid var(--border, #8886); border-radius: var(--radius-ui, 12px); box-shadow: 0 8px 30px #0004; }
header, .actions { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
h2 { margin: 0 auto 0 0; }
h3 { font-size: 16px; overflow-wrap: anywhere; }
h3 span, .hint { color: var(--text-secondary); font-size: 12px; }
article, form { border-top: 1px solid var(--border, #8886); margin-top: 16px; padding-top: 12px; }
p, li { white-space: pre-wrap; overflow-wrap: anywhere; }
label { display: flex; gap: 8px; align-items: center; margin: 8px 0; }
input:not([type=checkbox]), textarea { min-width: 0; flex: 1; padding: 6px; background: var(--bg-main); color: var(--text-main); border: 1px solid var(--border, #8886); border-radius: 6px; }
.auto-toggle, .auto-state { color: var(--accent); }
.blocked { border-left: 3px solid var(--danger, #df5555); padding: 8px; color: var(--danger, #df5555); font-weight: 600; }
button { cursor: pointer; border: 1px solid var(--border, #8886); border-radius: 6px; background: transparent; color: var(--text-main); padding: 6px 10px; }
button:disabled { opacity: 0.45; cursor: not-allowed; }
details { margin-top: 12px; }
summary { cursor: pointer; }
</style>
