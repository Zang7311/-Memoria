// 《铃·记忆体》记忆 Store（useMemoryStore）完整实现（AI-4）
// 管理 memories 列表、currentSet（当前记忆集）、searchKeyword、sets（所有记忆集）
// 通过 tauri.ts 调用后端 IPC；后端未就绪时静默回退，不阻塞 UI
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { Memory } from '../types'
import {
  createMemorySet as ipcCreateSet,
  deleteMemory as ipcDelete,
  getMemories as ipcGet,
  listMemorySets as ipcListSets,
  markMemoryImportant as ipcMark,
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
  }
})
