<!-- 《铃·记忆体》记忆面板（AI-4）
     右侧可收起面板：时间线分组展示 + 搜索 + 删除 + 收藏 + 记忆集管理 -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useMemoryStore } from '../stores/memoryStore'
import { useMilestoneStore } from '../stores/milestoneStore'
import type { Memory } from '../types'

const store = useMemoryStore()
const milestone = useMilestoneStore()

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
  // 记忆中心：按分类筛选后的列表分组
  const list = store.filteredMemories
  const map = new Map<string, Memory[]>()
  for (const m of list) {
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

// —— 记忆透明页（P1）：统计与重要记忆 ——
// 重要记忆（当前列表内）
const importantMemories = computed(() => store.memories.filter((m) => isImportant(m)))
// 最早/最近时间（当前列表）
const earliest = computed(() => {
  const t = store.memories.map((m) => m.timestamp || '').filter(Boolean).sort()
  return t.length ? t[0].slice(0, 10) : '—'
})
const latest = computed(() => {
  const t = store.memories.map((m) => m.timestamp || '').filter(Boolean).sort()
  return t.length ? t[t.length - 1].slice(0, 10) : '—'
})
// 隐私说明折叠
const showPrivacy = ref(false)

// 删除（带确认）
function onDelete(m: Memory) {
  if (confirm(`确定删除这条记忆吗？\n${summaryOf(m)}`)) {
    store.deleteMemory(m.id)
  }
}

// 编辑（记忆中心：改内容后重新分类）
function onEdit(m: Memory) {
  const newContent = prompt('编辑这条记忆：', m.content)
  if (newContent !== null && newContent.trim() && newContent !== m.content) {
    store.editMemory(m.id, newContent.trim())
  }
}

// 批量删除（带确认）
function batchDelete() {
  const n = store.selectedIds.size
  if (n === 0) return
  if (confirm(`确定删除选中的 ${n} 条记忆吗？`)) {
    store.deleteSelected()
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
  // P3：第一次创建记忆集里程碑（幂等）
  milestone.record('first_memory', '创建了第一个记忆集').catch(() => {})
}

onMounted(() => {
  store.loadSets()
  store.loadMemories()
  store.loadStats()
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

      <!-- 记忆透明页（P1 重点）：铃记住了什么 -->
      <div class="transparency">
        <div class="trans-head" @click="showPrivacy = !showPrivacy" title="点击查看隐私说明">
          <span class="trans-title">铃记住了</span>
          <span class="trans-toggle">{{ showPrivacy ? '❯' : '❮' }}</span>
        </div>

        <!-- 统计行 -->
        <div class="trans-stats">
          <div class="stat"><b>{{ store.memories.length }}</b><span>条记忆</span></div>
          <div class="stat"><b>{{ importantMemories.length }}</b><span>条重要</span></div>
          <div class="stat"><b>{{ earliest }}</b><span>最早</span></div>
          <div class="stat"><b>{{ latest }}</b><span>最近</span></div>
        </div>

        <!-- 重要记忆胶囊（铃最该记住的事） -->
        <div v-if="importantMemories.length > 0" class="important-box">
          <div class="important-label">⭐ 铃最在意的：</div>
          <div v-for="m in importantMemories.slice(0, 5)" :key="m.id" class="important-chip">
            <span class="imp-text" :title="m.content">{{ summaryOf(m) }}</span>
            <span class="imp-role">{{ roleLabel(m) }}</span>
            <button class="imp-unmark" title="取消重要" @click="onMark(m)">✕</button>
          </div>
          <div v-if="importantMemories.length > 5" class="important-more">还有 {{ importantMemories.length - 5 }} 条…</div>
        </div>
        <div v-else class="important-empty">还没有重要记忆——聊天中觉得哪句重要，可以在这里点 ⭐ 收藏</div>

        <!-- 隐私承诺（折叠） -->
        <div v-if="showPrivacy" class="privacy-box">
          <p>🔒 <b>记忆只保存在本机</b>（你的数据目录），不会上传到任何地方。</p>
          <p>设置主密码后，API 密钥等敏感信息会加密存储。</p>
          <p>你可以随时在下方时间线里查看、删除或收藏任何一条记忆。</p>
        </div>
        <div class="privacy-hint">🔒 本机存储 · 不上传 · 可随时删除</div>
      </div>

      <!-- 记忆中心（大项目）：容量 + 分类筛选 + 批量操作 -->
      <div class="center-bar">
        <div v-if="store.stats" class="cap-row">
          <span class="cap-item">🧠 {{ store.stats.total }} 条</span>
          <span class="cap-item">💾 {{ store.stats.size_mb.toFixed(1) }} MB</span>
          <span v-if="store.stats.duplicate_count > 0" class="cap-item warn" :title="'发现 ' + store.stats.duplicate_count + ' 条重复记忆'">⚠️ 重复 {{ store.stats.duplicate_count }}</span>
        </div>
        <!-- 分类标签 -->
        <div class="cat-bar">
          <button class="cat-chip" :class="{ on: store.categoryFilter === '' }" @click="store.setCategory('')">全部</button>
          <button
            v-for="c in store.stats?.categories.slice(0, 6) || []"
            :key="c.name"
            class="cat-chip"
            :class="{ on: store.categoryFilter === c.name }"
            @click="store.setCategory(store.categoryFilter === c.name ? '' : c.name)"
          >
            {{ c.name }}({{ c.count }})
          </button>
        </div>
        <!-- 批量操作条 -->
        <div v-if="store.selectedIds.size > 0" class="batch-bar">
          <span class="batch-count">已选 {{ store.selectedIds.size }} 条</span>
          <button class="batch-btn" @click="store.markSelectedImportant(true)">⭐ 标重要</button>
          <button class="batch-btn" @click="store.markSelectedImportant(false)">取消重要</button>
          <button class="batch-btn danger" @click="batchDelete">🗑 删除</button>
          <button class="batch-btn" @click="store.selectedIds = new Set()">取消</button>
        </div>
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
            :class="{ expanded: isExpanded(m.id), selected: store.selectedIds.has(m.id) }"
          >
            <div class="mem-row" @click="toggleExpand(m.id)">
              <input
                type="checkbox"
                class="mem-check"
                :checked="store.selectedIds.has(m.id)"
                @click.stop="store.toggleSelect(m.id)"
                title="选择（批量操作）"
              />
              <span class="mem-time">{{ timeOf(m) }}</span>
              <span class="mem-role" :class="m.role">{{ roleLabel(m) }}</span>
              <span v-if="m.category" class="mem-cat">{{ m.category }}</span>
              <span v-if="(m.use_count || 0) > 0" class="mem-uses" title="铃想起它的次数">×{{ m.use_count }}</span>
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
                <button class="act-btn" title="编辑" @click="onEdit(m)">✏️</button>
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
  font-size: var(--fs-16);
  line-height: 1;
}
.panel-title {
  font-size: var(--fs-14);
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
  font-size: var(--fs-10);
}
.set-bar {
  display: flex;
  gap: 6px;
  padding: 8px 12px 0;
}
/* —— 记忆透明页（P1 重点）样式：毛玻璃背景板 —— */
.transparency {
  margin: 8px 10px 4px;
  padding: 10px 12px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.25);
  backdrop-filter: blur(14px) saturate(1.4);
  -webkit-backdrop-filter: blur(14px) saturate(1.4);
  border: 1px solid rgba(255, 255, 255, 0.35);
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.06), inset 0 1px 0 rgba(255, 255, 255, 0.3);
}
/* 暗色主题：毛玻璃底色加深 */
.app-root.dark .transparency,
.app-root.ios-glass .transparency {
  background: rgba(30, 30, 40, 0.35);
  border-color: rgba(255, 255, 255, 0.12);
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.25), inset 0 1px 0 rgba(255, 255, 255, 0.08);
}
.trans-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
  user-select: none;
  margin-bottom: 6px;
}
.trans-title {
  font-size: var(--fs-13);
  font-weight: 700;
  color: var(--accent, #ff7a94);
}
.trans-toggle { font-size: var(--fs-10); opacity: 0.6; }
.trans-stats {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 4px 8px;
  margin-bottom: 8px;
}
.stat {
  display: flex;
  align-items: baseline;
  gap: 4px;
  font-size: var(--fs-10);
  color: var(--text-secondary);
}
.stat b { font-size: var(--fs-14); color: var(--text-main); }
.important-box {
  border-top: 1px dashed var(--border, rgba(128, 128, 128, 0.25));
  padding-top: 6px;
}
.important-label {
  font-size: var(--fs-10);
  color: var(--text-secondary);
  margin-bottom: 4px;
}
.important-chip {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  margin-bottom: 4px;
  border-radius: 10px;
  background: rgba(255, 122, 148, 0.12);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  border: 1px solid rgba(255, 122, 148, 0.25);
  font-size: var(--fs-11);
}
.imp-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-main);
}
.imp-role {
  font-size: var(--fs-10);
  color: var(--text-secondary);
  flex-shrink: 0;
}
.imp-unmark {
  border: none;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  font-size: var(--fs-10);
  padding: 0 2px;
  flex-shrink: 0;
}
.imp-unmark:hover { color: var(--danger); }
.important-more { font-size: var(--fs-10); color: var(--text-secondary); padding-top: 2px; }
.important-empty {
  font-size: var(--fs-10);
  color: var(--text-secondary);
  border-top: 1px dashed var(--border, rgba(128, 128, 128, 0.25));
  padding-top: 6px;
  line-height: 1.5;
}
.privacy-box {
  margin-top: 6px;
  padding: 8px;
  border-radius: 8px;
  background: rgba(128, 128, 128, 0.08);
  font-size: var(--fs-10);
  color: var(--text-secondary);
  line-height: 1.7;
}
.privacy-box p { margin: 0 0 4px; }
.privacy-box p:last-child { margin-bottom: 0; }
.privacy-hint {
  margin-top: 6px;
  font-size: var(--fs-10);
  color: var(--success, #4caf50);
  text-align: center;
}
/* —— 记忆中心（大项目）样式 —— */
.center-bar { padding: 0 10px 4px; }
.cap-row {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
  font-size: var(--fs-11);
  color: var(--text-secondary);
  padding: 2px 2px 6px;
}
.cap-item.warn { color: var(--warning, #f0ad4e); }
.cat-bar { display: flex; gap: 4px; flex-wrap: wrap; margin-bottom: 6px; }
.cat-chip {
  font-size: var(--fs-10);
  padding: 2px 8px;
  border-radius: 10px;
  border: 1px solid var(--border, rgba(128, 128, 128, 0.3));
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.15s;
}
.cat-chip.on { background: var(--accent, #ff7a94); border-color: var(--accent, #ff7a94); color: #fff; }
.cat-chip:hover { border-color: var(--accent, #ff7a94); }
.batch-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  padding: 6px 8px;
  border-radius: 8px;
  background: rgba(255, 122, 148, 0.08);
  border: 1px solid rgba(255, 122, 148, 0.25);
  margin-bottom: 6px;
}
.batch-count { font-size: var(--fs-11); color: var(--accent, #ff7a94); font-weight: 600; }
.batch-btn {
  font-size: var(--fs-10);
  padding: 2px 8px;
  border-radius: 8px;
  border: 1px solid var(--border, rgba(128, 128, 128, 0.3));
  background: transparent;
  color: var(--text-main);
  cursor: pointer;
}
.batch-btn:hover { border-color: var(--accent, #ff7a94); }
.batch-btn.danger { color: var(--danger, #d9534f); }
.batch-btn.danger:hover { border-color: var(--danger, #d9534f); }
.mem-check { accent-color: var(--accent, #ff7a94); cursor: pointer; flex-shrink: 0; }
.mem-item.selected { background: rgba(255, 122, 148, 0.08); border-radius: 8px; }
.mem-cat {
  font-size: var(--fs-9);
  padding: 0 5px;
  border-radius: 6px;
  background: rgba(128, 128, 128, 0.15);
  color: var(--text-secondary);
  flex-shrink: 0;
  white-space: nowrap;
}
.mem-uses {
  font-size: var(--fs-9);
  color: var(--info, #1a56a8);
  flex-shrink: 0;
}
.set-select {
  flex: 1;
  min-width: 0;
  font-size: var(--fs-12);
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
  font-size: var(--fs-16);
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
  font-size: var(--fs-12);
  padding: 4px 6px;
  border-radius: 6px;
  border: 1px solid var(--border, rgba(128, 128, 128, 0.3));
  background: var(--input-bg, #fff);
  color: var(--text-main, #222);
}
.confirm-btn {
  font-size: var(--fs-12);
  padding: 2px 8px;
  border: none;
  border-radius: 6px;
  background: var(--accent, #ffb6c1);
  color: var(--text-user);
  cursor: pointer;
}
.search-box {
  padding: 8px 12px 0;
}
.search-input {
  width: 100%;
  box-sizing: border-box;
  font-size: var(--fs-12);
  padding: 5px 8px;
  border-radius: 8px;
  border: 1px solid var(--border, rgba(128, 128, 128, 0.3));
  background: var(--input-bg, #fff);
  color: var(--text-main, #222);
}
.error-tip {
  margin: 8px 12px 0;
  font-size: var(--fs-11);
  color: var(--danger, #e53935);
}
.timeline {
  flex: 1;
  overflow-y: auto;
  padding: 8px 10px;
}
.loading,
.empty {
  font-size: var(--fs-12);
  color: var(--text-secondary, #888);
  text-align: center;
  padding: 16px 0;
}
.day-group {
  margin-bottom: 10px;
}
.day-label {
  font-size: var(--fs-11);
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
  font-size: var(--fs-12);
}
.mem-time {
  color: var(--text-secondary, #888);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}
.mem-role {
  flex-shrink: 0;
  font-size: var(--fs-11);
  padding: 1px 5px;
  border-radius: 4px;
  color: var(--text-user);
}
.mem-role.user {
  background: var(--info);
}
.mem-role.assistant {
  background: var(--accent);
}
.mem-preview {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-main, #222);
}
.mem-full {
  margin-top: 4px;
  font-size: var(--fs-12);
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
  font-size: var(--fs-14);
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
