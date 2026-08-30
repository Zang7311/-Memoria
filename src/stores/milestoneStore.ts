// 《铃·记忆体》陪伴记录 Store（P3：与铃的日记）
// 每日日记（持续累积）+ 里程碑（纪念章）
// 纯本地，非游戏化——日记是流水账，不是成就列表
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { getMilestones, recordDailyChat, recordDailyTool, recordMilestone } from '../utils/tauri'

export interface MilestoneItem {
  key: string
  label: string
  date: string
}

export interface DailyEntry {
  date: string
  chat_count: number
  tool_count: number
  topics: string[]
}

export const useMilestoneStore = defineStore('milestone', () => {
  const firstDate = ref<string | null>(null)
  const days = ref(0)
  const items = ref<MilestoneItem[]>([])
  const daily = ref<DailyEntry[]>([])
  const loaded = ref(false)

  /** 加载陪伴记录 */
  async function load() {
    try {
      const res = await getMilestones()
      firstDate.value = res.first_date
      days.value = res.days
      items.value = res.items
      daily.value = res.daily
      loaded.value = true
    } catch {
      loaded.value = true
    }
  }

  /** 内部刷新（记录后同步最新数据） */
  async function refresh() {
    try {
      const res = await getMilestones()
      firstDate.value = res.first_date
      days.value = res.days
      items.value = res.items
      daily.value = res.daily
    } catch {
      /* 静默 */
    }
  }

  /** 记录里程碑（幂等：后端同一 key 只记一次） */
  async function record(key: string, label: string) {
    try {
      await recordMilestone(key, label)
      await refresh()
    } catch {
      /* 静默失败，不影响主流程 */
    }
  }

  /** 记录一次聊天（当日累积） */
  async function recordChat(text: string) {
    try {
      await recordDailyChat(text)
    } catch {
      /* 静默 */
    }
  }

  /** 记录一次工具箱使用 */
  async function recordTool(name: string) {
    try {
      await recordDailyTool(name)
    } catch {
      /* 静默 */
    }
  }

  /** 某个里程碑是否已完成 */
  function has(key: string): boolean {
    return items.value.some((m) => m.key === key)
  }

  return { firstDate, days, items, daily, loaded, load, record, recordChat, recordTool, has }
})
