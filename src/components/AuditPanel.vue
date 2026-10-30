<script setup lang="ts">
// 执行记录面板（抽屉内流式布局）：标题与关闭由抽屉标签条负责，这里只留刷新/导出/清空
import { computed, onMounted, ref, watch } from 'vue'
import { useAuditStore } from '../stores/auditStore'
import type { AuditQuery } from '../types/audit'
import TheIcon from './TheIcon.vue'
import AppButton from './ui/AppButton.vue'
import AppStatusDot from './ui/AppStatusDot.vue'
import AppInput from './ui/AppInput.vue'
import AppEmpty from './ui/AppEmpty.vue'
import { confirmDialog } from '../utils/dialog'

// 关闭由标签条处理；仍声明 close，避免父组件的监听落到根元素上
defineEmits<{ close: [] }>()

const audit = useAuditStore()
const keyword = ref('')
const kind = ref('')
const result = ref('')
const range = ref('all')
const expanded = ref<string | null>(null)
const kindLabels: Record<string, string> = { chat: '对话', agent: 'Agent', sub_agent: '子Agent', goal: '目标', forge: '自造' }
const resultLabels: Record<string, string> = { ok: '成功', partial: '部分', failed: '失败', blocked: '卡住', cancelled: '已取消' }
const resultStates: Record<string, 'ok' | 'warn' | 'err' | 'idle'> = { ok: 'ok', partial: 'warn', failed: 'err', blocked: 'warn', cancelled: 'idle' }
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
  const ok = await confirmDialog('确定清空全部执行记录？此操作不可恢复，不会删除会话、目标或文件。', {
    danger: true,
    confirmText: '清空',
  })
  if (!ok) return
  if (await audit.clear(true)) { expanded.value = null; await refresh() }
}
</script>

<template>
  <div class="audit-panel" role="region" aria-label="执行记录">
    <!-- 轻量操作条：只剩功能按钮，标题与关闭已在抽屉标签条上 -->
    <div class="panel-bar">
      <AppButton size="sm" :disabled="audit.loading" @click="refresh">
        <TheIcon name="refresh" :size="14" />刷新
      </AppButton>
      <AppButton size="sm" :disabled="audit.loading" @click="exportJson">
        <TheIcon name="save" :size="14" />导出 JSON
      </AppButton>
      <AppButton size="sm" variant="text" :disabled="audit.loading" @click="clear">
        <TheIcon name="delete" :size="14" />清空记录
      </AppButton>
      <span class="bar-meta">{{ audit.entries.length }} 条</span>
    </div>

    <div class="panel-body">
      <p class="hint">仅保留简报与路径，不记录参数、文件内容、网页正文或人格设定。归档记录也可检索。导出当前筛选结果。</p>

      <!-- 搜索与筛选：关键词独占一行，三个下拉在窄抽屉里平分一行 -->
      <div class="filters">
        <AppInput v-model="keyword" placeholder="搜索标题、工具名、路径、简报" />
        <div class="filter-row">
          <label class="filter">
            <span>类型</span>
            <select v-model="kind">
              <option value="">全部</option>
              <option v-for="(label, value) in kindLabels" :key="value" :value="value">{{ label }}</option>
            </select>
          </label>
          <label class="filter">
            <span>结果</span>
            <select v-model="result">
              <option value="">全部</option>
              <option v-for="(label, value) in resultLabels" :key="value" :value="value">{{ label }}</option>
            </select>
          </label>
          <label class="filter">
            <span>时间</span>
            <select v-model="range">
              <option value="today">今天</option>
              <option value="week">7天</option>
              <option value="all">全部</option>
            </select>
          </label>
        </div>
      </div>

      <p v-if="audit.error" class="alert err" role="alert">
        <AppStatusDot state="err" text="出错" />
        <span>{{ audit.error }}</span>
      </p>
      <p v-if="audit.loading" class="state-line" role="status">
        <span class="run-dot" aria-hidden="true" />正在读取执行记录
      </p>
      <AppEmpty
        v-else-if="!audit.entries.length"
        text="暂无匹配的执行记录"
        hint="换个关键词，或把类型/结果/时间改成「全部」再看看。"
      />

      <!-- 记录卡片：点头部展开明细 -->
      <article v-for="entry in audit.entries" :key="entry.id" class="entry-card">
        <button
          type="button"
          class="entry-head"
          :aria-expanded="expanded === entry.id"
          @click="expanded = expanded === entry.id ? null : entry.id"
        >
          <span class="entry-meta">{{ time(entry.at) }} · {{ kindLabels[entry.kind] || entry.kind }}</span>
          <strong class="entry-title">{{ entry.title || '未命名任务' }}</strong>
          <span class="entry-meta entry-facts">
            <AppStatusDot :state="resultStates[entry.result] || 'idle'" :text="resultLabels[entry.result] || entry.result" />
            <span>工具数 {{ entry.tools.length }}</span>
            <span>改动文件数 {{ entry.files_changed.length }}</span>
            <span>耗时 {{ entry.duration_ms }} 毫秒</span>
          </span>
        </button>

        <section v-if="expanded === entry.id" class="entry-detail">
          <p class="detail-text">{{ entry.summary }}</p>
          <p v-if="entry.request_id" class="detail-meta">请求：{{ entry.request_id }}</p>
          <p v-if="entry.session_id" class="detail-meta">会话：{{ entry.session_id }}</p>
          <p v-if="entry.goal_id" class="detail-meta">目标：{{ entry.goal_id }}</p>

          <h3 class="detail-title">工具调用明细</h3>
          <p v-if="!entry.tools.length" class="detail-meta">未调用工具</p>
          <ol class="detail-list">
            <li v-for="(tool, index) in entry.tools" :key="index">
              <span class="detail-line">
                <strong>{{ tool.name }}</strong>
                <span>意图：{{ tool.intent || '未标注' }}</span>
                <AppStatusDot :state="tool.ok ? 'ok' : 'err'" :text="tool.ok ? '成功' : '失败或中断'" />
              </span>
              <p class="detail-meta">{{ time(tool.at) }} · {{ tool.brief }}</p>
            </li>
          </ol>

          <h3 class="detail-title">改动的文件路径</h3>
          <p v-if="!entry.files_changed.length" class="detail-meta">未记录文件改动</p>
          <ul class="detail-list">
            <li v-for="path in entry.files_changed" :key="path">{{ path }}</li>
          </ul>
        </section>
      </article>
    </div>
  </div>
