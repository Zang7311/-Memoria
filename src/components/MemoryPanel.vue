<!-- 《铃·记忆体》记忆面板（AI-4）
     右侧可收起面板：时间线分组展示 + 搜索 + 删除 + 收藏 + 记忆集管理 -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useMemoryStore } from '../stores/memoryStore'
import type { Memory } from '../types'

const store = useMemoryStore()

// 面板展开/收起
const expanded = ref(true)

// 新建记忆集输入框
const showCreateInput = ref(false)
const newSetName = ref('')

// 按日期分组（timestamp 前 10 位为日期）
interface DayGroup {
  date: string
  items: Memory[]
}
const groups = computed<DayGroup[]>(() => {
  const map = new Map<string, Memory[]>()
  for (const m of store.memories) {
    const date = (m.timestamp || '').slice(0, 10) || '未知日期'
    if (!map.has(date)) map.set(date, [])
    map.get(date)!.push(m)
  }
  // 日期倒序，组内时间倒序（最新的在前）
  const arr = [...map.entries()].sort((a, b) => (a[0] < b[0] ? 1 : -1))
  return arr.map(([date, items]) => ({
    date,
    items: [...items].sort((a, b) => (a.timestamp < b.timestamp ? 1 : -1)),
  }))
})

// 展开的条目 id
const expandedIds = ref<Set<string>>(new Set())

function toggleExpand(id: string) {
  if (expandedIds.value.has(id)) expandedIds.value.delete(id)
  else expandedIds.value.add(id)
  // 触发响应式更新
  expandedIds.value = new Set(expandedIds.value)
}

function isExpanded(id: string) {
  return expandedIds.value.has(id)
}

function timeOf(m: Memory) {
  return (m.timestamp || '').slice(11, 16) || '--:--'
}

function roleLabel(m: Memory) {
  return m.role === 'assistant' ? '铃' : '主人'
}

// 摘要（前 20 字）
function summaryOf(m: Memory) {
  const text = m.content || ''
  return text.length > 20 ? text.slice(0, 20) + '…' : text
}

// 删除（带确认）
function onDelete(m: Memory) {
  if (confirm(`确定删除这条记忆吗？\n${summaryOf(m)}`)) {
    store.deleteMemory(m.id)
  }
}

// 收藏
function onMark(m: Memory) {
  store.markImportant(m.id)
}

function isImportant(m: Memory) {
  return !!m.tags?.includes('important')
}

// 搜索（防抖 300ms）
let searchTimer: ReturnType<typeof setTimeout> | undefined
function onSearchInput() {
  clearTimeout(searchTimer)
  searchTimer = setTimeout(() => store.search(store.searchKeyword), 300)
}

// 切换记忆集
function onSwitchSet(e: Event) {
  const name = (e.target as HTMLSelectElement).value
  if (name !== store.currentSet) store.switchSet(name)
}

// 新建记忆集
async function onCreateSet() {
  const name = newSetName.value.trim()
  if (!name) return
  await store.createSet(name)
  newSetName.value = ''
  showCreateInput.value = false
}

onMounted(() => {
  store.loadSets()
  store.loadMemories()
})
</script>

<template>
  <aside class="memory-panel" :class="{ expanded }">
    <!-- 面板头部 -->
    <div class="panel-header" @click="expanded = !expanded">
      <span v-if="expanded" class="panel-title">记忆</span>
      <span v-else class="collapsed-icon" title="展开记忆面板">📒</span>
      <span class="toggle-btn">{{ expanded ? '❯' : '❮' }}</span>
    </div>

    <template v-if="expanded">
      <!-- 记忆集管理 -->
      <div class="set-bar">
        <select :value="store.currentSet" class="set-select" @change="onSwitchSet">
          <option v-for="s in store.sets" :key="s" :value="s">{{ s }}</option>
        </select>
        <button class="new-set-btn" title="新建记忆集" @click="showCreateInput = !showCreateInput">＋</button>
      </div>
      <div v-if="showCreateInput" class="create-set-row">
        <input
          v-model="newSetName"
          class="set-input"
          placeholder="记忆集名称"
          @keyup.enter="onCreateSet"
        />
        <button class="confirm-btn" @click="onCreateSet">确定</button>
      </div>

      <!-- 搜索框 -->
      <div class="search-box">
        <input
          v-model="store.searchKeyword"
          class="search-input"
          placeholder="🔍 搜索记忆…"
          @input="onSearchInput"
        />
      </div>

      <!-- 错误提示 -->
      <div v-if="store.errorMsg" class="error-tip">{{ store.errorMsg }}</div>

      <!-- 时间线 -->
      <div class="timeline">
        <div v-if="store.isLoading" class="loading">加载中…</div>
        <div v-else-if="groups.length === 0" class="empty">还没有记忆哦～</div>

        <div v-for="g in groups" :key="g.date" class="day-group">
          <div class="day-label">{{ g.date }}</div>
          <div
            v-for="m in g.items"
            :key="m.id"
            class="mem-item"
            :class="{ expanded: isExpanded(m.id) }"
          >
            <div class="mem-row" @click="toggleExpand(m.id)">
              <span class="mem-time">{{ timeOf(m) }}</span>
              <span class="mem-role" :class="m.role">{{ roleLabel(m) }}</span>
              <span class="mem-preview">{{ isExpanded(m.id) ? m.content : summaryOf(m) }}</span>
            </div>
            <div v-if="isExpanded(m.id)" class="mem-full">
              <div class="mem-full-content">{{ m.content }}</div>
              <div class="mem-actions">
                <button
                  class="act-btn"
                  :class="{ active: isImportant(m) }"
                  :title="isImportant(m) ? '已收藏' : '标记重要'"
                  @click="onMark(m)"
                >
                  ⭐
                </button>
                <button class="act-btn danger" title="删除" @click="onDelete(m)">🗑️</button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </template>
  </aside>
