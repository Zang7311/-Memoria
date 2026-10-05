<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useAuditStore } from '../stores/auditStore'
import type { AuditQuery } from '../types/audit'

const emit = defineEmits<{ close: [] }>()
const audit = useAuditStore()
const keyword = ref('')
const kind = ref('')
const result = ref('')
const range = ref('all')
const expanded = ref<string | null>(null)
const kindLabels: Record<string, string> = { chat: '对话', agent: 'Agent', sub_agent: '子Agent', goal: '目标', forge: '自造' }
const resultLabels: Record<string, string> = { ok: '成功', partial: '部分', failed: '失败', blocked: '卡住', cancelled: '已取消' }
const query = computed<AuditQuery>(() => {
  const since = range.value === 'today' ? new Date().setHours(0, 0, 0, 0) / 1000
    : range.value === 'week' ? Math.floor(Date.now() / 1000) - 7 * 86400 : undefined
  return { keyword: keyword.value, kind: kind.value, result: result.value, since }
})
function refresh() { return audit.load(query.value) }
onMounted(refresh)
watch(query, refresh)
function time(at: number) { return new Date(at * 1000).toLocaleString('zh-CN') }
async function exportJson() {
  const json = await audit.exportJson(query.value)
  if (json === null) return
  const url = URL.createObjectURL(new Blob([json], { type: 'application/json;charset=utf-8' }))
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = '执行记录.json'
  anchor.click()
  setTimeout(() => URL.revokeObjectURL(url), 1000)
}
async function clear() {
  if (!confirm('确定清空全部执行记录？此操作不可恢复，不会删除会话、目标或文件。')) return
  if (await audit.clear(true)) { expanded.value = null; await refresh() }
}
</script>

<template>
  <aside class="audit-panel" aria-label="执行记录面板">
    <header><h2>执行记录</h2><button :disabled="audit.loading" @click="refresh">刷新</button><button @click="emit('close')">关闭</button></header>
    <p class="hint">仅保留简报与路径，不记录参数、文件内容、网页正文或人格设定。归档记录也可检索。导出当前筛选结果。</p>
    <div class="filters">
      <label>关键词<input v-model="keyword" placeholder="标题、工具名、路径、简报" /></label>
      <label>类型<select v-model="kind"><option value="">全部</option><option v-for="(label, value) in kindLabels" :key="value" :value="value">{{ label }}</option></select></label>
      <label>结果<select v-model="result"><option value="">全部</option><option v-for="(label, value) in resultLabels" :key="value" :value="value">{{ label }}</option></select></label>
      <label>时间<select v-model="range"><option value="today">今天</option><option value="week">7天</option><option value="all">全部</option></select></label>
    </div>
    <div class="actions"><button :disabled="audit.loading" @click="exportJson">导出 JSON</button><button :disabled="audit.loading" @click="clear">清空全部记录</button><span>{{ audit.entries.length }} 条</span></div>
    <p v-if="audit.error" class="error" role="alert">{{ audit.error }}</p>
    <p v-if="audit.loading" role="status">正在读取执行记录</p>
    <p v-else-if="!audit.entries.length">暂无匹配的执行记录</p>
    <article v-for="entry in audit.entries" :key="entry.id">
      <button class="entry" :aria-expanded="expanded === entry.id" @click="expanded = expanded === entry.id ? null : entry.id">
        <span>{{ time(entry.at) }} · {{ kindLabels[entry.kind] || entry.kind }}</span>
        <strong>{{ entry.title || '未命名任务' }}</strong>
        <span>工具数 {{ entry.tools.length }} · 改动文件数 {{ entry.files_changed.length }} · {{ resultLabels[entry.result] || entry.result }} · 耗时 {{ entry.duration_ms }} 毫秒</span>
      </button>
      <section v-if="expanded === entry.id">
        <p>{{ entry.summary }}</p>
        <p v-if="entry.request_id">请求：{{ entry.request_id }}</p>
        <p v-if="entry.session_id">会话：{{ entry.session_id }}</p>
        <p v-if="entry.goal_id">目标：{{ entry.goal_id }}</p>
        <h3>工具调用明细</h3>
        <p v-if="!entry.tools.length">未调用工具</p>
        <ol><li v-for="(tool, index) in entry.tools" :key="index"><strong>{{ tool.name }}</strong> · 意图：{{ tool.intent || '未标注' }} · {{ tool.ok ? '成功' : '失败或中断' }}<p>{{ time(tool.at) }} · {{ tool.brief }}</p></li></ol>
        <h3>改动的文件路径</h3><p v-if="!entry.files_changed.length">未记录文件改动</p>
        <ul><li v-for="path in entry.files_changed" :key="path">{{ path }}</li></ul>
      </section>
    </article>
  </aside>
</template>

<style scoped>
.audit-panel { position: fixed; right: 16px; top: 64px; bottom: 24px; width: min(620px, calc(100vw - 32px)); z-index: 100; overflow-y: auto; padding: 20px; box-sizing: border-box; background: var(--bg-main); color: var(--text-main); border: 1px solid var(--border, #8886); border-radius: var(--radius-ui, 12px); box-shadow: 0 8px 30px #0004; }
header, .actions, .filters { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
h2 { margin: 0 auto 0 0; }
h3 { font-size: 14px; }
.hint, .entry span { font-size: 12px; color: var(--text-secondary); }
label { display: flex; gap: 6px; align-items: center; margin: 6px 0; }
input, select, button { padding: 6px 10px; border: 1px solid var(--border, #8886); border-radius: 6px; background: var(--bg-main); color: var(--text-main); }
input { min-width: 0; width: 190px; }
button { cursor: pointer; }
button:disabled { opacity: 0.45; cursor: not-allowed; }
article { border-top: 1px solid var(--border, #8886); margin-top: 12px; padding-top: 12px; }
.entry { display: flex; flex-direction: column; align-items: flex-start; gap: 6px; text-align: left; width: 100%; }
p, li, strong { white-space: pre-wrap; overflow-wrap: anywhere; }
.error { color: var(--danger, #df5555); }
</style>
