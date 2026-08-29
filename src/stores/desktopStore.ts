// 《铃·记忆体》桌面交互 Store（AI-6）
// 管理：悬浮球可见性、工具箱条目、屏幕监测规则与状态
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { ScreenMonitorRule, ToolboxItem } from '../types'
import {
  deleteMonitorRule,
  deleteToolboxItem,
  executeToolbox,
  getMonitorRules,
  listToolboxItems,
  saveToolboxItem,
  setFloatingBallVisibility as invokeSetFloatingBallVisibility,
  toggleMonitoring as invokeToggleMonitoring,
  updateMonitorRule as invokeUpdateMonitorRule,
} from '../utils/tauri'

export const useDesktopStore = defineStore('desktop', () => {
  // —— 悬浮球 ——
  const floatingBallVisible = ref(false)

  // —— 工具箱 ——
  const toolboxItems = ref<ToolboxItem[]>([])
  const toolboxLoading = ref(false)

  // —— 屏幕监测 ——
  const monitorRules = ref<ScreenMonitorRule[]>([])
  const isMonitoring = ref(false)
  const monitorInterval = ref(3)
  const monitoringAvailable = ref(true)
  const monitoringLoading = ref(false)

  // ==================== 悬浮球 ====================

  async function setFloatingBallVisibility(visible: boolean): Promise<void> {
    floatingBallVisible.value = visible
    await invokeSetFloatingBallVisibility(visible)
  }

  // ==================== 工具箱 ====================

  /** 加载工具箱条目（预设 + 用户自定义） */
  async function loadToolboxItems(): Promise<void> {
    toolboxLoading.value = true
    try {
      const res = await listToolboxItems()
      toolboxItems.value = res.items
    } catch (e) {
      console.error('加载工具箱失败：', e)
    } finally {
      toolboxLoading.value = false
    }
  }

  /** 执行工具箱命令，返回执行结果（null 表示失败） */
  async function executeToolboxItem(id: string, input?: string): Promise<{ output?: string; error?: string } | null> {
    try {
      const res = await executeToolbox(id, input)
      if (!res.success) {
        return { error: res.error || '命令执行失败' }
      }
      return { output: res.output }
    } catch (e) {
      return { error: String(e) }
    }
  }

  /** 新增/更新用户自定义条目 */
  async function addOrUpdateToolboxItem(item: ToolboxItem): Promise<void> {
    await saveToolboxItem(item)
    await loadToolboxItems()
  }

  /** 删除用户自定义条目 */
  async function removeToolboxItem(id: string): Promise<void> {
    await deleteToolboxItem(id)
    await loadToolboxItems()
  }

  // ==================== 屏幕监测 ====================

  /** 加载监测状态 + 规则列表 */
  async function loadMonitorRules(): Promise<void> {
    monitoringLoading.value = true
    try {
      const res = await getMonitorRules()
      monitorRules.value = res.rules
      isMonitoring.value = res.enabled
      monitorInterval.value = res.interval_seconds
      monitoringAvailable.value = res.available
    } catch (e) {
      console.error('加载监测规则失败：', e)
    } finally {
      monitoringLoading.value = false
    }
  }

  /** 更新（或新增）单条规则 */
  async function updateMonitorRule(rule: ScreenMonitorRule): Promise<void> {
    await invokeUpdateMonitorRule(rule)
    await loadMonitorRules()
  }

  /** 删除单条规则 */
  async function removeMonitorRule(id: string): Promise<void> {
    await deleteMonitorRule(id)
    await loadMonitorRules()
  }

  /** 启用/禁用监测（可附带新频率），返回最终是否启用 */
  async function toggleMonitoring(enabled: boolean, intervalSeconds?: number): Promise<boolean> {
    const finalEnabled = await invokeToggleMonitoring(enabled, intervalSeconds)
    isMonitoring.value = finalEnabled
    if (intervalSeconds) monitorInterval.value = intervalSeconds
    return finalEnabled
  }

  return {
    // 悬浮球
    floatingBallVisible,
    setFloatingBallVisibility,
    // 工具箱
    toolboxItems,
    toolboxLoading,
    loadToolboxItems,
    executeToolboxItem,
    addOrUpdateToolboxItem,
    removeToolboxItem,
    // 屏幕监测
    monitorRules,
    isMonitoring,
    monitorInterval,
    monitoringAvailable,
    monitoringLoading,
    loadMonitorRules,
    updateMonitorRule,
    removeMonitorRule,
    toggleMonitoring,
  }
})
