// 全局确认框（替代 confirm）：调用处写 await confirmDialog('删除该会话？')，
// 由 <AppDialog /> 统一渲染，返回 true/false。
import { reactive } from 'vue'

export interface DialogOptions {
  title?: string
  confirmText?: string
  cancelText?: string
  /** 危险操作：确认按钮用错误色 */
  danger?: boolean
}

interface DialogState extends Required<DialogOptions> {
  open: boolean
  text: string
}

export const dialogState = reactive<DialogState>({
  open: false,
  title: '确认',
  text: '',
  confirmText: '确定',
  cancelText: '取消',
  danger: false,
})

let resolver: ((value: boolean) => void) | null = null

/** 弹一个确认框，等待用户选择 */
export function confirmDialog(text: string, options: DialogOptions = {}): Promise<boolean> {
  // 连着弹两个时，前一个按取消处理，避免 resolver 被覆盖后永远悬着
  if (resolver) resolveDialog(false)
  dialogState.title = options.title ?? '确认'
  dialogState.text = text
  dialogState.confirmText = options.confirmText ?? '确定'
  dialogState.cancelText = options.cancelText ?? '取消'
  dialogState.danger = options.danger ?? false
  dialogState.open = true
  return new Promise<boolean>((resolve) => {
    resolver = resolve
  })
}

export function resolveDialog(value: boolean) {
  if (!dialogState.open && !resolver) return
  dialogState.open = false
  const done = resolver
  resolver = null
  done?.(value)
}
