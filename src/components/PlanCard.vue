<!-- 四、先规划后执行：计划确认卡（AppCard 重画）
     后端拆出多步计划后推上来，这里先给主人看、可改/可删/可加，确认后才真正开跑。
     不操作的话倒计时结束自动按当前步骤继续（后端另有 120 秒兜底，两边都不会把任务卡死）。 -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useChatStore } from '../stores/chatStore'
import { agentPlanRespond } from '../utils/tauri'
import AppCard from './ui/AppCard.vue'
import AppButton from './ui/AppButton.vue'
import AppInput from './ui/AppInput.vue'
import TheIcon from './TheIcon.vue'

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
  <AppCard
    v-if="plan"
    class="plan"
    role="region"
    aria-label="执行计划确认"
    title="铃打算这么做"
    :hint="remain + ' 秒后自动开始'"
  >
    <p class="hint">步骤可以改、可以删、可以加；确认后铃就按这个执行。</p>

    <div class="steps">
      <div v-for="(_step, index) in goals" :key="index" class="step">
        <span class="no">{{ index + 1 }}</span>
        <AppInput
          v-model="goals[index]"
          class="step-input"
          :aria-label="'第 ' + (index + 1) + ' 步'"
          placeholder="这一步要做什么"
          :disabled="busy"
        />
        <AppButton
          variant="text"
          size="sm"
          class="del"
          title="删除这一步"
          aria-label="删除这一步"
          :disabled="busy"
          @click="removeStep(index)"
        >
          <TheIcon name="close" :size="12" />
        </AppButton>
      </div>
    </div>

    <AppButton variant="text" size="sm" class="add" :disabled="busy" @click="addStep">
      <TheIcon name="add" :size="12" />
      <span>加一步</span>
    </AppButton>

    <p v-if="notice" class="notice">{{ notice }}</p>

    <div class="actions">
      <AppButton variant="ghost" :disabled="busy" @click="settle(false)">取消任务</AppButton>
      <AppButton variant="primary" :disabled="busy || !canConfirm" @click="settle(true)">按计划执行</AppButton>
    </div>
  </AppCard>
</template>

<style scoped>
.plan {
  margin-top: var(--ui-sp-3);
}
.hint {
  margin: 0 0 var(--ui-sp-3);
  font-size: var(--ui-fs-12);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
}
.steps {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-2);
}
.step {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  min-width: 0;
}
.no {
  flex: none;
  width: 20px;
  height: 20px;
  display: grid;
  place-items: center;
  border-radius: var(--ui-r-pill);
  background: var(--ui-accent-soft);
  color: var(--ui-accent);
  font-size: var(--ui-fs-11);
  line-height: 1;
}
/* AppInput 根元素带本组件作用域，直接吃掉剩余宽度 */
.step .step-input {
  flex: 1;
  min-width: 0;
}
.del,
.add {
  flex: none;
}
.add {
  margin-top: var(--ui-sp-2);
}
.notice {
  margin: var(--ui-sp-2) 0 0;
  font-size: var(--ui-fs-12);
  color: var(--ui-warn);
  line-height: var(--ui-line);
}
.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--ui-sp-2);
  margin-top: var(--ui-sp-3);
}
</style>
