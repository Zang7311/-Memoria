<!-- 《铃·记忆体》诊断面板（AI-7 4.3）
     系统信息 + 日志查看器（级别过滤/搜索）+ 一键导出诊断包 -->
<script setup lang="ts">
import { onMounted, ref } from 'vue'
import type { LogLevel, SystemInfo } from '../types'
import { clearLogs, exportDiagnostic, getLogs, getSystemInfo } from '../utils/tauri'

const info = ref<SystemInfo | null>(null)
const loadingInfo = ref(false)

const logs = ref<string[]>([])
const total = ref(0)
const levelFilter = ref<LogLevel | undefined>(undefined)
const keyword = ref('')
const logMsg = ref('')

function lineClass(line: string): string {
  const upper = line.toUpperCase()
  if (upper.includes('[ERROR]')) return 'err'
  if (upper.includes('[WARN]')) return 'warn'
  if (upper.includes('[INFO]')) return 'info'
  return ''
}

async function loadInfo() {
  loadingInfo.value = true
  try {
    const res = await getSystemInfo()
    info.value = res.info
  } catch (e) {
    logMsg.value = `✗ 系统信息获取失败：${e}`
  } finally {
    loadingInfo.value = false
  }
}

async function loadLogs() {
  try {
    const res = await getLogs({ limit: 300, level: levelFilter.value, keyword: keyword.value || undefined })
    logs.value = res.logs
    total.value = res.total
  } catch (e) {
    logMsg.value = `✗ 日志获取失败：${e}`
  }
}

async function doExport() {
  try {
    const res = await exportDiagnostic({ include_logs: true, include_config: true, include_system_info: true })
    logMsg.value = res.success ? `✓ 诊断包已导出：${res.file_path}` : `✗ 导出失败：${res.error}`
  } catch (e) {
    logMsg.value = `✗ 导出失败：${e}`
  }
}

async function doClear() {
  await clearLogs()
  await loadLogs()
  logMsg.value = '✓ 日志已清空'
}

onMounted(() => {
  loadInfo()
  loadLogs()
})
</script>

<template>
  <div class="diag">
    <!-- 系统信息 -->
    <div class="card">
      <div class="card-title">🖥️ 系统信息</div>
      <div v-if="loadingInfo" class="hint">加载中…</div>
      <table v-else-if="info" class="info-table">
        <tr><td>应用版本</td><td>{{ info.app_version }}</td></tr>
        <tr><td>操作系统</td><td>{{ info.os_name }} {{ info.os_version }}</td></tr>
        <tr><td>CPU</td><td>{{ info.cpu_name }}（{{ info.cpu_cores }} 核，使用率 {{ info.cpu_usage.toFixed(1) }}%）</td></tr>
        <tr><td>内存</td><td>{{ (info.memory_used_mb / 1024).toFixed(1) }} / {{ (info.memory_total_mb / 1024).toFixed(1) }} GB</td></tr>
        <tr v-for="d in info.disks" :key="d.name">
          <td>磁盘 {{ d.name }}</td>
          <td>可用 {{ d.available_gb }} / 共 {{ d.total_gb }} GB</td>
        </tr>
      </table>
    </div>

    <!-- 日志查看器 -->
    <div class="card">
      <div class="card-title">📋 日志查看器（共 {{ total }} 条）</div>
      <div class="log-toolbar">
        <select v-model="levelFilter" class="input" @change="loadLogs">
          <option :value="undefined">全部级别</option>
          <option value="trace">TRACE</option>
          <option value="debug">DEBUG</option>
          <option value="info">INFO</option>
          <option value="warn">WARN</option>
          <option value="error">ERROR</option>
        </select>
        <input v-model="keyword" class="input" placeholder="搜索关键词…" @keyup.enter="loadLogs" />
        <button class="btn ghost" @click="loadLogs">搜索</button>
        <button class="btn ghost" @click="doClear">清空</button>
      </div>
      <div class="log-list">
        <div v-if="logs.length === 0" class="hint">暂无日志</div>
        <div v-for="(l, i) in logs" :key="i" class="log-line" :class="lineClass(l)">{{ l }}</div>
      </div>
    </div>

    <!-- 导出诊断包 -->
    <div class="card">
      <div class="card-title">📦 一键导出诊断包</div>
      <p class="hint">打包（脱敏配置 + 全部日志 + 系统信息）为 zip，保存到数据目录下 diagnostics/</p>
      <button class="btn primary" @click="doExport">导出诊断包</button>
      <div v-if="logMsg" class="msg">{{ logMsg }}</div>
    </div>
  </div>
</template>

<style scoped>
.diag { display: flex; flex-direction: column; gap: 14px; }
.card {
  background: var(--bg-bar, rgba(34, 32, 36, 0.85));
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  border-radius: 14px; padding: 16px;
}
.card-title { font-weight: 600; font-size: var(--fs-14); margin-bottom: 10px; }
.hint { font-size: var(--fs-12); color: var(--text-secondary); margin: 0 0 10px; }
.info-table { width: 100%; border-collapse: collapse; font-size: var(--fs-13); }
.info-table td { padding: 5px 8px; border-bottom: 1px solid var(--border); }
.info-table td:first-child { color: var(--text-secondary); width: 120px; white-space: nowrap; }
.log-toolbar { display: flex; gap: 8px; margin-bottom: 10px; flex-wrap: wrap; }
.input {
  padding: 6px 10px; border-radius: 8px; border: 1px solid var(--border);
  background: var(--input-bg); color: var(--text-main); font-size: var(--fs-12);
}
.log-list {
  max-height: 260px; overflow-y: auto; background: rgba(0, 0, 0, 0.25);
  border-radius: 8px; padding: 8px; font-family: Consolas, monospace; font-size: var(--fs-11);
}
.log-line { padding: 2px 4px; white-space: pre-wrap; word-break: break-all; color: var(--text-secondary); }
.log-line.err { color: var(--danger); }
.log-line.warn { color: var(--warning); }
.log-line.info { color: var(--info); }
.btn { padding: 6px 14px; border-radius: 8px; border: none; cursor: pointer; font-size: var(--fs-12); }
.btn.primary { background: var(--accent, #ff7a94); color: var(--text-user); }
.btn.ghost { background: rgba(128, 128, 128, 0.18); color: var(--text-main); }
.msg { font-size: var(--fs-12); margin-top: 8px; color: var(--accent); word-break: break-all; }
</style>
