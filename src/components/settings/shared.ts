// 设置页各分页共用的小工具。只放「跨分页复用」的东西，不放任何业务状态：
//   · ensureSettingsLoaded：分页可以被单独挂载，不依赖外框 onMounted 的先后顺序
//   · exportConfigFile / importConfigFromFile：通用页与记忆页共用同一套配置导入导出
//   · SETTINGS_TOOLBOX：工具箱面板要跨分页存活，所以开关状态放在外框，分页通过 inject 取
import type { InjectionKey, Ref } from 'vue'
import { useSettingStore } from '../../stores/settingStore'
import { toastErr, toastOk } from '../../utils/toast'

/** 外框提供的工具箱开关（面板本身挂在外框上，切分页不会把它关掉） */
export interface ToolboxApi {
  open: Ref<boolean>
  toggle: () => void
}
export const SETTINGS_TOOLBOX: InjectionKey<ToolboxApi> = Symbol('settings-toolbox')

/** 确保配置已从后端加载（外框与分页都会调，已加载即直接返回） */
export async function ensureSettingsLoaded(): Promise<void> {
  const setting = useSettingStore()
  if (!setting.loaded) await setting.loadConfig()
}

/** 导出配置 JSON（通用页 / 记忆页共用） */
export async function exportConfigFile(): Promise<void> {
  const setting = useSettingStore()
  const res = await setting.exportToFile()
  if (res.success) toastOk(`已导出：${res.path}`)
  else toastErr(`导出失败：${res.error}`)
}

/** 从指定 JSON 路径导入配置（通用页 / 记忆页共用），返回是否成功 */
export async function importConfigFromFile(path: string): Promise<boolean> {
  const setting = useSettingStore()
  const target = path.trim()
  if (!target) {
    toastErr('请先填写配置文件的完整路径')
    return false
  }
  try {
    await setting.importFromFile(target)
    toastOk(`已从 ${target} 导入配置`)
    return true
  } catch (e) {
    toastErr(`导入失败：${e}`)
    return false
  }
}
