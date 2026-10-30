<!-- 《铃·记忆体》屏幕监测设置（AI-6 任务 5 / 4.3，挂在设置页「监测」分区）
     总开关 / 监测频率 / 规则列表（应用名 · 触发回复 · 冷却 · 启用 · 编辑 · 删除）/ 添加规则 / 导入模板
     样式全部走 src/styles/tokens.css 的 --ui-* 令牌；图标用 TheIcon、状态用 AppStatusDot，无 emoji。 -->
<script setup lang="ts">
import { onMounted, ref } from 'vue'
import TheIcon from './TheIcon.vue'
import AppButton from './ui/AppButton.vue'
import AppCard from './ui/AppCard.vue'
import AppEmpty from './ui/AppEmpty.vue'
import AppField from './ui/AppField.vue'
import AppStatusDot from './ui/AppStatusDot.vue'
import AppSwitch from './ui/AppSwitch.vue'
import { confirmDialog } from '../utils/dialog'
import { toastErr, toastOk } from '../utils/toast'
import { useDesktopStore } from '../stores/desktopStore'
import type { ScreenMonitorRule } from '../types'

const desktop = useDesktopStore()

const INTERVALS = [1, 3, 5, 10, 30]
const errorMsg = ref('')

// —— 规则编辑弹窗 ——
const showEditor = ref(false)
const editingRule = ref<ScreenMonitorRule>({
  id: '',
  app_name: '',
  trigger_reply: '',
  enabled: true,
  cooldown_seconds: 30,
})
const editorError = ref('')

onMounted(() => {
  desktop.loadMonitorRules()
})

async function onToggleEnabled(enabled: boolean) {
  const ok = await desktop.toggleMonitoring(enabled)
  if (!ok) {
    errorMsg.value = '屏幕监测不可用，请检查系统权限（已自动禁用）'
    setTimeout(() => (errorMsg.value = ''), 4000)
  }
}

async function onIntervalChange(e: Event) {
  const v = Number((e.target as HTMLSelectElement).value)
  await desktop.toggleMonitoring(desktop.isMonitoring, v)
}

// —— 添加/编辑规则 ——
function openAdd() {
  editingRule.value = {
    id: `rule_${Date.now()}`,
    app_name: '',
    trigger_reply: '',
    enabled: true,
    cooldown_seconds: 30,
  }
  editorError.value = ''
  showEditor.value = true
}

function openEdit(rule: ScreenMonitorRule) {
  editingRule.value = { ...rule }
  editorError.value = ''
  showEditor.value = true
}

async function saveRule() {
  const r = editingRule.value
  if (!r.app_name.trim()) {
    editorError.value = '请输入应用名（支持通配符 *）'
    return
  }
  if (!r.trigger_reply.trim()) {
    editorError.value = '请输入触发回复内容'
    return
  }
  try {
    await desktop.updateMonitorRule({ ...r, cooldown_seconds: Math.max(0, r.cooldown_seconds || 0) })
    showEditor.value = false
    toastOk('规则已保存')
  } catch (e) {
    editorError.value = String(e)
  }
}

/** 单条规则的启停（写入仍走 updateMonitorRule，失败给提示，不静默吞掉） */
async function toggleRule(rule: ScreenMonitorRule, enabled: boolean) {
  try {
    await desktop.updateMonitorRule({ ...rule, enabled })
  } catch (e) {
    toastErr(`保存失败：${e}`)
  }
}

async function removeRule(rule: ScreenMonitorRule) {
  const ok = await confirmDialog(`确定删除规则「${rule.app_name}」吗？删除后无法恢复。`, {
    title: '删除规则',
    confirmText: '删除',
    danger: true,
  })
  if (!ok) return
  try {
    await desktop.removeMonitorRule(rule.id)
    toastOk('规则已删除')
  } catch (e) {
    toastErr(`删除失败：${e}`)
  }
}

