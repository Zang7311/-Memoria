<!-- 《铃·记忆体》救援模式（P3）
     启动参数 --recovery 或配置损坏时进入的极简恢复窗口：
     检测配置/记忆/插件/日志 + 导出诊断报告 + 重置配置 + 退出 -->
<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { exportDiagnostic, recoveryCheck, recoveryResetConfig } from '../utils/tauri'
import { confirmDialog } from '../utils/dialog'
import TheIcon from '../components/TheIcon.vue'

interface CheckItem {
  name: string
  ok: boolean
  detail: string
}

const items = ref<CheckItem[]>([])
const checking = ref(true)
const msg = ref('')
const busy = ref(false)
const done = ref(false)

onMounted(async () => {
  try {
    items.value = await recoveryCheck()
  } catch (e) {
    msg.value = `检测失败：${e}`
  } finally {
    checking.value = false
  }
})

// 导出诊断报告
async function onExport() {
  busy.value = true
  msg.value = ''
  try {
    const res = await exportDiagnostic({ include_logs: true, include_config: true, include_system_info: true })
    if (res.success && res.file_path) {
      msg.value = `诊断报告已导出：${res.file_path}`
    } else {
      msg.value = `导出失败：${res.error || '未知错误'}`
    }
  } catch (e) {
    msg.value = `导出失败：${e}`
  } finally {
    busy.value = false
  }
}

// 重置配置
async function onResetConfig() {
  const ok = await confirmDialog('将备份当前配置并恢复默认设置（记忆数据不受影响）。继续？', {
    danger: true,
    confirmText: '恢复默认',
  })
  if (!ok) return
  busy.value = true
  msg.value = ''
  try {
    const r = await recoveryResetConfig()
    msg.value = `${r}`
    done.value = true
  } catch (e) {
    msg.value = `重置失败：${e}`
  } finally {
    busy.value = false
  }
}

// 退出恢复模式
function onExit() {
  window.close()
}
</script>

<template>
  <div class="recovery">
    <div class="rec-card">
      <div class="rec-logo"><TheIcon name="toolbox" :size="20" /> 铃·恢复模式</div>
      <p class="rec-sub">配置或启动出现问题，先在这里检查一下～</p>

      <!-- 检测列表 -->
      <div class="rec-checks">
        <div v-if="checking" class="rec-loading">检测中…</div>
        <div v-for="it in items" :key="it.name" class="rec-item" :class="{ bad: !it.ok }">
          <span class="rec-mark" />
          <span class="rec-name">{{ it.name }}</span>
          <span class="rec-detail">{{ it.detail }}</span>
        </div>
      </div>

      <!-- 操作 -->
      <div class="rec-actions">
        <button class="rec-btn" :disabled="busy" @click="onExport"><TheIcon name="file" :size="14" /> 导出诊断报告</button>
        <button class="rec-btn danger" :disabled="busy" @click="onResetConfig"><TheIcon name="refresh" :size="14" /> 重置配置</button>
        <button class="rec-btn primary" @click="onExit"><TheIcon name="power" :size="14" /> 退出</button>
      </div>
      <p v-if="done" class="rec-tip">配置已重置，请退出后重新启动软件。</p>
      <p v-if="msg" class="rec-msg">{{ msg }}</p>
      <p class="rec-tip">提示：导出诊断报告后，可把文件发给开发者排查问题。</p>
    </div>
  </div>
</template>

<style scoped>
.recovery {
  height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--ui-sp-4);
  background: linear-gradient(160deg, var(--ui-bg), var(--ui-panel));
  color: var(--ui-text);
}
.rec-card {
  width: min(560px, 100%);
  padding: var(--ui-sp-5) var(--ui-sp-6);
  background: var(--ui-raised);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-lg);
  box-shadow: var(--ui-shadow);
  color: var(--ui-text);
}
.rec-logo {
  font-size: var(--ui-fs-20);
  font-weight: 800;
  color: var(--ui-accent);
}
.rec-sub {
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
  margin: var(--ui-sp-1) 0 var(--ui-sp-4);
}
.rec-checks {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
  margin-bottom: var(--ui-sp-4);
}
.rec-loading {
  font-size: var(--ui-fs-13);
  color: var(--ui-text-3);
  text-align: center;
  padding: var(--ui-sp-2);
}
.rec-item {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border: 1px solid transparent;
  border-radius: var(--ui-r-md);
  background: var(--ui-hover);
  font-size: var(--ui-fs-13);
}
.rec-item.bad {
  background: color-mix(in srgb, var(--ui-err) 12%, transparent);
  border-color: color-mix(in srgb, var(--ui-err) 30%, transparent);
}
.rec-mark {
  width: 8px;
  height: 8px;
  flex: none;
  border-radius: var(--ui-r-pill);
  background: var(--ui-ok);
}
.rec-item.bad .rec-mark {
  background: var(--ui-err);
}
.rec-name {
  font-weight: 600;
  flex-shrink: 0;
  color: var(--ui-text);
}
.rec-detail {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.rec-actions {
  display: flex;
  gap: var(--ui-sp-2);
  flex-wrap: wrap;
}
.rec-btn {
  flex: 1;
  min-width: 120px;
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-3);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  background: var(--ui-panel);
  color: var(--ui-text);
  font-size: var(--ui-fs-13);
  cursor: pointer;
  transition: background var(--ui-dur-1) var(--ui-ease), border-color var(--ui-dur-1) var(--ui-ease);
}
.rec-btn:hover:not(:disabled) {
  background: var(--ui-hover);
  border-color: var(--ui-border-strong);
}
.rec-btn.primary {
  background: var(--ui-accent);
  border-color: var(--ui-accent);
  color: var(--ui-on-accent);
  font-weight: 600;
}
.rec-btn.danger {
  border-color: color-mix(in srgb, var(--ui-err) 50%, transparent);
  color: var(--ui-err);
}
.rec-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.rec-tip {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  margin: var(--ui-sp-2) 0 0;
}
.rec-msg {
  font-size: var(--ui-fs-12);
  color: var(--ui-accent);
  margin: var(--ui-sp-2) 0 0;
}
</style>
