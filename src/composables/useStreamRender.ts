// 《铃·记忆体》流式渲染组合式函数
// 任务 5 + 任务 7：监听 Tauri 事件 chat_chunk / chat_end / chat_error，
// 驱动 store 的流式追加；send(content) 用于发送消息。
//
// 说明：后端（send_message / chat_chunk / chat_end / chat_error）由 AI-3 实现。
// 本阶段后端未就绪时，通过 mock 事件驱动 UI（渲染路径与真实事件完全一致）。
// 设置环境变量 VITE_USE_MOCK=0（或后端就绪后）即走真实 Tauri 事件。
import { ref, onMounted, onUnmounted } from 'vue'
import { useChatStore } from '../stores/chatStore'
import { useSettingStore } from '../stores/settingStore'
import { useDesktopStore } from '../stores/desktopStore'
import { sendMessage, onChatChunk, onChatEnd, onChatError, onChatUsage } from '../utils/tauri'
import type { ChatUsage } from '../types'
import type { UnlistenFn } from '@tauri-apps/api/event'

// 是否使用 mock 事件（默认关闭，走真实后端；仅当显式设置 VITE_USE_MOCK=1 时开启，用于无后端演示）
const USE_MOCK = import.meta.env.VITE_USE_MOCK === '1'

export function useStreamRender() {
  const chat = useChatStore()
  const setting = useSettingStore()
  const desktop = useDesktopStore()
  const unlisteners = ref<UnlistenFn[]>([])
  // 当前正在流式的消息 id
  let activeId: string | null = null

  // —— AI 工具箱意图检测：开启 ai_toolbox 且消息匹配工具意图时，直接执行工具箱工具 ——
  function detectToolboxIntent(content: string): { id: string; input?: string } | null {
    const t = content.toLowerCase()
    if (/清理内存|释放内存|内存清理/.test(t)) return { id: 'clean-memory' }
    if (/截屏|截图|屏幕截图/.test(t)) return { id: 'screenshot' }
    if (/内网ip|本机ip|我的ip|局域网ip/.test(t)) return { id: 'lan-ip' }
    if (/运势|今日运势|运气/.test(t)) return { id: 'fortune' }
    if (/锁定屏幕|锁屏|锁电脑/.test(t)) return { id: 'lock' }
    if (/取消关机/.test(t)) return { id: 'cancel-shutdown' }
    if (/测速|网速检测/.test(t)) return { id: 'speedtest' }
    if (/蓝屏|dump/.test(t)) return { id: 'bsod' }
    if (/虚拟机|打开虚拟机/.test(t)) return { id: 'vm' }
    const ping = t.match(/ping\s+([\w.:\-]+)/)
    if (ping) return { id: 'ping', input: ping[1] }
    const ip = t.match(/查(?:询)?ip\s+([\d.]+)/)
    if (ip) return { id: 'ip-lookup', input: ip[1] }
    return null
  }

  function makeId() {
    return `msg_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`
  }

  // —— 处理一个片段（真实事件与 mock 共用同一路径）——
  function handleChunk(chunk: string) {
    if (activeId) chat.appendToMessage(activeId, chunk)
  }

  // —— 处理结束 ——
  function handleEnd() {
    if (activeId) chat.finishStream(activeId)
    activeId = null
    // 流式结束后把当前会话保存到后端（多会话持久化）
    chat.saveCurrentSession().catch(() => {})
  }

  // —— 处理错误 ——
  function handleError(_error: string) {
    if (activeId) chat.errorStream(activeId)
    activeId = null
    chat.saveCurrentSession().catch(() => {})
  }

  // —— 处理 token 用量（API 模式流式结束）——
  function handleUsage(u: ChatUsage) {
    chat.setUsage(u)
  }

  // 发送一条消息：追加用户消息 -> 创建空的铃回复 -> 触发后端/ mock 流式
  async function send(content: string, depth = 2) {
    if (chat.isLoading) return
    // 确保有活跃会话（多会话模式下首次发送前自动建一个）
    if (!chat.activeSessionId) {
      await chat.createSession()
    }
    const userMsg = {
      id: makeId(),
      role: 'user' as const,
      content,
      timestamp: new Date().toISOString(),
    }
    const assistantId = makeId()
    const assistantMsg = {
      id: assistantId,
      role: 'assistant' as const,
      content: '',
      timestamp: new Date().toISOString(),
    }

    chat.addMessage(userMsg)
    chat.addMessage(assistantMsg)
    activeId = assistantId
    chat.beginStream(assistantId)

    // —— AI 工具箱：开启且命中工具意图时，直接执行工具箱工具并返回结果（不走 AI 模型）——
    const intent = setting.aiToolbox ? detectToolboxIntent(content) : null
    if (intent) {
      try {
        const result = await desktop.executeToolboxItem(intent.id, intent.input)
        const out = result?.error
          ? `执行工具箱「${intent.id}」失败：${result.error}`
          : result?.output ?? `已执行工具箱「${intent.id}」`
        handleChunk(out)
      } catch (e) {
        handleChunk(`执行工具箱「${intent.id}」出错：${e}`)
      }
      handleEnd()
      return
    }

    if (USE_MOCK) {
      await runMock(content)
      return
    }

    try {
      await sendMessage(content, depth)
    } catch (e) {
      // 后端未实现或 IPC 异常：退化为 mock，保证 UI 可演示
      console.warn('[send_message] 调用失败，回退 mock：', e)
      await runMock(content)
    }
  }

  // mock 流式：拆分为 2~5 字片段，用定时器驱动（仅测试用途）
  function runMock(content: string) {
    return new Promise<void>((resolve) => {
      const reply = `收到啦主人～关于「${content || '这件事'}」，铃已经在为你记忆并记录在本子里啦。😊`
      const pieces: string[] = []
      let i = 0
      while (i < reply.length) {
        const size = 2 + Math.floor(Math.random() * 4)
        pieces.push(reply.slice(i, i + size))
        i += size
      }
      let idx = 0
      const timer = setInterval(() => {
        if (idx >= pieces.length) {
          clearInterval(timer)
          handleEnd()
          resolve()
          return
        }
        handleChunk(pieces[idx])
        idx++
      }, 100)
    })
  }

  onMounted(() => {
    if (!USE_MOCK) {
      Promise.all([onChatChunk(handleChunk), onChatEnd(handleEnd), onChatError(handleError), onChatUsage(handleUsage)]).then(
        (fns) => (unlisteners.value = fns)
      )
    }
  })

  onUnmounted(() => {
    unlisteners.value.forEach((fn) => fn())
    unlisteners.value = []
  })

  return { send }
}
