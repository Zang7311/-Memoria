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

  /** 执行工具箱命令，返回执行结果（null 表示失败）
   *  危险自定义命令（后端返回 needs_confirm）会弹确认框，确认后带 confirm=true 重试 */
  async function executeToolboxItem(id: string, input?: string): Promise<{ output?: string; error?: string } | null> {
    const DANGER_CONFIRM_TEXT = '⚠️ 该自定义命令包含危险操作（可能影响系统或无法恢复），确定要继续执行吗？'
    try {
      let res = await executeToolbox(id, input)
      if (!res.success && String(res.error || '').includes('needs_confirm')) {
        if (!confirm(DANGER_CONFIRM_TEXT)) return { error: '已取消执行（危险操作未确认）' }
        res = await executeToolbox(id, input, true)
      }
      if (!res.success) {
        return { error: res.error || '命令执行失败' }
      }
      return { output: res.output }
    } catch (e) {
      const msg = String(e)
      if (msg.includes('needs_confirm')) {
        if (!confirm(DANGER_CONFIRM_TEXT)) return { error: '已取消执行（危险操作未确认）' }
        try {
          const res2 = await executeToolbox(id, input, true)
          return res2.success ? { output: res2.output } : { error: res2.error || '命令执行失败' }
        } catch (e2) {
          return { error: String(e2) }
        }
      }
      return { error: msg }
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
