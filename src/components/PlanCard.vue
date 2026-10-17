<!-- 4️⃣ 先规划后执行：计划确认卡
     后端拆出多步计划后推上来，这里先给主人看、可改/可删/可加，确认后才真正开跑。
     不操作的话倒计时结束自动按当前步骤继续（后端另有 120 秒兜底，两边都不会把任务卡死）。 -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useChatStore } from '../stores/chatStore'
import { agentPlanRespond } from '../utils/tauri'

const chat = useChatStore()
// 编辑副本：不直接改 store 里的计划，取消时原计划仍完整
const goals = ref<string[]>([])
const remain = ref(0)
const busy = ref(false)
const notice = ref('')
let timer: number | undefined
// 倒计时只在真正挂载后启动（SSR / 单测环境不乱起定时器）
let mounted = false

const plan = computed(() => chat.pendingPlan)

watch(plan, (p) => {
  if (!p) {
    stopTimer()
    return
  }
  goals.value = p.steps.map((s) => s.goal)
  remain.value = p.wait_seconds
  notice.value = ''
  if (mounted) startTimer()
}, { immediate: true })

onMounted(() => {
  mounted = true
  if (plan.value) startTimer()
})

function startTimer() {
  stopTimer()
  timer = window.setInterval(() => {
    remain.value = Math.max(0, remain.value - 1)
    if (remain.value <= 0) {
      stopTimer()
      void settle(true, '计划已经超时，铃先按原计划开始了。')
    }
  }, 1000)
}

function stopTimer() {
  if (timer) {
    clearInterval(timer)
    timer = undefined
  }
}

function removeStep(index: number) {
  goals.value.splice(index, 1)
}

function addStep() {
  goals.value.push('')
}

const canConfirm = computed(() => goals.value.some((goal) => goal.trim()))

async function settle(approve: boolean, fallbackNotice = '') {
  const current = plan.value
  if (!current || busy.value) return
  busy.value = true
  stopTimer()
  const steps = goals.value.map((goal) => goal.trim()).filter(Boolean)
  try {
    const ok = await agentPlanRespond(current.plan_id, approve, approve ? steps : undefined)
    if (!ok) {
      // 后端已经超时/取消了这条计划，告诉主人真实情况，不假装还来得及
      notice.value = fallbackNotice || '这个计划已经超时，铃按原计划继续了。'
    }
  } catch {
    notice.value = '回应没发出去，铃会按原计划继续。'
  } finally {
    busy.value = false
    chat.clearPlan()
  }
}

onUnmounted(stopTimer)
</script>

<template>
  <div v-if="plan" class="plan-card" role="region" aria-label="执行计划确认">
    <div class="plan-head">
      <span class="plan-title">铃打算这么做</span>
      <span class="plan-countdown">{{ remain }} 秒后自动开始</span>
    </div>
    <p class="plan-hint">步骤可以改、可以删、可以加；确认后铃就按这个执行。</p>
    <div class="plan-steps">
      <div v-for="(_step, index) in goals" :key="index" class="plan-step">
        <span class="step-no">{{ index + 1 }}</span>
        <input
          v-model="goals[index]"
          class="step-input"
          :aria-label="'第 ' + (index + 1) + ' 步'"
          placeholder="这一步要做什么"
          :disabled="busy"
        />
        <button class="step-del" title="删除这一步" :disabled="busy" @click="removeStep(index)">×</button>
      </div>
    </div>
    <button class="add-step" :disabled="busy" @click="addStep">＋ 加一步</button>
    <p v-if="notice" class="plan-notice">{{ notice }}</p>
    <div class="plan-actions">
      <button class="btn-ghost" :disabled="busy" @click="settle(false)">取消任务</button>
      <button class="btn-primary" :disabled="busy || !canConfirm" @click="settle(true)">按计划执行</button>
    </div>
  </div>
</template>

<style scoped>
.plan-card {
  margin: 8px 16px 4px;
  padding: 12px 14px;
  border: 1px solid var(--border, rgba(128, 128, 128, 0.35));
  border-left: 3px solid var(--accent, #ff7a94);
  border-radius: var(--radius-ui, 12px);
  background: var(--bg-bar, rgba(34, 32, 36, 0.9));
  color: var(--text-main, #eee6e7);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
}
.plan-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.plan-title {
  font-size: var(--fs-14, 14px);
  font-weight: 600;
}
.plan-countdown {
  font-size: var(--fs-11, 11px);
  color: var(--text-secondary, #9a9294);
  white-space: nowrap;
}
.plan-hint {
  margin: 6px 0 8px;
  font-size: var(--fs-12, 12px);
  color: var(--text-secondary, #9a9294);
}
.plan-steps {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.plan-step {
  display: flex;
  align-items: center;
  gap: 8px;
}
.step-no {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--fs-11, 11px);
  background: color-mix(in srgb, var(--accent, #ff7a94) 25%, transparent);
  color: var(--text-main, #eee6e7);
}
.step-input {
  flex: 1;
  min-width: 0;
  padding: 6px 8px;
  border-radius: calc(var(--radius-ui, 12px) - 6px);
  border: 1px solid var(--border, rgba(128, 128, 128, 0.35));
  background: var(--input-bg, #2a272b);
  color: var(--text-main, #eee6e7);
  font-size: var(--fs-13, 13px);
  font-family: inherit;
  outline: none;
}
.step-input:focus {
  border-color: var(--accent, #ff7a94);
}
.step-del,
.add-step {
  border: none;
  background: transparent;
  color: var(--text-secondary, #9a9294);
  cursor: pointer;
  font-size: var(--fs-13, 13px);
  flex-shrink: 0;
}
.step-del:hover {
  color: var(--danger, #ff6b6b);
}
.add-step {
  margin-top: 8px;
  padding: 2px 0;
}
.add-step:hover {
  color: var(--accent, #ff7a94);
}
.plan-notice {
  margin: 8px 0 0;
  font-size: var(--fs-12, 12px);
  color: var(--text-secondary, #9a9294);
}
.plan-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 10px;
}
.btn-ghost,
.btn-primary {
  padding: 6px 12px;
  border-radius: calc(var(--radius-ui, 12px) - 6px);
  font-size: var(--fs-12, 12px);
  cursor: pointer;
  font-family: inherit;
}
.btn-ghost {
  border: 1px solid var(--border, rgba(128, 128, 128, 0.35));
  background: transparent;
  color: var(--text-secondary, #9a9294);
}
.btn-primary {
  border: none;
  background: var(--accent, #ff7a94);
  color: #fff;
  font-weight: 600;
}
.btn-ghost:disabled,
.btn-primary:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
</style>