// —— 导入模板 ——
const TEMPLATES: Record<string, ScreenMonitorRule[]> = {
  游戏模式: [
    { id: `rule_${Date.now()}_game1`, app_name: 'minecraft*', trigger_reply: '主人开始挖方块了呀，注意别摔死喵~', enabled: true, cooldown_seconds: 300 },
    { id: `rule_${Date.now()}_game2`, app_name: 'steam*', trigger_reply: '要玩游戏了吗？铃在旁边给你加油！', enabled: true, cooldown_seconds: 300 },
  ],
  工作模式: [
    { id: `rule_${Date.now()}_work1`, app_name: 'code*', trigger_reply: '主人专心写代码的样子真帅，累了记得休息一下~', enabled: true, cooldown_seconds: 600 },
    { id: `rule_${Date.now()}_work2`, app_name: 'chrome.exe', trigger_reply: '又在查资料？需要铃帮忙整理吗？', enabled: true, cooldown_seconds: 600 },
  ],
}

async function importTemplate(name: string) {
  const rules = TEMPLATES[name]
  try {
    await Promise.all(rules.map((r) => desktop.updateMonitorRule(r)))
    toastOk(`已导入「${name}」模板：${rules.length} 条规则`)
  } catch (e) {
    toastErr(`导入失败：${e}`)
  }
}
</script>

<template>
  <div class="page">
    <!-- 总开关 + 频率 -->
    <AppCard title="屏幕监测">
      <AppSwitch
        :model-value="desktop.isMonitoring"
        label="开启监测"
        hint="关闭后铃不再主动观察窗口，已配置的规则会保留"
        :disabled="!desktop.monitoringAvailable"
        @update:model-value="onToggleEnabled"
      />
      <AppField label="监测频率" hint="间隔越短越及时，但更耗电也更占 CPU">
        <select class="select" :value="desktop.monitorInterval" @change="onIntervalChange">
          <option v-for="iv in INTERVALS" :key="iv" :value="iv">{{ iv }} 秒</option>
        </select>
      </AppField>

      <p v-if="!desktop.monitoringAvailable" class="notice">
        <AppStatusDot state="warn" />
        <span class="notice-text">屏幕监测不可用，请检查系统权限；仍可手动配置规则作为备选</span>
      </p>
      <p v-if="errorMsg" class="notice err">
        <AppStatusDot state="err" />
        <span class="notice-text">{{ errorMsg }}</span>
      </p>
    </AppCard>

    <!-- 规则列表 -->
    <AppCard title="监测规则" :hint="`共 ${desktop.monitorRules.length} 条`">
      <template #actions>
        <AppButton variant="primary" size="sm" @click="openAdd">
          <TheIcon name="add" :size="14" />
          添加规则
        </AppButton>
      </template>

      <div class="toolbar">
        <AppButton size="sm" @click="importTemplate('游戏模式')">导入：游戏模式</AppButton>
        <AppButton size="sm" @click="importTemplate('工作模式')">导入：工作模式</AppButton>
      </div>

      <AppEmpty
        v-if="desktop.monitorRules.length === 0 && !desktop.monitoringLoading"
        text="还没有规则"
        hint="点右上角「添加规则」配一条，或直接导入现成的模板。"
        action-label="添加规则"
        @action="openAdd"
      />

      <ul v-else class="rule-list">
        <li v-for="rule in desktop.monitorRules" :key="rule.id" class="rule" :class="{ off: !rule.enabled }">
          <div class="rule-head">
            <code class="app-name">{{ rule.app_name }}</code>
            <span class="rule-meta">冷却 {{ rule.cooldown_seconds }} 秒</span>
            <span class="spacer" />
            <AppSwitch
              class="rule-switch"
              :model-value="rule.enabled"
              label="启用"
              @update:model-value="(v) => toggleRule(rule, v)"
            />
            <button type="button" class="icon-btn" title="编辑规则" aria-label="编辑规则" @click="openEdit(rule)">
              <TheIcon name="edit" :size="14" />
            </button>
            <button type="button" class="icon-btn" title="删除规则" aria-label="删除规则" @click="removeRule(rule)">
              <TheIcon name="delete" :size="14" />
            </button>
          </div>
          <p class="reply">{{ rule.trigger_reply }}</p>
        </li>
      </ul>
    </AppCard>

    <!-- 规则编辑弹窗 -->
    <div v-if="showEditor" class="modal-mask" @click.self="showEditor = false">
      <div class="modal" role="dialog" aria-modal="true" aria-label="规则编辑">
        <div class="modal-title">
          {{ editingRule.app_name && editingRule.trigger_reply ? '编辑规则' : '添加规则' }}
        </div>
        <AppField label="应用名" hint="支持通配符，如 chrome* / notepad.exe">
          <AppInput v-model="editingRule.app_name" placeholder="chrome*" />
        </AppField>
        <AppField label="触发回复" hint="铃会说的话，按你平时跟她说话的口吻写就好">
          <textarea v-model="editingRule.trigger_reply" rows="2" class="input textarea" placeholder="铃会说的话…" />
        </AppField>
        <AppField label="冷却时间（秒）" hint="同一条规则两次触发之间的最短间隔">
          <input v-model.number="editingRule.cooldown_seconds" type="number" min="0" class="input" />
        </AppField>
        <p v-if="editorError" class="notice err">
          <AppStatusDot state="err" />
          <span class="notice-text">{{ editorError }}</span>
        </p>
        <div class="modal-actions">
          <AppButton @click="showEditor = false">取消</AppButton>
          <AppButton variant="primary" @click="saveRule">保存</AppButton>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.page {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-3);
  padding-bottom: var(--ui-sp-4);
}

