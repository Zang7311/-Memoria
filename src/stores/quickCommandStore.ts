// 《铃·记忆体》快捷指令 Store（AI-9）
// 管理：快捷指令列表的加载/保存/删除，以及执行（供设置页与聊天触发共用）
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { ExecuteQuickCommandResponse, QuickCommand } from '../types'
import {
  deleteQuickCommand,
  executeQuickCommand,
  listQuickCommands,
  saveQuickCommand,
} from '../utils/tauri'

export const useQuickCommandStore = defineStore('quickCommand', () => {
  const commands = ref<QuickCommand[]>([])
  const loading = ref(false)
  const lastResult = ref<ExecuteQuickCommandResponse | null>(null)

  /** 加载快捷指令列表 */
  async function load(): Promise<void> {
    loading.value = true
    try {
      const res = await listQuickCommands()
      commands.value = res.commands
    } catch (e) {
      console.error('加载快捷指令失败：', e)
    } finally {
      loading.value = false
    }
  }

  /** 新增/更新一条指令 */
  async function save(cmd: QuickCommand): Promise<void> {
    await saveQuickCommand(cmd)
    await load()
  }

  /** 删除一条指令 */
  async function remove(id: string): Promise<void> {
    await deleteQuickCommand(id)
    await load()
  }

  /** 执行一条指令，返回执行结果 */
  async function execute(id: string): Promise<ExecuteQuickCommandResponse | null> {
    try {
      const res = await executeQuickCommand(id)
      lastResult.value = res
      return res
    } catch (e) {
      console.error('执行快捷指令失败：', e)
      return null
    }
  }

  return { commands, loading, lastResult, load, save, remove, execute }
})
