// 《铃·记忆体》记忆 Store（useMemoryStore）完整实现（AI-4）
// 管理 memories 列表、currentSet（当前记忆集）、searchKeyword、sets（所有记忆集）
// 通过 tauri.ts 调用后端 IPC；后端未就绪时静默回退，不阻塞 UI
import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { Memory } from '../types'
import {
  createMemorySet as ipcCreateSet,
  deleteMemory as ipcDelete,
  deleteMemoriesBatch as ipcDeleteBatch,
  editMemoryContent as ipcEditMemory,
  getMemories as ipcGet,
  listMemorySets as ipcListSets,
  markImportantBatch as ipcMarkBatch,
  markMemoryImportant as ipcMark,
  memoryStats as ipcStats,
  switchMemorySet as ipcSwitch,
} from '../utils/tauri'

export const useMemoryStore = defineStore('memory', () => {
  // 记忆列表（当前显示的）
  const memories = ref<Memory[]>([])
  // 当前激活的记忆集名称
  const currentSet = ref<string>('default')
  // 搜索关键词
  const searchKeyword = ref<string>('')
  // 所有记忆集名称
  const sets = ref<string[]>(['default'])
  // 加载状态
  const isLoading = ref(false)
  // 错误信息（友好提示）
  const errorMsg = ref<string>('')

  /** 加载记忆列表（带搜索/分页） */
  async function loadMemories() {
    isLoading.value = true
    errorMsg.value = ''
    try {
      const resp = await ipcGet({
        keyword: searchKeyword.value || undefined,
        set_name: currentSet.value,
        limit: 200,
      })
      memories.value = resp.memories
    } catch (e) {
      errorMsg.value = String(e)
      memories.value = []
    } finally {
      isLoading.value = false
    }
  }

  /** 加载所有记忆集 */
  async function loadSets() {
    try {
      sets.value = await ipcListSets()
    } catch {
      sets.value = ['default']
    }
  }

  /** 删除单条记忆（后端确认后刷新） */
  async function deleteMemory(id: string) {
    try {
      await ipcDelete(id, currentSet.value)
      await loadMemories()
    } catch (e) {
      errorMsg.value = String(e)
    }
  }

  /** 切换记忆集 */
  async function switchSet(name: string) {
    try {
      await ipcSwitch(name)
      currentSet.value = name
      searchKeyword.value = ''
      await loadMemories()
    } catch (e) {
      errorMsg.value = String(e)
    }
  }

  /** 创建新记忆集并切换到它 */
  async function createSet(name: string) {
    try {
      await ipcCreateSet(name)
      await loadSets()
      await switchSet(name)
    } catch (e) {
      errorMsg.value = String(e)
    }
  }

  /** 标记记忆为重要（⭐） */
  async function markImportant(id: string) {
    try {
      await ipcMark(id, currentSet.value)
      await loadMemories()
    } catch (e) {
      errorMsg.value = String(e)
    }
  }

  /** 搜索记忆（实时过滤） */
  async function search(keyword: string) {
    searchKeyword.value = keyword
    await loadMemories()
  }

  // —— 记忆中心（大项目）：统计 / 分类筛选 / 批量操作 ——
  const stats = ref<{ total: number; size_mb: number; important_count: number; duplicate_count: number; categories: { name: string; count: number }[] } | null>(null)
  const categoryFilter = ref<string>('')
  const selectedIds = ref<Set<string>>(new Set())

  /** 加载记忆中心统计 */
  async function loadStats() {
    try {
      stats.value = await ipcStats(currentSet.value)
    } catch {
      /* 静默 */
    }
  }

  /** 按分类过滤（空=全部） */
  function setCategory(cat: string) {
    categoryFilter.value = cat
  }

  /** 过滤后的记忆列表（按分类） */
  const filteredMemories = computed(() => {
    if (!categoryFilter.value) return memories.value
    return memories.value.filter((m) => (m.category || '日常对话') === categoryFilter.value)
  })

  /** 切换勾选 */
  function toggleSelect(id: string) {
    const s = new Set(selectedIds.value)
    if (s.has(id)) s.delete(id)
    else s.add(id)
    selectedIds.value = s
  }

  /** 全选/取消全选当前列表 */
  function toggleSelectAll() {
    const list = filteredMemories.value
    if (selectedIds.value.size === list.length && list.length > 0) {
      selectedIds.value = new Set()
    } else {
      selectedIds.value = new Set(list.map((m) => m.id))
    }
  }

  /** 批量删除 */
  async function deleteSelected() {
    const ids = [...selectedIds.value]
    if (ids.length === 0) return
    try {
      await ipcDeleteBatch(ids, currentSet.value)
      selectedIds.value = new Set()
      await loadMemories()
      await loadStats()
    } catch (e) {
      errorMsg.value = String(e)
    }
  }

  /** 批量标记重要 */
  async function markSelectedImportant(important: boolean) {
    const ids = [...selectedIds.value]
    if (ids.length === 0) return
    try {
      await ipcMarkBatch(ids, important, currentSet.value)
      selectedIds.value = new Set()
      await loadMemories()
      await loadStats()
    } catch (e) {
      errorMsg.value = String(e)
    }
  }

  /** 编辑记忆内容 */
  async function editMemory(id: string, content: string) {
    try {
      await ipcEditMemory(id, content, currentSet.value)
      await loadMemories()
    } catch (e) {
      errorMsg.value = String(e)
    }
  }

  // 基础 action：添加一条记忆（本地插入，AI-3 对话后也可调用）
  function addMemory(mem: Memory) {
    memories.value.push(mem)
  }

  return {
    memories,
    currentSet,
    searchKeyword,
    sets,
    isLoading,
    errorMsg,
    loadMemories,
    loadSets,
    deleteMemory,
    switchSet,
    createSet,
    markImportant,
    search,
    addMemory,
    // 记忆中心
    stats,
    categoryFilter,
    selectedIds,
    loadStats,
    setCategory,
    filteredMemories,
    toggleSelect,
    toggleSelectAll,
    deleteSelected,
    markSelectedImportant,
    editMemory,
  }
})