</template>

<style scoped>
/* 流式布局：占满抽屉内容区，滚动交给内容区自己 */
.audit-panel {
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

.hint {
  margin: 0;
  font-size: var(--ui-fs-12);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
}

/* 搜索区 */
.filters {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-2);
}
.filter-row {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(88px, 1fr));
  gap: var(--ui-sp-2);
}
.filter {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
  min-width: 0;
}
.filter > span {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
}
.filter select {
  width: 100%;
  min-width: 0;
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-1);
  background: var(--ui-raised);
  color: var(--ui-text);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-sm);
  font-size: var(--ui-fs-12);
}
.filter select:focus {
  border-color: var(--ui-accent);
}

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

/* 读取中：小圆环转圈 */
.state-line {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  margin: 0;
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
}
.run-dot {
  flex: none;
  width: 10px;
  height: 10px;
  border-radius: var(--ui-r-pill);
  border: 2px solid var(--ui-accent-soft);
  border-top-color: var(--ui-accent);
  animation: audit-spin 700ms linear infinite;
}
@keyframes audit-spin {
  to { transform: rotate(360deg); }
}

/* 记录卡片 */
.entry-card {
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-md);
  background: var(--ui-raised);
  overflow: hidden;
}
.entry-head {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: var(--ui-sp-1);
  width: 100%;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border: 0;
  background: transparent;
  color: var(--ui-text);
  text-align: left;
  transition: background var(--ui-dur-1) var(--ui-ease);
}
.entry-head:hover {
  background: var(--ui-hover);
}
.entry-title {
  font-size: var(--ui-fs-13);
  font-weight: 600;
  line-height: var(--ui-line);
  overflow-wrap: anywhere;
}
.entry-meta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
}
.entry-detail {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
  padding: var(--ui-sp-2) var(--ui-sp-3) var(--ui-sp-3);
  border-top: 1px solid var(--ui-border);
  font-size: var(--ui-fs-12);
}
.detail-text {
  margin: 0;
  color: var(--ui-text);
  line-height: var(--ui-line);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.detail-meta {
  margin: 0;
  color: var(--ui-text-3);
  line-height: var(--ui-line);
  overflow-wrap: anywhere;
}
.detail-title {
  margin: var(--ui-sp-2) 0 0;
  font-size: var(--ui-fs-12);
  font-weight: 600;
  color: var(--ui-text-2);
}
.detail-line {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  color: var(--ui-text-2);
}
.detail-list {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
  margin: 0;
  padding-left: var(--ui-sp-4);
  color: var(--ui-text-2);
  line-height: var(--ui-line);
}
.detail-list li {
  overflow-wrap: anywhere;
}
</style>
