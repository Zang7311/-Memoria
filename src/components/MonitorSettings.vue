<!-- 《铃·记忆体》屏幕监测设置（AI-6 任务 5 / 4.3，挂在设置页）
     总开关 / 监测频率 / 规则表格（应用名|触发回复|冷却|启用|编辑|删除）/ 添加规则 / 导入模板 -->
<script setup lang="ts">
import { onMounted, ref } from 'vue'
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

async function onToggleEnabled() {
  const ok = await desktop.toggleMonitoring(!desktop.isMonitoring)
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
  } catch (e) {
    editorError.value = String(e)
  }
}

async function removeRule(rule: ScreenMonitorRule) {
  await desktop.removeMonitorRule(rule.id)
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

function importTemplate(name: string) {
  const rules = TEMPLATES[name]
  rules.forEach((r) => desktop.updateMonitorRule(r))
}
</script>

<template>
  <div class="monitor-settings">
    <div class="section-title">👁️ 屏幕监测</div>

    <!-- 总开关 + 频率 -->
    <div class="row">
      <label class="switch-wrap">
        <span class="label">开启监测</span>
        <input
          type="checkbox"
          class="switch"
          :checked="desktop.isMonitoring"
          :disabled="!desktop.monitoringAvailable"
          @change="onToggleEnabled"
        />
      </label>
      <label class="interval-wrap">
        <span class="label">监测频率</span>
        <select :value="desktop.monitorInterval" @change="onIntervalChange">
          <option v-for="iv in INTERVALS" :key="iv" :value="iv">{{ iv }} 秒</option>
        </select>
      </label>
    </div>

    <div v-if="!desktop.monitoringAvailable" class="unavailable">
      ⚠️ 屏幕监测不可用，请检查系统权限；仍可手动配置规则作为备选
    </div>
    <div v-if="errorMsg" class="unavailable">{{ errorMsg }}</div>

    <!-- 操作按钮 -->
    <div class="actions">
      <button class="btn primary" @click="openAdd">＋ 添加规则</button>
      <button class="btn ghost" @click="importTemplate('游戏模式')">导入：游戏模式</button>
      <button class="btn ghost" @click="importTemplate('工作模式')">导入：工作模式</button>
    </div>

    <!-- 规则表格 -->
    <table class="rule-table">
      <thead>
        <tr>
          <th>应用名</th>
          <th>触发回复</th>
          <th>冷却(秒)</th>
          <th>启用</th>
          <th>操作</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="rule in desktop.monitorRules" :key="rule.id">
          <td><code class="app-name">{{ rule.app_name }}</code></td>
          <td class="reply-cell">{{ rule.trigger_reply }}</td>
          <td>{{ rule.cooldown_seconds }}</td>
          <td>
            <input
              type="checkbox"
              :checked="rule.enabled"
              @change="desktop.updateMonitorRule({ ...rule, enabled: !rule.enabled })"
            />
          </td>
          <td class="ops">
            <button class="op-btn" title="编辑" @click="openEdit(rule)">✏️</button>
            <button class="op-btn" title="删除" @click="removeRule(rule)">🗑️</button>
          </td>
        </tr>
        <tr v-if="desktop.monitorRules.length === 0 && !desktop.monitoringLoading">
          <td colspan="5" class="empty">还没有规则，点“添加规则”配置一个吧~</td>
        </tr>
      </tbody>
    </table>

    <!-- 规则编辑弹窗 -->
    <div v-if="showEditor" class="modal-mask" @click.self="showEditor = false">
      <div class="modal">
        <div class="modal-title">{{ editingRule.app_name && editingRule.trigger_reply ? '编辑规则' : '添加规则' }}</div>
        <label class="field">
          <span>应用名（支持通配符，如 chrome* / notepad.exe）</span>
          <input v-model="editingRule.app_name" placeholder="chrome*" />
        </label>
        <label class="field">
          <span>触发回复</span>
          <textarea v-model="editingRule.trigger_reply" rows="2" placeholder="铃会说的话…" />
        </label>
        <label class="field">
          <span>冷却时间（秒）</span>
          <input v-model.number="editingRule.cooldown_seconds" type="number" min="0" />
        </label>
        <div v-if="editorError" class="editor-error">{{ editorError }}</div>
        <div class="modal-actions">
          <button class="btn cancel" @click="showEditor = false">取消</button>
          <button class="btn confirm" @click="saveRule">保存</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.monitor-settings {
  color: var(--text-main, #eee6e7);
}
.section-title {
  font-weight: 600;
  font-size: var(--fs-15);
  margin-bottom: 14px;
}
.row {
  display: flex;
  align-items: center;
  gap: 28px;
  margin-bottom: 12px;
}
.switch-wrap,
.interval-wrap {
  display: flex;
  align-items: center;
  gap: 8px;
}
.label {
  font-size: var(--fs-13);
  color: var(--text-secondary, #9a9294);
}
.switch {
  width: 40px;
  height: 20px;
  accent-color: var(--accent, #ff7a94);
}
select {
  padding: 5px 8px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--input-bg);
  color: var(--text-main);
}
.unavailable {
  margin-bottom: 10px;
  padding: 8px 12px;
  border-radius: 8px;
  background: rgba(255, 160, 60, 0.15);
  color: var(--warning);
  font-size: var(--fs-12);
}
.actions {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
  flex-wrap: wrap;
}
.btn {
  padding: 6px 14px;
  border-radius: 8px;
  border: none;
  cursor: pointer;
  font-size: var(--fs-13);
}
.btn.primary { background: var(--accent, #ff7a94); color: var(--text-user); }
.btn.ghost { background: rgba(128, 128, 128, 0.18); color: var(--text-main); }
.rule-table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--fs-12);
}
.rule-table th {
  text-align: left;
  padding: 6px 8px;
  color: var(--text-secondary, #9a9294);
  border-bottom: 1px solid var(--border);
  font-weight: 500;
}
.rule-table td {
  padding: 6px 8px;
  border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
  vertical-align: middle;
}
.app-name {
  background: rgba(128, 128, 128, 0.15);
  padding: 2px 6px;
  border-radius: 5px;
  font-size: var(--fs-11);
}
.reply-cell {
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ops {
  white-space: nowrap;
}
.op-btn {
  background: none;
  border: none;
  cursor: pointer;
  font-size: var(--fs-14);
  padding: 2px 4px;
  opacity: 0.8;
}
.op-btn:hover { opacity: 1; }
.empty {
  text-align: center;
  color: var(--text-secondary, #9a9294);
  padding: 18px 0;
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
  width: 340px;
  max-height: 88vh;
  overflow-y: auto;
  box-sizing: border-box;
  background: var(--bg-bar, #262328);
  border: 1px solid var(--border, rgba(255, 255, 255, 0.15));
  border-radius: 14px;
  padding: 18px;
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
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}
.btn.cancel { background: rgba(128, 128, 128, 0.2); color: var(--text-main); }
.btn.confirm { background: var(--accent, #ff7a94); color: var(--text-user); }
</style>
