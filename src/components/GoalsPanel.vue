<script setup lang="ts">
// 目标面板（抽屉内流式布局）：标题与关闭由抽屉标签条负责，这里只留「新建」和内容
import { ref } from 'vue'
import { useGoalStore } from '../stores/goalStore'
import type { AgentGoal } from '../types/goals'
import TheIcon from './TheIcon.vue'
import AppButton from './ui/AppButton.vue'
import AppSwitch from './ui/AppSwitch.vue'
import AppStatusDot from './ui/AppStatusDot.vue'
import AppEmpty from './ui/AppEmpty.vue'
import { confirmDialog } from '../utils/dialog'

// 关闭由标签条处理；仍声明 close，避免父组件的监听落到根元素上
defineEmits<{ close: [] }>()

const goals = useGoalStore()
const title = ref('')
const description = ref('')
const maxRuns = ref(20)
const maxSteps = ref(30)
const maxSeconds = ref(600)
const creating = ref(false)
const submitting = ref(false)
const statusLabels: Record<string, string> = { active: '进行中', paused: '已暂停', done: '已完成', blocked: '需你决定', cancelled: '已取消' }
const statusStates: Record<string, 'ok' | 'warn' | 'err' | 'idle'> = { active: 'ok', paused: 'warn', done: 'ok', blocked: 'err', cancelled: 'idle' }
const stepLabels: Record<string, string> = { pending: '待开始', doing: '进行中', done: '已完成', failed: '失败' }
const outcomeLabels: Record<string, string> = { ok: '成功', partial: '部分完成', failed: '失败', blocked: '需要决定' }
function time(at: number) { return new Date(at * 1000).toLocaleString('zh-CN') }
function autoLabel(goal: AgentGoal) { return `自动推进（每 ${goal.auto_interval_secs / 60} 分钟一次）` }
async function create() {
  submitting.value = true
  try {
    await goals.create(title.value, description.value, { max_runs: maxRuns.value, max_seconds_per_run: maxSeconds.value, max_steps_per_run: maxSteps.value })
    if (!goals.error) { title.value = ''; description.value = ''; creating.value = false }
  } finally { submitting.value = false }
}
/** 自动推进开关：开关自身受 store 数据控制，失败会自动回到原状态 */
async function setAuto(goal: AgentGoal, value: boolean) {
  await goals.settings(goal, value, goal.auto_interval_secs)
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
  const ok = await confirmDialog(`确定删除目标「${goal.title}」及其全部推进记录？此操作不可恢复。`, {
    danger: true,
    confirmText: '删除',
  })
  if (ok) await goals.remove(goal.id)
}
</script>

<template>
  <div class="goals-panel" role="region" aria-label="目标">
    <!-- 轻量操作条：只剩功能按钮，标题与关闭已在抽屉标签条上 -->
    <div class="panel-bar">
      <AppButton size="sm" @click="creating = !creating">
        <TheIcon name="add" :size="14" />{{ creating ? '收起新建' : '新建目标' }}
      </AppButton>
      <span class="bar-meta">{{ goals.goals.length }} 个目标</span>
    </div>

    <div class="panel-body">
      <p class="hint">自动推进默认关闭，仅在应用进程运行时推进。每次约消耗 1 次 API 调用，工具、规划与自检可能增加调用，按当前配置模型计费。退出应用即停止。</p>

      <p v-if="goals.error" class="alert err" role="alert">
        <AppStatusDot state="err" text="出错" />
        <span>{{ goals.error }}</span>
      </p>
      <p v-if="goals.feedback" class="feedback" role="status">{{ goals.feedback }}</p>

      <!-- 新建表单 -->
      <form v-if="creating" class="create-form" @submit.prevent="create">
        <label class="field"><span>标题</span><input v-model="title" required maxlength="200" /></label>
        <label class="field"><span>详细说明</span><textarea v-model="description" required rows="3" /></label>
        <div class="field-row">
          <label class="field"><span>推进次数上限</span><input v-model.number="maxRuns" type="number" min="1" max="4294967295" required /></label>
          <label class="field"><span>单次步数上限</span><input v-model.number="maxSteps" type="number" min="1" max="4294967295" required /></label>
          <label class="field"><span>单次超时（秒）</span><input v-model.number="maxSeconds" type="number" min="1" max="4294967295" required /></label>
        </div>
        <div class="form-actions">
          <AppButton type="submit" variant="primary" size="sm" :disabled="submitting">建立目标</AppButton>
          <AppButton variant="text" size="sm" @click="creating = false">取消</AppButton>
        </div>
      </form>

      <AppEmpty
        v-if="!goals.goals.length && !creating"
        text="暂无目标"
        hint="新建一个目标后，可逐次推进并保留进度。"
        action-label="新建目标"
        @action="creating = true"
      />

      <!-- 目标卡片 -->
      <article v-for="goal in goals.goals" :key="goal.id" class="goal-card">
        <header class="goal-head">
          <h3 class="goal-title">{{ goal.title }}</h3>
          <AppStatusDot :state="statusStates[goal.status] || 'idle'" :text="statusLabels[goal.status]" />
        </header>
        <p class="goal-text">{{ goal.progress }}</p>
        <p class="goal-text">下一步：{{ goal.next_action || '无' }}</p>
        <p class="meta">推进 {{ goal.used.runs }}/{{ goal.budget.max_runs }} 次 · 已用 {{ goal.used.total_seconds }} 秒 · 模型调用 {{ goal.used.calls ?? 0 }} 次 · 更新 {{ time(goal.updated_at) }}</p>
        <p v-if="goal.blocked_reason" class="alert warn" role="alert">
          <AppStatusDot state="warn" text="需要你决定" />
          <span>{{ goal.blocked_reason }}</span>
        </p>
        <p v-if="goals.runningId === goal.id" class="running" role="status">
          <span class="run-dot" aria-hidden="true" />
          <span>{{ goals.automatic ? '正在自动推进' : '正在推进' }}：{{ goal.title }}</span>
        </p>

        <AppSwitch
          :model-value="goal.auto_advance"
          :label="autoLabel(goal)"
          :disabled="goal.status !== 'active'"
          @update:modelValue="setAuto(goal, $event)"
        />
        <div class="field-row">
          <label class="field"><span>间隔（分钟）</span><input type="number" :value="goal.auto_interval_secs / 60" min="0.0166666667" step="any" @change="interval(goal, $event)" /></label>
          <label class="field"><span>推进次数上限</span><input type="number" :value="goal.budget.max_runs" min="1" step="1" @change="budget(goal, $event)" /></label>
          <label class="field"><span>单次步数上限</span><input type="number" :value="goal.budget.max_steps_per_run ?? 30" min="1" max="4294967295" step="1" @change="stepBudget(goal, $event)" /></label>
        </div>
        <p v-if="goal.auto_advance" class="auto-state">自动推进中 · 每 {{ goal.auto_interval_secs / 60 }} 分钟 · 已自动推进 {{ goal.auto_runs }} 次</p>

        <div class="actions">
          <AppButton size="sm" :disabled="goal.status !== 'active' || !!goals.runningId" @click="goals.advance(goal.id)">推进一次</AppButton>
          <AppButton size="sm" :disabled="goal.status !== 'active'" @click="goals.setStatus(goal.id, 'paused')">暂停</AppButton>
          <AppButton size="sm" :disabled="!['paused', 'blocked'].includes(goal.status) || goals.runningId === goal.id" @click="goals.setStatus(goal.id, 'active')">继续</AppButton>
          <AppButton size="sm" :disabled="['done', 'cancelled'].includes(goal.status)" @click="goals.setStatus(goal.id, 'cancelled')">取消</AppButton>
          <AppButton size="sm" variant="text" :disabled="goals.runningId === goal.id" @click="remove(goal)">删除</AppButton>
        </div>

        <details class="goal-details">
          <summary>步骤与推进记录</summary>
          <p class="goal-text">{{ goal.description }}</p>
          <ol>
            <li v-for="(step, index) in goal.steps" :key="index">{{ stepLabels[step.status] }}：{{ step.text }}</li>
          </ol>
          <ul>
            <li v-for="(checkpoint, index) in goal.checkpoints" :key="index">{{ time(checkpoint.at) }} · {{ outcomeLabels[checkpoint.outcome] }} · {{ checkpoint.summary }}</li>
          </ul>
        </details>
      </article>
    </div>
  </div>
