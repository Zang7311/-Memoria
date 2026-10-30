// 《铃·记忆体》桌面文件拖进窗口 → 变成附件
//
// Tauri v2 的拖放事件只给文件**路径**，内容得让后端读（read_dropped_file 命令），
// 所以这里只做三件事：挂监听、把路径交给后端、把结果广播给输入框。
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import type { Attachment } from '../types'

/** 后端返回的结构（与 src-tauri/src/commands/read_dropped_file.rs 对齐） */
interface DroppedPayload {
  name: string
  kind: 'image' | 'text'
  mime: string
  data: string
  size: number
}

/** 一次最多收几个文件，防止手滑拖一整个文件夹进来 */
const MAX_FILES = 5

/** 输入框监听这个事件接收附件 */
export const DROP_EVENT = 'ling:attachments-dropped'

export interface DropResult {
  ok: Attachment[]
  failed: string[]
}

/** 逐个把路径读成附件；单个失败不影响其它文件 */
export async function readDroppedPaths(paths: string[]): Promise<DropResult> {
  const ok: Attachment[] = []
  const failed: string[] = []
  for (const path of paths.slice(0, MAX_FILES)) {
    const baseName = path.split(/[\\/]/).filter(Boolean).pop() || path
    try {
      const payload = await invoke<DroppedPayload>('read_dropped_file', { path })
      // 同名同大小视为重复，别让用户拖两次变成两个附件
      if (!ok.some((a) => a.name === payload.name && a.size === payload.size)) {
        ok.push({ name: payload.name, kind: payload.kind, mime: payload.mime, data: payload.data, size: payload.size })
      }
    } catch (e) {
      failed.push(`${baseName}：${e instanceof Error ? e.message : String(e)}`)
    }
  }
  if (paths.length > MAX_FILES) failed.push(`一次最多 ${MAX_FILES} 个文件，多余的没加进来`)
  return { ok, failed }
}

/**
 * 装窗口级拖放监听。
 * onActive 用来画「松开就加进附件」的高亮层；返回取消监听函数。
 */
export async function installWindowFileDrop(
  onActive: (active: boolean) => void,
  onFiles: (result: DropResult) => void,
): Promise<() => void> {
  const win = getCurrentWebviewWindow()
  const unlisten = await win.onDragDropEvent(async (event) => {
    const type = event.payload.type
    if (type === 'enter' || type === 'over') {
      onActive(true)
      return
    }
    onActive(false)
    if (type !== 'drop') return
    const result = await readDroppedPaths(event.payload.paths)
    if (result.ok.length || result.failed.length) onFiles(result)
  })
  return unlisten
}

/** 把结果广播给输入框（ChatInput 收到后追加到待发附件） */
export function broadcastDrop(result: DropResult) {
  window.dispatchEvent(new CustomEvent<DropResult>(DROP_EVENT, { detail: result }))
}