</template>

<style scoped>
.memory-panel {
  width: 44px;
  flex-shrink: 0;
  border-left: 1px solid var(--border, rgba(128, 128, 128, 0.2));
  background: var(--bg-bar, rgba(255, 255, 255, 0.6));
  transition: width 0.25s ease;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.memory-panel.expanded {
  width: 280px;
}
.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border, rgba(128, 128, 128, 0.2));
  cursor: pointer;
  user-select: none;
}
/* 收起态：竖排窄条，只显示图标 */
.memory-panel:not(.expanded) .panel-header {
  flex-direction: column;
  gap: 6px;
  padding: 12px 0;
  min-height: 64px;
}
.collapsed-icon {
  font-size: 16px;
  line-height: 1;
}
.panel-title {
  font-size: 14px;
  font-weight: 600;
  white-space: nowrap;
  cursor: pointer;
  user-select: none;
}
.toggle-btn {
  cursor: pointer;
  opacity: 0.7;
}
.memory-panel:not(.expanded) .toggle-btn {
  font-size: 10px;
}
.set-bar {
  display: flex;
  gap: 6px;
  padding: 8px 12px 0;
}
.set-select {
  flex: 1;
  min-width: 0;
  font-size: 12px;
  padding: 4px 6px;
  border-radius: 6px;
  border: 1px solid var(--border, rgba(128, 128, 128, 0.3));
  background: var(--input-bg, #fff);
  color: var(--text-main, #222);
}
.new-set-btn {
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 16px;
  opacity: 0.7;
}
.create-set-row {
  display: flex;
  gap: 6px;
  padding: 6px 12px 0;
}
.set-input {
  flex: 1;
  min-width: 0;
  font-size: 12px;
  padding: 4px 6px;
  border-radius: 6px;
  border: 1px solid var(--border, rgba(128, 128, 128, 0.3));
  background: var(--input-bg, #fff);
  color: var(--text-main, #222);
}
.confirm-btn {
  font-size: 12px;
  padding: 2px 8px;
  border: none;
  border-radius: 6px;
  background: var(--accent, #ffb6c1);
  color: #fff;
  cursor: pointer;
}
.search-box {
  padding: 8px 12px 0;
}
.search-input {
  width: 100%;
  box-sizing: border-box;
  font-size: 12px;
  padding: 5px 8px;
  border-radius: 8px;
  border: 1px solid var(--border, rgba(128, 128, 128, 0.3));
  background: var(--input-bg, #fff);
  color: var(--text-main, #222);
}
.error-tip {
  margin: 8px 12px 0;
  font-size: 11px;
  color: var(--danger, #e53935);
}
.timeline {
  flex: 1;
  overflow-y: auto;
  padding: 8px 10px;
}
.loading,
.empty {
  font-size: 12px;
  color: var(--text-secondary, #888);
  text-align: center;
  padding: 16px 0;
}
.day-group {
  margin-bottom: 10px;
}
.day-label {
  font-size: 11px;
  color: var(--text-secondary, #888);
  padding: 2px 0 4px;
  border-bottom: 1px dashed var(--border, rgba(128, 128, 128, 0.2));
  margin-bottom: 4px;
}
.mem-item {
  border-radius: 8px;
  padding: 4px 6px;
  cursor: pointer;
}
.mem-item:hover {
  background: rgba(128, 128, 128, 0.08);
}
.mem-row {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
}
.mem-time {
  color: var(--text-secondary, #888);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}
.mem-role {
  flex-shrink: 0;
  font-size: 11px;
  padding: 1px 5px;
  border-radius: 4px;
  color: #fff;
}
.mem-role.user {
  background: #5b8def;
}
.mem-role.assistant {
  background: #ff9ec4;
}
.mem-preview {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-main, #222);
}
.mem-full {
  margin-top: 4px;
  font-size: 12px;
}
.mem-full-content {
  background: rgba(128, 128, 128, 0.08);
  border-radius: 6px;
  padding: 6px 8px;
  white-space: pre-wrap;
  word-break: break-word;
}
.mem-actions {
  display: flex;
  gap: 6px;
  margin-top: 4px;
  justify-content: flex-end;
}
.act-btn {
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 14px;
  opacity: 0.6;
}
.act-btn:hover {
  opacity: 1;
}
.act-btn.active {
  opacity: 1;
}
.act-btn.danger:hover {
  filter: drop-shadow(0 0 2px rgba(229, 57, 53, 0.6));
}
</style>