</template>

<style scoped>
/* 流式布局：占满抽屉内容区，滚动交给内容区自己 */
.goals-panel {
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
  gap: var(--ui-sp-3);
  padding: var(--ui-sp-3);
}

.hint,
.meta {
  margin: 0;
  font-size: var(--ui-fs-12);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
}
.meta {
  font-size: var(--ui-fs-11);
}

/* 提示条 */
.alert {
  display: flex;
  align-items: flex-start;
  gap: var(--ui-sp-2);
  margin: 0;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-radius: var(--ui-r-sm);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  overflow-wrap: anywhere;
}
.alert.err {
  background: color-mix(in srgb, var(--ui-err) 10%, transparent);
  border-left: 3px solid var(--ui-err);
  color: var(--ui-err);
}
.alert.warn {
  background: color-mix(in srgb, var(--ui-warn) 10%, transparent);
  border-left: 3px solid var(--ui-warn);
  color: var(--ui-warn);
}
.feedback {
  margin: 0;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-radius: var(--ui-r-sm);
  background: var(--ui-active);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text-2);
  overflow-wrap: anywhere;
}

/* 表单与卡片共用外壳 */
.create-form,
.goal-card {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-3);
  background: var(--ui-raised);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-md);
}

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
.field textarea:focus {
  border-color: var(--ui-accent);
}
.field-row {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(92px, 1fr));
  gap: var(--ui-sp-2);
}
.form-actions {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
}

.goal-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--ui-sp-2);
}
.goal-title {
  flex: 1;
  min-width: 0;
  margin: 0;
  font-size: var(--ui-fs-14);
  font-weight: 600;
  line-height: var(--ui-line);
  overflow-wrap: anywhere;
}
.goal-text {
  margin: 0;
  font-size: var(--ui-fs-13);
  line-height: var(--ui-line);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

/* 进行中：小圆环转圈，替代原来的文字符号 */
.running {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  margin: 0;
  font-size: var(--ui-fs-12);
  color: var(--ui-accent);
}
.run-dot {
  flex: none;
  width: 10px;
  height: 10px;
  border-radius: var(--ui-r-pill);
  border: 2px solid var(--ui-accent-soft);
  border-top-color: var(--ui-accent);
  animation: goal-spin 700ms linear infinite;
}
@keyframes goal-spin {
  to { transform: rotate(360deg); }
}
.auto-state {
  margin: 0;
  font-size: var(--ui-fs-12);
  color: var(--ui-accent);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
}

.goal-details {
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
}
.goal-details summary {
  cursor: pointer;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-12);
}
.goal-details ol,
.goal-details ul {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
  margin: var(--ui-sp-2) 0 0;
  padding-left: var(--ui-sp-4);
}
.goal-details li {
  line-height: var(--ui-line);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>
