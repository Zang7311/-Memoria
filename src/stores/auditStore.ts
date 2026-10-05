import { ref } from 'vue'
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import type { AuditEntry, AuditQuery } from '../types/audit'

export const useAuditStore = defineStore('audit', () => {
  const entries = ref<AuditEntry[]>([])
  const error = ref('')
  const loading = ref(false)
  let revision = 0
  let clearing = false

  async function load(query: AuditQuery = {}) {
    if (clearing) return
    const current = ++revision
    loading.value = true
    error.value = ''
    try {
      const result = await invoke<AuditEntry[]>('list_audit', { query })
      if (current === revision) entries.value = result
    } catch (reason) {
      if (current === revision) error.value = '读取执行记录失败：' + String(reason)
    } finally {
      if (current === revision) loading.value = false
    }
  }

  async function exportJson(query: AuditQuery = {}): Promise<string | null> {
    error.value = ''
    try { return await invoke<string>('export_audit', { query }) }
    catch (reason) { error.value = '导出执行记录失败：' + String(reason); return null }
  }

  async function clear(confirmed: boolean): Promise<boolean> {
    if (!confirmed || clearing) return false
    clearing = true
    ++revision
    loading.value = true
    error.value = ''
    try {
      await invoke('clear_audit', { confirmed: true })
      entries.value = []
      return true
    } catch (reason) {
      error.value = '清空执行记录失败：' + String(reason)
      return false
    } finally { clearing = false; loading.value = false }
  }

  return { entries, error, loading, load, exportJson, clear }
})
