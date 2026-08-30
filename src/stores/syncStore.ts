// 《铃·记忆体》同步 Store（AI-8 4.4）
// 设备发现 / 手动连接 / 同步 / 更新检查 / 网络状态
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { NetworkStatus, SyncDevice, SyncHistoryEntry } from '../types'
import {
  addManualDevice,
  checkUpdate,
  discoverDevices,
  getNetworkStatus,
  getSyncDevices,
  getSyncStatus,
  onNetworkStatusChanged,
  onSyncProgress,
  setConflictPolicy,
  startSync,
} from '../utils/tauri'
import { useMilestoneStore } from './milestoneStore'

export const useSyncStore = defineStore('sync', () => {
  /** 发现的设备列表 */
  const devices = ref<SyncDevice[]>([])
  /** 同步状态 */
  const syncStatus = ref<'idle' | 'discovering' | 'syncing' | 'done' | 'error'>('idle')
  /** 同步进度 0~1 */
  const syncProgress = ref(0)
  /** 连接模式：auto 自动发现 / manual 手动连接 */
  const connectionMode = ref<'auto' | 'manual'>('auto')
  /** 手动 IP / 端口 */
  const manualIp = ref('')
  const manualPort = ref(54546)
  /** 同步历史 */
  const history = ref<SyncHistoryEntry[]>([])
  /** 网络状态 */
  const networkStatus = ref<NetworkStatus>('unknown')
  /** 当前提示信息 */
  const message = ref('')
  /** 冲突策略 */
  const conflictPolicy = ref<'newest' | 'local' | 'remote'>('newest')
  /** 可用记忆集（从设置数据路径读取，列表由 MemoryPanel 提供，这里用静态常见集） */
  const memorySets = ref<string[]>(['default'])

  let unlistenFns: Array<() => void> = []

  /** 扫描局域网设备 */
  async function discover() {
    syncStatus.value = 'discovering'
    message.value = '正在扫描局域网设备…'
    try {
      const res = await discoverDevices(3)
      devices.value = res.devices
      syncStatus.value = 'idle'
      message.value = res.devices.length > 0 ? `发现 ${res.devices.length} 台设备` : '未发现设备，可尝试手动连接'
    } catch (e) {
      syncStatus.value = 'error'
      message.value = `扫描失败：${e}`
    }
  }

  /** 手动连接（UDP 被阻断时的备选） */
  async function manualConnect() {
    if (!manualIp.value.trim()) {
      message.value = '请输入目标 IP'
      return
    }
    syncStatus.value = 'discovering'
    message.value = `正在连接 ${manualIp.value}:${manualPort.value}…`
    try {
      const res = await addManualDevice(manualIp.value.trim(), manualPort.value)
      devices.value = res.devices
      syncStatus.value = 'idle'
      message.value = '手动连接成功，设备已加入列表'
    } catch (e) {
      syncStatus.value = 'error'
      message.value = `连接失败：${e}`
    }
  }

  /** 开始同步（从目标设备拉取记忆集） */
  async function doSync(targetDevice: string, setName: string) {
    syncStatus.value = 'syncing'
    syncProgress.value = 0
    message.value = '正在同步…'
    try {
      const req = {
        target_device: targetDevice,
        set_name: setName,
        manual_ip: connectionMode.value === 'manual' ? manualIp.value.trim() || null : null,
        manual_port: connectionMode.value === 'manual' ? manualPort.value : null,
      }
      const res = await startSync(req)
      syncStatus.value = res.success ? 'done' : 'error'
      message.value = res.message
      syncProgress.value = 1
      // P3：第一次同步成功里程碑（幂等）
      if (res.success) {
        useMilestoneStore().record('first_sync', '第一次同步成功').catch(() => {})
      }
      await refreshStatus()
    } catch (e) {
      syncStatus.value = 'error'
      message.value = `同步失败：${e}`
    }
  }

  /** 刷新同步状态与历史 */
  async function refreshStatus() {
    try {
      const st = await getSyncStatus()
      syncStatus.value = st.status
      syncProgress.value = st.progress
      history.value = st.history
      if (st.message) message.value = st.message
    } catch {
      /* 忽略 */
    }
  }

  /** 检查更新（force 强制） */
  async function checkUpdateNow(force = false) {
    try {
      const res = await checkUpdate(force)
      if (res.error) {
        // 检查失败（网络/限流等），如实告知，不误报"最新"
        message.value = `更新检查失败：${res.error}`
        return res
      }
      if (res.has_update && res.version_info) {
        message.value = `发现新版本 ${res.version_info.latest_version}！`
      } else {
        message.value = '当前已是最新版本'
      }
      return res
    } catch (e) {
      message.value = `更新检查失败：${e}`
      return null
    }
  }

  /** 设置冲突策略 */
  async function setPolicy(policy: 'newest' | 'local' | 'remote') {
    conflictPolicy.value = policy
    await setConflictPolicy(policy)
  }

  /** 初始化：加载缓存设备 + 网络状态 + 事件监听 */
  async function init() {
    try {
      const devs = await getSyncDevices()
      devices.value = devs.devices
    } catch {
      /* 忽略 */
    }
    try {
      const ns = await getNetworkStatus()
      networkStatus.value = ns.status
    } catch {
      /* 忽略 */
    }
    refreshStatus().catch(() => {})
    if (unlistenFns.length === 0) {
      unlistenFns.push(
        await onSyncProgress((p) => {
          if (p.total > 0) syncProgress.value = p.current / p.total
          message.value = p.phase === 'done' ? '同步完成' : `同步进度：${p.current}${p.total > 0 ? `/${p.total}` : ''}`
        }),
        await onNetworkStatusChanged((e) => {
          networkStatus.value = e.status
          if (e.status === 'offline') {
            message.value = '网络已断开，切换到脚本模式'
          } else if (e.status === 'online') {
            message.value = '网络已恢复'
          }
        }),
      )
    }
  }

  /** 清理事件监听 */
  function dispose() {
    unlistenFns.forEach((fn) => fn())
    unlistenFns = []
  }

  return {
    devices,
    syncStatus,
    syncProgress,
    connectionMode,
    manualIp,
    manualPort,
    history,
    networkStatus,
    message,
    conflictPolicy,
    memorySets,
    discover,
    manualConnect,
    doSync,
    refreshStatus,
    checkUpdateNow,
    setPolicy,
    init,
    dispose,
  }
})
