// 《铃·记忆体》AI-5 插件 Store（usePluginStore）
// 管理插件列表、安装/卸载/启用/禁用/权限/终端命令
import { defineStore } from 'pinia'
import {
  addTerminalCommand,
  disablePlugin,
  enablePlugin,
  installPlugin,
  listPlugins,
  setPluginPermission,
  uninstallPlugin,
} from '../utils/tauri'
import type { Plugin } from '../types'

interface PluginState {
  plugins: Plugin[]
  isLoading: boolean
  errorMsg: string
  /** 安装弹窗当前输入（本地路径 或 Git URL） */
  installSource: string
  /** 终端命令表单 */
  termName: string
  termCommand: string
  termDescription: string
}

export const usePluginStore = defineStore('plugin', {
  state: (): PluginState => ({
    plugins: [],
    isLoading: false,
    errorMsg: '',
    installSource: '',
    termName: '',
    termCommand: '',
    termDescription: '',
  }),

  getters: {
    /** 判断是否为终端命令插件（manifest.main 为空） */
    isTerminalPlugin: () => (p: Plugin) => p.manifest.main.trim() === '',
    /** 普通插件（排除终端命令） */
    normalPlugins(state): Plugin[] {
      return state.plugins.filter((p) => !this.isTerminalPlugin(p))
    },
    /** 终端命令插件 */
    terminalPlugins(state): Plugin[] {
      return state.plugins.filter((p) => this.isTerminalPlugin(p))
    },
    enabledCount(state): number {
      return state.plugins.filter((p) => p.enabled).length
    },
  },

  actions: {
    async loadPlugins() {
      this.isLoading = true
      this.errorMsg = ''
      try {
        const res = await listPlugins()
        this.plugins = res.plugins
      } catch (e) {
        this.errorMsg = String(e)
      } finally {
        this.isLoading = false
      }
    },

    async install(source: string) {
      this.errorMsg = ''
      try {
        const plugin = await installPlugin(source)
        // 安装成功后刷新列表
        await this.loadPlugins()
        return plugin
      } catch (e) {
        this.errorMsg = String(e)
        throw e
      }
    },

    async uninstall(id: string) {
      this.errorMsg = ''
      try {
        await uninstallPlugin(id)
        this.plugins = this.plugins.filter((p) => p.id !== id)
      } catch (e) {
        this.errorMsg = String(e)
        throw e
      }
    },

    async toggle(plugin: Plugin) {
      this.errorMsg = ''
      try {
        const updated = plugin.enabled ? await disablePlugin(plugin.id) : await enablePlugin(plugin.id)
        const idx = this.plugins.findIndex((p) => p.id === plugin.id)
        if (idx >= 0) this.plugins[idx] = updated
      } catch (e) {
        this.errorMsg = String(e)
        throw e
      }
    },

    async setPermission(id: string, permission: string, allow: boolean) {
      this.errorMsg = ''
      try {
        const updated = await setPluginPermission(id, permission, allow)
        const idx = this.plugins.findIndex((p) => p.id === id)
        if (idx >= 0) this.plugins[idx] = updated
      } catch (e) {
        this.errorMsg = String(e)
        throw e
      }
    },

    async addTerminal(name: string, command: string, description: string) {
      this.errorMsg = ''
      try {
        const plugin = await addTerminalCommand(name, command, description)
        this.plugins.push(plugin)
        return plugin
      } catch (e) {
        this.errorMsg = String(e)
        throw e
      }
    },
  },
})
