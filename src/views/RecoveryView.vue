<!-- 《铃·记忆体》救援模式（P3）
     启动参数 --recovery 或配置损坏时进入的极简恢复窗口：
     检测配置/记忆/插件/日志 + 导出诊断报告 + 重置配置 + 退出 -->
<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { exportDiagnostic, recoveryCheck, recoveryResetConfig } from '../utils/tauri'

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
      msg.value = `✅ 诊断报告已导出：${res.file_path}`
    } else {
      msg.value = `✗ 导出失败：${res.error || '未知错误'}`
    }
  } catch (e) {
    msg.value = `✗ 导出失败：${e}`
  } finally {
    busy.value = false
  }
}

// 重置配置
async function onResetConfig() {
  if (!confirm('将备份当前配置并恢复默认设置（记忆数据不受影响）。继续？')) return
  busy.value = true
  msg.value = ''
  try {
    const r = await recoveryResetConfig()
    msg.value = `✅ ${r}`
    done.value = true
  } catch (e) {
    msg.value = `✗ 重置失败：${e}`
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
      <div class="rec-logo">🔧 铃·恢复模式</div>
      <p class="rec-sub">配置或启动出现问题，先在这里检查一下～</p>

      <!-- 检测列表 -->
      <div class="rec-checks">
        <div v-if="checking" class="rec-loading">检测中…</div>
        <div v-for="it in items" :key="it.name" class="rec-item" :class="{ bad: !it.ok }">
          <span class="rec-mark">{{ it.ok ? '✓' : '⚠' }}</span>
          <span class="rec-name">{{ it.name }}</span>
          <span class="rec-detail">{{ it.detail }}</span>
        </div>
      </div>

      <!-- 操作 -->
      <div class="rec-actions">
        <button class="rec-btn" :disabled="busy" @click="onExport">📦 导出诊断报告</button>
        <button class="rec-btn danger" :disabled="busy" @click="onResetConfig">♻️ 重置配置</button>
        <button class="rec-btn primary" @click="onExit">🚪 退出</button>
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
  background: linear-gradient(160deg, #2e2e40, #23233a);
  font-family: 'Microsoft YaHei', 'PingFang SC', sans-serif;
}
.rec-card {
  width: 560px;
  background: rgba(255, 255, 255, 0.06);
  backdrop-filter: blur(16px);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 16px;
  padding: 24px 28px;
  color: #eee;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.4);
}
.rec-logo { font-size: 20px; font-weight: 800; color: #ff9db4; }
.rec-sub { font-size: 12px; color: #aaa; margin: 6px 0 16px; }
.rec-checks { display: flex; flex-direction: column; gap: 6px; margin-bottom: 16px; }
.rec-loading { font-size: 13px; color: #888; text-align: center; padding: 10px; }
.rec-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.05);
  font-size: 13px;
}
.rec-item.bad { background: rgba(255, 69, 58, 0.12); border: 1px solid rgba(255, 69, 58, 0.3); }
.rec-mark { font-weight: 800; color: #4caf50; }
.rec-item.bad .rec-mark { color: #ff453a; }
.rec-name { font-weight: 600; flex-shrink: 0; }
.rec-detail { font-size: 11px; color: #999; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rec-actions { display: flex; gap: 8px; }
.rec-btn {
  flex: 1;
  padding: 9px 0;
  border-radius: 10px;
  border: 1px solid rgba(255, 255, 255, 0.2);
  background: rgba(255, 255, 255, 0.08);
  color: #eee;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.15s;
}
.rec-btn:hover { background: rgba(255, 255, 255, 0.16); }
.rec-btn.primary { background: #ff7a94; border-color: #ff7a94; color: #fff; font-weight: 600; }
.rec-btn.danger { border-color: rgba(255, 69, 58, 0.5); color: #ff6b5e; }
.rec-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.rec-tip { font-size: 11px; color: #888; margin: 10px 0 0; }
.rec-msg { font-size: 12px; color: #ff9db4; margin: 10px 0 0; }
</style>
