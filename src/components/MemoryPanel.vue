<!-- 《铃·记忆体》记忆面板（抽屉内嵌版）
     标题与关闭由主窗口抽屉的标签条负责，这里只负责内容：
     记忆库切换 / 新建库 / 概览与时间范围 / 重要性筛选 / 搜索 / 日期分组列表 / 批量操作 -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useMemoryStore } from '../stores/memoryStore'
import { useMilestoneStore } from '../stores/milestoneStore'
import type { Memory } from '../types'
import TheIcon from './TheIcon.vue'
import AppButton from './ui/AppButton.vue'
import AppCard from './ui/AppCard.vue'
import AppEmpty from './ui/AppEmpty.vue'
import AppInput from './ui/AppInput.vue'
import AppStatusDot from './ui/AppStatusDot.vue'
import { confirmDialog } from '../utils/dialog'

const store = useMemoryStore()
const milestone = useMilestoneStore()

// 新建记忆库输入框
const showCreateInput = ref(false)
const newSetName = ref('')

// 时间范围筛选（本地过滤，不改后端查询）
type RangeId = 'all' | '7' | '30'
const range = ref<RangeId>('all')
// 只看重要
const importantOnly = ref(false)
// 隐私说明折叠
const showPrivacy = ref(false)

// 日期工具：按本地时区算截止日，和记忆 timestamp 前 10 位直接比较
function pad2(n: number) {
  return String(n).padStart(2, '0')
}
function dateStr(d: Date) {
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`
}
function cutoffDate(days: number) {
  const d = new Date()
  d.setDate(d.getDate() - (days - 1))
  return dateStr(d)
}
function inRange(m: Memory) {
  if (range.value === 'all') return true
  const day = (m.timestamp || '').slice(0, 10)
  return day >= cutoffDate(Number(range.value))
}

// 时间范围卡片（数量按当前搜索/分类结果统计）
const rangeCards = computed(() => {
  const list = store.filteredMemories
  const countSince = (min: string) =>
    list.filter((m) => (m.timestamp || '').slice(0, 10) >= min).length
  return [
    { id: 'all' as RangeId, label: '全部', count: list.length },
    { id: '7' as RangeId, label: '近 7 天', count: countSince(cutoffDate(7)) },
    { id: '30' as RangeId, label: '近 30 天', count: countSince(cutoffDate(30)) },
  ]
})

// 当前可见的记忆（时间范围 + 只看重要，叠加在分类/搜索之上）
const visibleMemories = computed(() =>
  store.filteredMemories.filter(
    (m) => inRange(m) && (!importantOnly.value || isImportant(m)),
  ),
)

// 按日期分组（timestamp 前 10 位为日期）
interface DayGroup {
  date: string
  items: Memory[]
}
const groups = computed<DayGroup[]>(() => {
  const map = new Map<string, Memory[]>()
  for (const m of visibleMemories.value) {
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

// —— 概览：重要记忆 / 最早 / 最近 / 容量 ——
function isImportant(m: Memory) {
  return !!m.tags?.includes('important')
}
const importantMemories = computed(() => store.memories.filter((m) => isImportant(m)))
const earliest = computed(() => {
  const t = store.memories.map((m) => m.timestamp || '').filter(Boolean).sort()
  return t.length ? t[0].slice(0, 10) : '—'
})
const latest = computed(() => {
  const t = store.memories.map((m) => m.timestamp || '').filter(Boolean).sort()
  return t.length ? t[t.length - 1].slice(0, 10) : '—'
})
const duplicateCount = computed(() => store.stats?.duplicate_count ?? 0)
const capText = computed(() =>
  store.stats
    ? `共 ${store.stats.total} 条 · ${store.stats.size_mb.toFixed(1)} MB`
    : '',
)
// 空态文案：搜索无结果和完全没记忆区分开
const emptyTitle = computed(() =>
  store.searchKeyword.trim() ? '没有找到匹配的记忆' : '还没有记忆',
)
const emptyHint = computed(() =>
  store.searchKeyword.trim()
    ? '换个关键词再试试'
    : '和铃聊天时产生的长期记忆会出现在这里',
)

// 删除（带确认）
async function onDelete(m: Memory) {
  const ok = await confirmDialog(`确定删除这条记忆吗？\n${summaryOf(m)}`, {
    danger: true,
    confirmText: '删除',
  })
  if (ok) store.deleteMemory(m.id)
}
// 编辑（改内容后后端重新分类）
function onEdit(m: Memory) {
  const newContent = prompt('编辑这条记忆：', m.content)
  if (newContent !== null && newContent.trim() && newContent !== m.content) {
    store.editMemory(m.id, newContent.trim())
  }
}
// 批量删除（带确认）
async function batchDelete() {
  const n = store.selectedIds.size
  if (n === 0) return
  const ok = await confirmDialog(`确定删除选中的 ${n} 条记忆吗？`, {
    danger: true,
    confirmText: '删除',
  })
  if (ok) store.deleteSelected()
}
// 标记重要（后端幂等，重复点击不会出错）
function onMark(m: Memory) {
  store.markImportant(m.id)
}
function clearSelection() {
  store.selectedIds = new Set()
}

// 搜索（防抖 300ms）
let searchTimer: ReturnType<typeof setTimeout> | undefined
function onSearchInput() {
  clearTimeout(searchTimer)
  searchTimer = setTimeout(() => store.search(store.searchKeyword), 300)
}

// 切换记忆库
function onSwitchSet(e: Event) {
  const name = (e.target as HTMLSelectElement).value
  if (name !== store.currentSet) store.switchSet(name)
}

// 新建记忆库
async function onCreateSet() {
  const name = newSetName.value.trim()
  if (!name) return
  await store.createSet(name)
  newSetName.value = ''
  showCreateInput.value = false
  // 第一次创建记忆集的里程碑（幂等）
  milestone.record('first_memory', '创建了第一个记忆集').catch(() => {})
}

function toggleCreate() {
  showCreateInput.value = !showCreateInput.value
  if (!showCreateInput.value) newSetName.value = ''
}

onMounted(() => {
  store.loadSets()
  store.loadMemories()
  store.loadStats()
})
</script>

<template>
  <section class="memory-panel">
    <!-- 轻量操作条：记忆库下拉 + 新建 -->
    <div class="bar">
      <TheIcon name="memory" :size="14" class="bar-ico" />
      <select
        class="set-select"
        :value="store.currentSet"
        aria-label="记忆库"
        @change="onSwitchSet"
      >
        <option v-for="s in store.sets" :key="s" :value="s">{{ s }}</option>
      </select>
      <AppButton variant="ghost" size="md" @click="toggleCreate">
        <TheIcon name="add" :size="14" />
        {{ showCreateInput ? '取消' : '新建库' }}
      </AppButton>
    </div>
    <div v-if="showCreateInput" class="create-row">
      <AppInput v-model="newSetName" placeholder="记忆库名称" @keyup.enter="onCreateSet" />
      <AppButton
        variant="primary"
        size="md"
        :disabled="!newSetName.trim()"
        @click="onCreateSet"
      >
        创建
      </AppButton>
    </div>

    <div class="scroll">
      <!-- 概览：铃记住了什么 + 时间范围卡片 + 隐私说明 -->
      <AppCard>
        <div class="ov-head">
          <span class="ov-title">铃记住了</span>
          <button class="link" type="button" @click="showPrivacy = !showPrivacy">
            {{ showPrivacy ? '收起隐私说明' : '隐私说明' }}
          </button>
        </div>

        <div class="stats">
          <div class="stat">
            <span class="stat-num">{{ store.memories.length }}</span>
            <span class="stat-label">条记忆</span>
          </div>
          <div class="stat">
            <span class="stat-num">{{ importantMemories.length }}</span>
            <span class="stat-label">条重要</span>
          </div>
          <div class="stat">
            <span class="stat-num date">{{ earliest }}</span>
            <span class="stat-label">最早</span>
          </div>
          <div class="stat">
            <span class="stat-num date">{{ latest }}</span>
            <span class="stat-label">最近</span>
          </div>
        </div>

        <div class="ranges">
          <button
            v-for="r in rangeCards"
            :key="r.id"
            class="range-card"
            type="button"
            :class="{ on: range === r.id }"
            @click="range = r.id"
          >
            <span class="range-num">{{ r.count }}</span>
            <span class="range-label">{{ r.label }}</span>
          </button>
        </div>

        <div v-if="showPrivacy" class="privacy">
          <p>
            <TheIcon name="lock" :size="12" class="privacy-ico" />
            记忆只保存在本机（你的数据目录），不会上传到任何地方。
          </p>
          <p>设置主密码后，API 密钥等敏感信息会加密存储。</p>
          <p>你可以随时在下方列表里查看、删除或标记重要。</p>
        </div>

        <div class="ov-foot">
          <span v-if="capText" class="cap">{{ capText }}</span>
          <span v-if="duplicateCount > 0" class="cap warn">重复 {{ duplicateCount }} 条</span>
          <AppStatusDot state="ok" text="本机存储 · 不上传" />
        </div>
      </AppCard>

      <!-- 搜索 -->
      <div class="search">
        <TheIcon name="search" :size="14" class="search-ico" />
        <AppInput
          v-model="store.searchKeyword"
          class="search-input"
          placeholder="搜索记忆"
          @input="onSearchInput"
        />
      </div>

      <!-- 筛选：重要性 + 分类 -->
      <div class="filters">
        <div class="filter-row">
          <button
            class="chip"
            type="button"
            :class="{ on: importantOnly }"
            @click="importantOnly = !importantOnly"
          >
            <TheIcon name="sponsor" :size="12" />
            只看重要
          </button>
          <span class="filter-count">当前 {{ visibleMemories.length }} 条</span>
        </div>
        <div class="cats">
          <button
            class="chip"
            type="button"
            :class="{ on: store.categoryFilter === '' }"
            @click="store.setCategory('')"
          >
            全部
          </button>
          <button
            v-for="c in store.stats?.categories.slice(0, 6) || []"
            :key="c.name"
            class="chip"
            type="button"
            :class="{ on: store.categoryFilter === c.name }"
            @click="store.setCategory(store.categoryFilter === c.name ? '' : c.name)"
          >
            {{ c.name }}<span class="chip-count">{{ c.count }}</span>
          </button>
        </div>
      </div>

      <!-- 批量操作 -->
      <div v-if="store.selectedIds.size > 0" class="batch">
        <span class="batch-count">已选 {{ store.selectedIds.size }} 条</span>
        <button class="chip" type="button" @click="store.markSelectedImportant(true)">标重要</button>
        <button class="chip" type="button" @click="store.markSelectedImportant(false)">取消重要</button>
        <button class="chip danger" type="button" @click="batchDelete">删除</button>
        <button class="chip" type="button" @click="clearSelection">取消</button>
      </div>

      <div v-if="store.errorMsg" class="error">{{ store.errorMsg }}</div>

      <!-- 时间线 -->
      <div v-if="store.isLoading" class="hint">加载中…</div>
      <AppEmpty
        v-else-if="store.memories.length === 0"
        :text="emptyTitle"
        :hint="emptyHint"
      />
      <AppEmpty
        v-else-if="groups.length === 0"
        text="没有符合条件的记忆"
        hint="换个时间范围或筛选条件看看"
      />
      <div v-else class="timeline">
        <div v-for="g in groups" :key="g.date" class="day">
          <div class="day-label">{{ g.date }}</div>
          <div
            v-for="m in g.items"
            :key="m.id"
            class="mem"
            :class="{ open: isExpanded(m.id), sel: store.selectedIds.has(m.id) }"
          >
            <div
              class="mem-row"
              role="button"
              tabindex="0"
              :aria-expanded="isExpanded(m.id)"
              @click="toggleExpand(m.id)"
              @keydown.enter.prevent="toggleExpand(m.id)"
            >
              <input
                class="check"
                type="checkbox"
                :checked="store.selectedIds.has(m.id)"
                title="选择（批量操作）"
                @click.stop="store.toggleSelect(m.id)"
                @keydown.space.stop
              />
              <span class="time">{{ timeOf(m) }}</span>
              <span class="role" :class="m.role">{{ roleLabel(m) }}</span>
              <span class="preview">{{ summaryOf(m) }}</span>
              <TheIcon
                v-if="isImportant(m)"
                name="sponsor"
                :size="12"
                class="heart"
                title="已标记重要"
              />
            </div>
            <div v-if="isExpanded(m.id)" class="mem-full">
              <p class="content">{{ m.content }}</p>
              <div class="mem-meta">
                <span v-if="m.category" class="tag">{{ m.category }}</span>
                <span v-if="(m.use_count || 0) > 0" class="uses">想起 {{ m.use_count }} 次</span>
              </div>
              <div class="mem-acts">
                <button
                  v-if="!isImportant(m)"
                  class="icon-act"
                  type="button"
                  title="标记重要"
                  aria-label="标记重要"
                  @click="onMark(m)"
                >
                  <TheIcon name="sponsor" :size="14" />
                </button>
                <span v-else class="marked">
                  <TheIcon name="sponsor" :size="12" />
                  已标记重要
                </span>
                <button class="icon-act" type="button" title="编辑" aria-label="编辑" @click="onEdit(m)">
                  <TheIcon name="edit" :size="14" />
                </button>
                <button
                  class="icon-act danger"
                  type="button"
                  title="删除"
                  aria-label="删除"
                  @click="onDelete(m)"
                >
                  <TheIcon name="delete" :size="14" />
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
/* 流内布局：宽高由抽屉决定，面板自己不再有外框、固定宽度和定位 */
.memory-panel {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
  color: var(--ui-text);
  background: transparent;
  font-size: var(--ui-fs-13);
}

/* 顶部轻量操作条 */
.bar {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-bottom: 1px solid var(--ui-border);
}
.bar-ico {
  flex: none;
  color: var(--ui-text-3);
}
.set-select {
  flex: 1;
  min-width: 0;
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-2);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-sm);
  background: var(--ui-raised);
  color: var(--ui-text);
  font-size: var(--ui-fs-13);
}
.set-select option {
  background: var(--ui-raised);
  color: var(--ui-text);
}
.create-row {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  padding: 0 var(--ui-sp-3) var(--ui-sp-2);
}
.create-row :deep(.app-input) {
  flex: 1;
  min-width: 0;
}

/* 内容区自己滚动 */
.scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-3);
  padding: var(--ui-sp-3);
}
/* 滚动区里的块不参与压缩（否则长列表会把概览卡挤扁、内容被裁掉） */
.scroll > * {
  flex: none;
}

/* —— 概览卡片 —— */
.ov-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--ui-sp-2);
}
.ov-title {
  font-size: var(--ui-fs-13);
  font-weight: 600;
  color: var(--ui-text);
}
.link {
  border: 0;
  background: transparent;
  color: var(--ui-text-3);
  font-size: var(--ui-fs-11);
  padding: var(--ui-sp-1);
  border-radius: var(--ui-r-sm);
}
.link:hover {
  background: var(--ui-hover);
  color: var(--ui-text);
}
.stats {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--ui-sp-1) var(--ui-sp-3);
  margin-top: var(--ui-sp-2);
}
.stat {
  display: flex;
  align-items: baseline;
  gap: var(--ui-sp-1);
  min-width: 0;
}
.stat-num {
  font-size: var(--ui-fs-14);
  font-weight: 600;
  color: var(--ui-text);
  font-variant-numeric: tabular-nums;
}
.stat-num.date {
  font-size: var(--ui-fs-12);
  font-weight: 400;
  color: var(--ui-text-2);
}
.stat-label {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
}
.ranges {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--ui-sp-2);
  margin-top: var(--ui-sp-3);
}
.range-card {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
  padding: var(--ui-sp-2);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  background: transparent;
  color: var(--ui-text-2);
  text-align: left;
  transition: background var(--ui-dur-1) var(--ui-ease), border-color var(--ui-dur-1) var(--ui-ease);
}
.range-card:hover {
  background: var(--ui-hover);
  border-color: var(--ui-border-strong);
}
.range-card.on {
  background: var(--ui-accent-soft);
  border-color: var(--ui-accent-line);
}
.range-num {
  font-size: var(--ui-fs-14);
  font-weight: 600;
  color: var(--ui-text);
  font-variant-numeric: tabular-nums;
}
.range-label {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
}
.range-card.on .range-label {
  color: var(--ui-accent);
}
.privacy {
  margin-top: var(--ui-sp-3);
  padding: var(--ui-sp-2);
  border-radius: var(--ui-r-sm);
  background: var(--ui-hover);
  font-size: var(--ui-fs-11);
  color: var(--ui-text-2);
  line-height: var(--ui-line);
}
.privacy p {
  margin: 0 0 var(--ui-sp-1);
}
.privacy p:last-child {
  margin-bottom: 0;
}
.privacy-ico {
  margin-right: var(--ui-sp-1);
  color: var(--ui-text-3);
}
.ov-foot {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-1) var(--ui-sp-2);
  margin-top: var(--ui-sp-3);
  font-size: var(--ui-fs-11);
}
.cap {
  color: var(--ui-text-3);
}
.cap.warn {
  color: var(--ui-warn);
}

/* —— 搜索 —— */
.search {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
}
.search-ico {
  flex: none;
  color: var(--ui-text-3);
}
.search-input {
  flex: 1;
  min-width: 0;
}

/* —— 筛选 —— */
.filters {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-2);
}
.filter-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--ui-sp-2);
}
.filter-count {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  white-space: nowrap;
}
.cats {
  display: flex;
  flex-wrap: wrap;
  gap: var(--ui-sp-1);
}
.chip {
  display: inline-flex;
  align-items: center;
  gap: var(--ui-sp-1);
  padding: var(--ui-sp-1) var(--ui-sp-2);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-pill);
  background: transparent;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-11);
  line-height: var(--ui-line);
  transition: background var(--ui-dur-1) var(--ui-ease), border-color var(--ui-dur-1) var(--ui-ease),
    color var(--ui-dur-1) var(--ui-ease);
}
.chip:hover {
  border-color: var(--ui-border-strong);
  color: var(--ui-text);
}
.chip.on {
  background: var(--ui-accent-soft);
  border-color: var(--ui-accent-line);
  color: var(--ui-accent);
}
.chip.danger {
  color: var(--ui-err);
}
.chip-count {
  color: var(--ui-text-3);
  font-variant-numeric: tabular-nums;
}

/* —— 批量操作 —— */
.batch {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-1);
  padding: var(--ui-sp-2);
  border: 1px solid var(--ui-accent-line);
  border-radius: var(--ui-r-md);
  background: var(--ui-accent-soft);
}
.batch-count {
  margin-right: var(--ui-sp-1);
  font-size: var(--ui-fs-11);
  color: var(--ui-accent);
  font-weight: 600;
}

.error {
  font-size: var(--ui-fs-12);
  color: var(--ui-err);
  line-height: var(--ui-line);
  word-break: break-word;
}
.hint {
  padding: var(--ui-sp-4) 0;
  text-align: center;
  font-size: var(--ui-fs-12);
  color: var(--ui-text-3);
}

/* —— 时间线 —— */
.timeline {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-3);
}
.day {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
}
.day-label {
  padding-bottom: var(--ui-sp-1);
  border-bottom: 1px solid var(--ui-border);
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  font-variant-numeric: tabular-nums;
}
.mem {
  border-radius: var(--ui-r-sm);
}
.mem:hover {
  background: var(--ui-hover);
}
.mem.sel {
  background: var(--ui-accent-soft);
}
.mem-row {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-1);
  padding: var(--ui-sp-1) var(--ui-sp-2);
  border-radius: var(--ui-r-sm);
  cursor: pointer;
}
.check {
  flex: none;
  margin: 0;
  accent-color: var(--ui-accent);
  cursor: pointer;
}
.time {
  flex: none;
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  font-variant-numeric: tabular-nums;
}
.role {
  flex: none;
  padding: 0 var(--ui-sp-1);
  border-radius: var(--ui-r-sm);
  font-size: var(--ui-fs-11);
  line-height: var(--ui-line);
}
.role.user {
  background: var(--ui-active);
  color: var(--ui-text-2);
}
.role.assistant {
  background: var(--ui-accent-soft);
  color: var(--ui-accent);
}
.preview {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--ui-fs-12);
  color: var(--ui-text);
}
.heart {
  flex: none;
  color: var(--ui-accent);
}
.mem-full {
  margin: var(--ui-sp-1) var(--ui-sp-2) var(--ui-sp-1);
  padding: var(--ui-sp-2);
  border-radius: var(--ui-r-sm);
  background: var(--ui-hover);
}
.content {
  margin: 0;
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text);
  white-space: pre-wrap;
  word-break: break-word;
}
.mem-meta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-1);
  margin-top: var(--ui-sp-1);
}
.tag {
  padding: 0 var(--ui-sp-1);
  border-radius: var(--ui-r-sm);
  background: var(--ui-active);
  font-size: var(--ui-fs-11);
  color: var(--ui-text-2);
}
.uses {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
}
.mem-acts {
  display: flex;
  justify-content: flex-end;
  gap: var(--ui-sp-1);
  margin-top: var(--ui-sp-1);
}
.marked {
  display: inline-flex;
  align-items: center;
  gap: var(--ui-sp-1);
  margin-right: auto;
  font-size: var(--ui-fs-11);
  color: var(--ui-accent);
}
.icon-act {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: var(--ui-sp-1);
  border: 0;
  border-radius: var(--ui-r-sm);
  background: transparent;
  color: var(--ui-text-2);
  transition: background var(--ui-dur-1) var(--ui-ease), color var(--ui-dur-1) var(--ui-ease);
}
.icon-act:hover {
  background: var(--ui-active);
  color: var(--ui-text);
}
.icon-act.danger {
  color: var(--ui-err);
}
</style>
