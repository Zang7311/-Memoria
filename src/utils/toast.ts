// 轻量消息提示（替代 alert）：任何地方调 toast() / toastOk() / toastErr()，
// 由 <AppToast /> 统一渲染在窗口右下角，几秒后自动消失。
import { reactive } from 'vue'

export type ToastKind = 'info' | 'ok' | 'err'

export interface ToastItem {
  id: number
  text: string
  kind: ToastKind
}

/** 全局队列（模块级，窗口内共享一份） */
export const toastState = reactive<{ items: ToastItem[] }>({ items: [] })

let seq = 0

export function toast(text: string, kind: ToastKind = 'info', ms = 2600): number {
  const id = ++seq
  toastState.items.push({ id, text, kind })
  // 最多同时显示 4 条，多的把最旧的顶掉
  if (toastState.items.length > 4) toastState.items.shift()
  window.setTimeout(() => dismiss(id), ms)
  return id
}

export function dismiss(id: number) {
  const index = toastState.items.findIndex((t) => t.id === id)
  if (index >= 0) toastState.items.splice(index, 1)
}

export const toastOk = (text: string, ms = 2200) => toast(text, 'ok', ms)
export const toastErr = (text: string, ms = 4200) => toast(text, 'err', ms)