/* ---------- 频率选择 ---------- */
.select {
  width: auto;
  max-width: 100%;
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-2);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-sm);
  background: var(--ui-raised);
  color: var(--ui-text);
  font-size: var(--ui-fs-13);
}
.select:focus-visible {
  border-color: var(--ui-accent);
}

/* ---------- 提示条 ---------- */
.notice {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  margin: var(--ui-sp-2) 0 0;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-radius: var(--ui-r-sm);
  background: color-mix(in srgb, var(--ui-warn) 12%, transparent);
  color: var(--ui-text);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
}
.notice.err {
  background: color-mix(in srgb, var(--ui-err) 12%, transparent);
}
.notice-text {
  min-width: 0;
  overflow-wrap: anywhere;
}

/* ---------- 规则列表 ---------- */
.toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  margin-bottom: var(--ui-sp-2);
}
.rule-list {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-2);
  margin: 0;
  padding: 0;
  list-style: none;
}
.rule {
  min-width: 0;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  background: var(--ui-panel);
}
.rule.off {
  opacity: 0.62;
}
.rule-head {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  min-width: 0;
}
.app-name {
  min-width: 0;
  padding: 1px var(--ui-sp-2);
  border-radius: var(--ui-r-sm);
  background: var(--ui-hover);
  color: var(--ui-text);
  font-family: var(--font-mono);
  font-size: var(--ui-fs-12);
  overflow-wrap: anywhere;
}
.rule-meta {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
}
.spacer {
  flex: 1 1 0;
  min-width: 0;
}
.rule-switch {
  flex: none;
}
.reply {
  margin: var(--ui-sp-1) 0 0;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-13);
  line-height: var(--ui-line);
  overflow-wrap: anywhere;
}
.icon-btn {
  display: inline-grid;
  place-items: center;
  flex: none;
  width: 26px;
  height: 26px;
  border: 1px solid transparent;
  border-radius: var(--ui-r-sm);
  background: transparent;
  color: var(--ui-text-2);
}
.icon-btn:hover {
  border-color: var(--ui-border);
  background: var(--ui-hover);
  color: var(--ui-text);
}

/* ---------- 编辑弹窗 ---------- */
.modal-mask {
  position: fixed;
  inset: 0;
  z-index: 600;
  display: grid;
  place-items: center;
  padding: var(--ui-sp-4);
  background: color-mix(in srgb, var(--ui-bg) 62%, transparent);
  backdrop-filter: blur(3px);
}
.modal {
  width: min(420px, 100%);
  max-height: 88vh;
  overflow-y: auto;
  padding: var(--ui-sp-4);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-lg);
  background: var(--ui-raised);
  box-shadow: var(--ui-shadow);
}
.modal-title {
  margin-bottom: var(--ui-sp-2);
  color: var(--ui-text);
  font-size: var(--ui-fs-14);
  font-weight: 600;
}
.input {
  width: 100%;
  min-width: 0;
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-3);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-sm);
  background: var(--ui-raised);
  color: var(--ui-text);
  font-size: var(--ui-fs-13);
}
.input:focus {
  border-color: var(--ui-accent);
}
.textarea {
  height: auto;
  min-height: 60px;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  line-height: var(--ui-line);
  resize: vertical;
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--ui-sp-2);
  margin-top: var(--ui-sp-3);
}
</style>
